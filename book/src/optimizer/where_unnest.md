# Where Unnest Rule

Rule name: `where_unnest` (on by default, runs first).

## What it rewrites

A `where(__...)` whose child traversal consists **only** of local element filters is replaced by
those filters, inlined at the position of the `where()`:

```rhai
v().where(__.has_label("person").has("age", P.gt(30)))   →   v().has_label("person").has("age", ...)
```

The local filters are `has`, `has_label`, `has_id`, `has_not`, `has_key`, `has_property`,
`has_value`, the predicate forms of `has` (`has_p`), `is` and `is(GType...)`. A `where()` with an
empty child traversal (which keeps every traverser) is removed.

## Why

A `where()` runs its child traversal once per incoming traverser. A child made of filters only
neither walks the graph nor changes the number of results, so running it inline is equivalent and
saves the per-traverser child execution. More importantly, the inlined filters become visible to
the rules that run later: [Filter Reorder](filter_reorder.md) can sort them, and
[Source Filter Pushdown](source_filter_pushdown.md) / [Has ID Pushdown](has_id_pushdown.md) can fold
them into `v()`/`e()`.

## When it does not fire

- The child contains any other step: `where(__.out("created"))`, `where(__.has_label("person").out())`,
  `where(__.values("age").is(P.gt(30)))`.
- The child starts or ends with an `as()` label (`where(__.as("a")...)`): a start or end label
  changes what the child runs on, so it cannot be flattened.
- `where(P...)` (the predicate form), `filter(__...)`, `not(__...)`, `and(...)`, `or(...)`: the rule
  looks at `where(traversal)` only. `g.v().filter(__.has_label("person"))` stays a `filter()` with a
  child.

## Example

With the rule (the default), the `has_label` inside the `where()` ends up folded into `v()`:

```console
$ graphersal -e 'g.v().where(__.has_label("person").has("age", P.gt(30))).values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"])                                           1       0       4    3.500µs     6.19
has("age", P.gt(30))                                            1       4       2    4.042µs     7.15
values("name")                                                  1       2       2   11.166µs    19.75
                                                      TOTAL:             execute:   56.542µs    33.09
=====================================================================================================
Optimizer rules applied: where_unnest, source_filter_pushdown
```

Without it, the child traversal runs once per vertex:

```console
$ graphersal -e 'g.with("optimizer.disabled", ["where_unnest"]).v().where(__.has_label("person").has("age", P.gt(30))).values("name").profile()'
Traversal Metrics
Step                                                                   Call      In     Out       Time    % Dur
===============================================================================================================
v()                                                                       1       0       6    1.667µs     3.75
where(__.has_label("person").has("age", P.gt(30)))                        1       6       2    7.042µs    15.85
  \> has_label("person")                                                  6       6       4    1.292µs     2.91
     [min: 0ns, avg: 215ns, max: 1.042µs]
  \> has("age", P.gt(30))                                                 4       4       2    3.292µs     7.41
     [min: 83ns, avg: 823ns, max: 3.000µs]
values("name")                                                            1       2       2    2.208µs     4.97
                                                                TOTAL:             execute:   44.416µs    24.58
===============================================================================================================
```

Note that no other rule fired either: `source_filter_pushdown` cannot see a `has_label` hidden in a
child traversal.

## Results

Unchanged. Both plans return `"josh"` and `"peter"`, in the same order. A filter-only child keeps
or drops a traverser exactly as the same filters do inline.

## Interaction with other rules

Runs first so that every later rule sees the inlined filters. Nested traversals are optimized
before their parent level, so a `where()` inside a `repeat()` body or a `union()` branch is unnested
too.
