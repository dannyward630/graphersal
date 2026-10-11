# Has ID Pushdown Rule

Rule name: `has_id_pushdown` (on by default, fourth in the pipeline).

## What it rewrites

A `has_id(...)` filter in the run of filters directly behind a leading `v()` or `e()` is removed and
its ids are moved into the start step, which then looks the ids up instead of scanning every
element:

```
v().has_id("1", "2")         →   v("1", "2")
v("1", "2").has_id("2", "3") →   v("2")              (the intersection)
v().has_label("person").has_id("1")  →  v("1").has_label("person")
```

- If the start step already has ids, the new id list is the intersection of both lists.
- The rule looks past `has_label` filters while it searches for `has_id` (it does not fold the
  labels; that is [Source Filter Pushdown](source_filter_pushdown.md)'s job).
- `has_id(P.within(...))` is built as a plain `has_id` and folds the same way.

## Why

An id lookup costs one hash-map access per id; the scan it replaces touches every vertex or edge of
the graph.

## When it does not fire

- The traversal does not start with `v()`/`e()`. A `v()` in the middle of a traversal
  (`g.v("1").as("a").v()`) and child traversals (`__.has_id("1")`) are left alone.
- Any step other than `has_id`/`has_label` sits between the start step and the `has_id`:
  `g.v().out().has_id("3")` keeps its filter.
- A `has_id` with another predicate (`has_id(P.neq("1"))`, `has_id(P.gt(...))`): it has no fixed id
  list to fold.

## Example

```
$ graphersal -e 'g.v("1", "2").has_id("2", "3").values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v("2")                                                          1       0       1    7.042µs     6.33
values("name")                                                  1       1       1    5.459µs     4.91
                                                      TOTAL:             execute:  111.209µs    11.24
=====================================================================================================
Optimizer rules applied: has_id_pushdown
```

With both rules that fold ids disabled, the scan and the filter stay separate (Filter Reorder
still moves `has_id` to the front):

```
$ graphersal -e 'g.with("optimizer.disabled", ["has_id_pushdown", "source_filter_pushdown"]).v().has_label("person").has_id("1", "2", "3").values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    5.500µs     4.56
has_id("1", "2", "3")                                           1       6       3    5.000µs     4.15
has_label("person")                                             1       3       2    3.667µs     3.04
values("name")                                                  1       2       2    5.917µs     4.91
                                                      TOTAL:             execute:  120.541µs    16.66
=====================================================================================================
Optimizer rules applied: filter_reorder
```

and with both enabled (the default), ids and labels end up in the start step:

```
$ graphersal -e 'g.v().has_label("person").has_id("1", "2", "3").values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(ids: ["1", "2", "3"], labels: ["person"])                     1       0       2    1.750µs     4.61
values("name")                                                  1       2       2    3.125µs     8.22
                                                      TOTAL:             execute:   38.000µs    12.83
=====================================================================================================
Optimizer rules applied: filter_reorder, has_id_pushdown, source_filter_pushdown
```

An empty intersection leaves a start step with an empty id list, which produces nothing. The
profile renders that empty list as `v([])`; the `Out` column (0) shows that nothing was scanned:

```
$ graphersal -e 'g.v("1").has_id("2").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v([])                                                           1       0       0    1.667µs     1.94
                                                      TOTAL:             execute:   86.000µs     1.94
=====================================================================================================
Optimizer rules applied: has_id_pushdown
```

## Results

Unchanged: `g.v("1", "2").has_id("2", "3")` returns `"vadas"` with and without the rule. An id that
does not exist is skipped by the lookup exactly as the filter would drop it.

A repeated id in the filter is folded once: the filter's ids are a set (`has_id("1", "1")` passes
vertex 1 once), while a start step's ids are a sequence (`g.v("1", "1")` yields vertex 1 twice, as
in TinkerPop), so `g.v().has_id("1", "1").count()` is `1` with and without the rule.

## Interaction with other rules

[Source Filter Pushdown](source_filter_pushdown.md), which runs right after, folds `has_id` as well,
so in the default pipeline the two overlap: disabling only `has_id_pushdown` produces the same plan.
`has_id_pushdown` is the one that still folds when the ids sit behind more than one `has_label`
(Source Filter Pushdown stops at the second `has_label`) and [Filter Reorder](filter_reorder.md) has
been disabled. With Filter Reorder on, `has_id` is moved in front of every `has_label` first.

```
$ graphersal -e 'g.with("optimizer.disabled", ["filter_reorder"]).v().has_label("person").has_label("software").has_id("3").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(ids: ["3"], labels: ["person"])                               1       0       0    1.541µs     4.71
has_label("software")                                           1       0       0      333ns     1.02
                                                      TOTAL:             execute:   32.709µs     5.73
=====================================================================================================
Optimizer rules applied: has_id_pushdown, source_filter_pushdown

$ graphersal -e 'g.with("optimizer.disabled", ["filter_reorder", "has_id_pushdown"]).v().has_label("person").has_label("software").has_id("3").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"])                                           1       0       4    5.375µs     9.38
has_label("software")                                           1       4       0    1.500µs     2.62
has_id("3")                                                     1       0       0       42ns     0.07
                                                      TOTAL:             execute:   57.333µs    12.06
=====================================================================================================
Optimizer rules applied: source_filter_pushdown
```
