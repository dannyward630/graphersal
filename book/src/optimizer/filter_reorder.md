# Filter Reorder Rule

Rule name: `filter_reorder` (on by default, third in the pipeline).

## What it rewrites

Every run of **adjacent** filter steps is sorted by an estimated cost, cheapest first. The sort is
stable: filters of the same cost keep the order in which they were written.

| Rank | Filters | Why this cost |
|---|---|---|
| 0 | `has_id` | an id comparison |
| 1 | `has_label` | a check of the element's labels |
| 2 | `has_key`, `has_property`, `has_not`, `is(GType...)` | a key presence or type check, no value comparison |
| 3 | `has`, `has` with a predicate, `is`, `has_value`, `has_id`/`has_label`/`has_key` with a predicate | a property value is read and compared |
| 4 | `where(...)`, `filter(...)`, `not(...)`, `and(...)`, `or(...)` | a child traversal runs per traverser |
| 5 | every other filter step | unknown cost |

## Why

A cheap filter that runs first shrinks the stream that the expensive filters must look at. It also
moves `has_id`/`has_label` to the front of the run, directly behind `v()`/`e()`, where
[Has ID Pushdown](has_id_pushdown.md) and [Source Filter Pushdown](source_filter_pushdown.md) can
fold them into the start step.

## When it does not fire

- The filters are already in rank order (the profile then does not list the rule).
- The filters are not adjacent: any non-filter step between them, including `as()`, `out()` or a
  `by()` modulator, ends the run. Only filters inside one run are sorted, nothing moves across
  another step.
- A run of a single filter.
- A filter with an effect (see [Results](#results)) splits the run: the filters before it and the
  filters after it are sorted separately.

## Example

`has("age", ...)` is written before `has_label("person")`. The rule swaps them, and the label filter
then folds into `v()`:

```console
$ graphersal -e 'g.v().has("age", P.gt(30)).has_label("person").values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"])                                           1       0       4    3.875µs     8.44
has("age", P.gt(30))                                            1       4       2    3.959µs     8.62
values("name")                                                  1       2       2    2.500µs     5.44
                                                      TOTAL:             execute:   45.916µs    22.51
=====================================================================================================
Optimizer rules applied: filter_reorder, source_filter_pushdown
```

Disabled, the plan keeps the written order, and the label filter can no longer be folded because it
does not follow `v()` directly:

```console
$ graphersal -e 'g.with("optimizer.disabled", ["filter_reorder"]).v().has("age", P.gt(30)).has_label("person").values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    1.417µs     3.91
has("age", P.gt(30))                                            1       6       2    4.084µs    11.27
has_label("person")                                             1       2       2      666ns     1.84
values("name")                                                  1       2       2    2.333µs     6.44
                                                      TOTAL:             execute:   36.250µs    23.45
=====================================================================================================
```

The rule also works in the middle of a traversal, where no pushdown follows:

```console
$ graphersal -e 'g.v().out().has("lang", "java").has_id("3").values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    2.833µs     3.53
out()                                                           1       6       6    5.459µs     6.80
has_id("3")                                                     1       6       3    2.084µs     2.60
has("lang", "java")                                             1       3       3    2.417µs     3.01
values("name")                                                  1       3       3    1.584µs     1.97
                                                      TOTAL:             execute:   80.250µs    17.92
=====================================================================================================
Optimizer rules applied: filter_reorder
```

## Results

For pure filters, the same results in the same order: a run of filters keeps a traverser only if
every filter in the run keeps it, whatever their order.

A filter with an effect is never moved, and nothing moves across it: a filter that mutates the
graph or writes a side effect, itself (`drop()`) or anywhere in its child traversals
(`filter(__.aggregate("x"))`, `where(__.sideEffect(..))`, `filter(__.property(..))`), ends the
run like a non-filter step. It therefore sees exactly the traversers it sees in the written order:

```console
$ graphersal -e 'g.v().filter(__.aggregate("x")).has_label("software").cap("x").count(Scope.local).to_list()'
6
$ graphersal -e 'g.with("optimizer.disabled", ["filter_reorder"]).v().filter(__.aggregate("x")).has_label("software").cap("x").count(Scope.local).to_list()'
6
```

Filters that can raise an error are reordered like any other filter (by design, as TinkerPop's
`FilterRankingStrategy` does): a cheap filter moved in front of one that fails for some elements
can drop those elements first, so a query that fails unoptimized may succeed optimized. The
results of a query that succeeds both ways are the same.

## Interaction with other rules

Runs after [Where Unnest](where_unnest.md), so filters inlined from a `where()` are sorted too, and
before the pushdown rules, which only fold filters that directly follow the start step.
