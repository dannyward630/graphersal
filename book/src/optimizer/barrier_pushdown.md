# Barrier Pushdown Rule

Rule name: `barrier_pushdown` (on by default, sixth in the pipeline).

Despite its name, this rule has nothing to do with barriers: it pushes edge label filters into the
edge-walking step in front of them.

## What it rewrites

The run of `has_label(...)` filters directly behind an `out_e()`, `in_e()` or `both_e()` is folded
into that step's label parameter:

```rhai
out_e().has_label("knows")                       →   out_e("knows")
out_e("knows", "created").has_label("created")   →   out_e("created")      (the intersection)
```

If the edge step already has labels, the folded list is the intersection of both lists. Unlike
[Source Filter Pushdown](source_filter_pushdown.md), every consecutive `has_label` folds: an edge
carries exactly one label, so intersecting is exact.

The rule fires anywhere in a traversal, not only at its start, and in child traversals too.

## Why

An `out_e("knows")` reads only the matching edges from the vertex's adjacency, instead of creating
a traverser for every incident edge and dropping most of them in the next step.

## When it does not fire

- The filter does not directly follow the edge step: `out_e().has("weight", ...).has_label("knows")`
  only folds because [Filter Reorder](filter_reorder.md) first moves `has_label` in front of `has`.
  An `as()` or any other step in between stops the rule.
- Vertex steps: `out().has_label("person")` filters the *adjacent vertices*, not the edges, and is
  left alone.
- `has_label` with another predicate (`has_label(P.neq("knows"))`).

## Example

```console
$ graphersal -e 'g.v("1").out_e().has_label("knows").in_v().values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v("1")                                                          1       0       1    4.625µs     4.21
out_e("knows")                                                  1       1       2    7.792µs     7.10
in_v()                                                          1       2       2    1.166µs     1.06
values("name")                                                  1       2       2    3.042µs     2.77
                                                      TOTAL:             execute:  109.791µs    15.14
=====================================================================================================
Optimizer rules applied: barrier_pushdown
```

Disabled, `out_e()` produces all three edges of vertex 1 and the filter drops one:

```console
$ graphersal -e 'g.with("optimizer.disabled", ["barrier_pushdown"]).v("1").out_e().has_label("knows").in_v().values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v("1")                                                          1       0       1    2.917µs     4.13
out_e()                                                         1       1       3    3.708µs     5.25
has_label("knows")                                              1       3       2    2.708µs     3.83
in_v()                                                          1       2       2      833ns     1.18
values("name")                                                  1       2       2    2.000µs     2.83
                                                      TOTAL:             execute:   70.625µs    17.23
=====================================================================================================
```

Together with Filter Reorder:

```console
$ graphersal -e 'g.v("1").out_e().has("weight", P.gt(0.4)).has_label("knows").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v("1")                                                          1       0       1   10.458µs     0.48
out_e("knows")                                                  1       1       2   37.375µs     1.70
has("weight", P.gt(0.4))                                        1       2       2  118.209µs     5.38
                                                      TOTAL:             execute:    2.197ms     7.56
=====================================================================================================
Optimizer rules applied: filter_reorder, barrier_pushdown
```

## Results

Unchanged, in the same order: the edge step produces the same edges the filter would have kept.

## Interaction with other rules

Runs after [Filter Reorder](filter_reorder.md), which brings `has_label` to the front of a run of
filters behind the edge step. [Lazy Barrier](lazy_barrier.md) never inserts a merge point directly
behind an `out_e()`/`in_e()`/`both_e()` (each edge is produced once), so the two rules do not
compete for the same position.
