# Source Filter Pushdown Rule

Rule name: `source_filter_pushdown` (on by default, fifth in the pipeline).

## What it rewrites

The run of `has_label(...)`, `has_id(...)` and `has(key, value)` filters directly behind a leading
`v()` or `e()` is folded into the start step, which then reads its candidates from the label index
or by id instead of scanning the whole graph, and checks the property conditions while it reads:

```rhai
v().has_label("person")                       →   v(labels: ["person"])
e().has_label("knows")                        →   e(labels: ["knows"])
v().has_label("person").has_id("1")           →   v(ids: ["1"], labels: ["person"])
v().has_label("person").has("title", "CEO")   →   v(labels: ["person"]).has("title", "CEO")
```

- `has_label(a, b)` folds as "carries at least one of `a`, `b`", which is exactly `has_label`'s
  meaning. `has_label(P.within(...))` folds the same way.
- `has_id` folds into the start step's id list (intersected with ids that are already there), like
  [Has ID Pushdown](has_id_pushdown.md).
- With ids and labels both folded, the start step looks the ids up and keeps the elements that
  carry one of the labels.
- `has(key, value)` with a plain property name folds as a condition the start step checks on each
  candidate before it creates a traverser, with exactly the equality of the `has` step. The
  profile shows the fused step as one row, `v(labels: ["person"]).has("title", "CEO")`. Without
  a [property index](../users_guide/property_indexes.md) every candidate is still read, but the
  ones that fail cost no traverser, no memory and no second pass (on 546 500 `person` vertices with
  500 matches: 18 ms → 11 ms, 42 MB → 40 KB); with one, the next rule,
  [Property Index](property_index.md), lets the start step read only the matches.

## Why

The label index answers "every vertex with label `person`" without touching other vertices. On a
graph where a label is a small share of all elements, this replaces a full scan by a read of just
that share. A folded `has(key, value)` keeps the elements that fail it from ever becoming
traversers.

## When it does not fire

- The traversal does not start with `v()`/`e()` (a mid-traversal `v()`, a child traversal).
- The first step after the start step is neither `has_label`, `has_id` nor `has(key, value)`.
  Anything in between (`as("a")`, `out()`, a predicate `has("age", P.gt(30))`, a jpath key
  `has(jpath("a.b"), 1)`) stops the rule; the filters after it stay filters.
  [Filter Reorder](filter_reorder.md) usually moves `has_label`/`has_id` to the front of a run of
  filters first, so `v().has("name", "marko").has_label("person")` folds completely.
- A folded `has(key, value)` keeps [Count Pushdown](count_pushdown.md) and
  [Group Count Pushdown](group_count_pushdown.md) from answering from the statistics (they count
  every element of the label): `v().has_label("person").has("age", 29).count()` scans and counts
  the matches.
- **Only the first `has_label` folds.** A second one (`v().has_label("person").has_label("software")`)
  stays a filter step, and so does everything after it. This is deliberate: a
  [multi-label vertex](../users_guide/multi_label_vertices.md) tagged `{a, b}` passes
  `has_label("a").has_label("b")`, and intersecting both label lists into one start step could drop
  it.
- A `has_label` with another predicate (`has_label(P.neq("person"))`) has no fixed label list and is
  left as a filter.

## Example

```console
$ graphersal -e 'g.v().has("name", "marko").has_label("person").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"]).has("name", "marko")                      1       0       1   78.041µs    15.09
                                                      TOTAL:             execute:  517.042µs    15.09
=====================================================================================================
Optimizer rules applied: filter_reorder, source_filter_pushdown
```

With the whole optimizer off, the scan touches all six vertices:

```console
$ graphersal -e 'g.with("optimizer.disabled", ["all"]).v().has_label("person").count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    2.875µs    10.18
has_label("person")                                             1       6       4    1.625µs     5.75
count()                                                         1       4       1       41ns     0.15
                                                      TOTAL:             execute:   28.250µs    16.07
=====================================================================================================
```

and with the default pipeline the label folds into `v()` (and the count then into the start step,
see [Count Pushdown](count_pushdown.md)):

```console
$ graphersal -e 'g.v().has_label("person").count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"]).count()                                   1       0       4    3.500µs    11.21
                                                      TOTAL:             execute:   31.209µs    11.21
=====================================================================================================
Optimizer rules applied: source_filter_pushdown, count_pushdown
```

Only the first of two label filters folds:

```console
$ graphersal -e 'g.v().has_label("person", "software").has_label("software").count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person", "software"])                               1       0       6    3.791µs    10.55
has_label("software")                                           1       6       2    1.083µs     3.02
count()                                                         1       2       1       41ns     0.11
                                                      TOTAL:             execute:   35.917µs    13.68
=====================================================================================================
Optimizer rules applied: source_filter_pushdown
```

## Results

Unchanged (`4` for the count above, with and without the rule). A vertex that carries several of the
requested labels is produced once, not once per matching label, and a repeated label or id in the
filter is folded once (`g.e().has_label("knows", "knows").count()` is `2`). The order of the produced elements
can differ from a full scan: the folded start step reads them label by label from the index.

```console
$ graphersal -e 'g.v().has_label("software", "person").id().fold().to_list()' -e 'g.with("optimizer.disabled", ["all"]).v().has_label("software", "person").id().fold().to_list()'
["3", "5", "1", "2", "4", "6"]
["1", "2", "3", "4", "5", "6"]
```

Add an `order()` step when a query depends on the order.

## Interaction with other rules

- [Where Unnest](where_unnest.md) and [Filter Reorder](filter_reorder.md) run first and bring label
  and id filters to the start step.
- [Group Count Pushdown](group_count_pushdown.md) and [Count Pushdown](count_pushdown.md) run later
  and need the filters already folded: they only fire when the grouping or counting step directly
  follows the start step.
