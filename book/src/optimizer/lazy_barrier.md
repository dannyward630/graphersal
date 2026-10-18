# Lazy Barrier Rule

Rule name: `lazy_barrier` (on by default, runs last).

## What it rewrites

Between two adjacent adjacency steps the rule inserts a merge point, shown as `lazy_barrier()` in
`.profile()`:

```rhai
v().out().out().count()   →   v().out().lazy_barrier().out().count()
```

It inserts before `out`, `in`, `both`, `out_e`, `in_e`, `both_e`, `out_v`, `in_v`, `both_v` or
`other_v` when the previous step is `out`, `in`, `both`, `out_v`, `in_v`, `both_v` or `other_v` (the
steps whose output can contain duplicates; an `out_e()` yields each edge once per input). Modulators
between the two are ignored. This follows TinkerPop's `LazyBarrierStrategy`.

## Why

Equal traversers (the same vertex reached along different routes) are merged into one traverser
that carries a bulk, so the next step expands each distinct vertex once instead of once per route.
On a dense graph this turns an exponential number of traversers into a bounded one. See
[Bulk and Barriers](../users_guide/bulk_and_barriers.md).

A merge costs a hash insert per traverser and pays only through the work it saves the next step,
so on a stream of 4096 traversers or more `lazy_barrier()` first estimates that saving: over the
first 4096 traversers it adds, for every one that would merge into an earlier one, its fan-out on
the following step (how many traversers that step would produce from it, counted without producing
them). Fewer than one saved output per two traversers and the stream passes through unmerged. A
high merge ratio alone is not enough: when the vertices the duplicates meet on have no edges in the
next step's direction, merging saves nothing (measured on an org chart: `has_label("person").out()`
gives 1.64M traversers that merge into 183k, but the merge cost 39 ms and saved 12 ms on the
following `out()`). Smaller streams are always merged. If the estimate says merge, the pass still
stops early when, after the first 4096 traversers, fewer than one in sixteen merged (and a
`repeat()` stops merging for its remaining iterations). Neither check changes results.

## When it does not fire

- Never across `as()` or any other non-modulator step: `v().out().as("x").out()` gets no merge
  point.
- Never at the end of a traversal level, and never next to an explicit `barrier()`.
- Never directly behind `out_e()`/`in_e()`/`both_e()`.
- Nested traversals (`repeat()` bodies, `union()` branches) are optimized on their own, so the rule
  applies inside each of them separately.

In some plans the rule still inserts its step, but the step passes traversers through without
merging (the profile row shows `Out` equal to `In`):

- the plan mutates the graph or records full paths (`path()`, `simple_path()`, shown as
  `[path: full]`), because then no two traversers are equal;
- `withSack(init, Operator)` with a merge operator, or `withBulk(false)`: merging is visible there,
  so only an explicit `barrier()` merges. See
  [Sack and Operators](../users_guide/sack_and_operators.md#automatic-merging-and-sacks);
- `g.with("bulk.merge", false)`, which turns every merge off.

## Example

On the modern graph, the six traversers that `out()` produces hold only four distinct vertices:

```console
$ graphersal -e 'g.v().out().out().count().profile()'
Traversal Metrics
Step                                                         Call      In     Out Bulk       Time    % Dur
==========================================================================================================
v()                                                             1       0       6    6  148.167µs     2.41
out()                                                           1       6       6    6   17.208µs     0.28
lazy_barrier()                                                  1       6       4    6  560.750µs     9.13
out()                                                           1       4       2    2  145.042µs     2.36
count()                                                         1       2       1    1      333ns     0.01
                                                      TOTAL:                  execute:    6.141ms    14.19
==========================================================================================================
Optimizer rules applied: lazy_barrier

$ graphersal -e 'g.with("optimizer.disabled", ["lazy_barrier"]).v().out().out().count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    6.542µs     5.16
out()                                                           1       6       6    7.709µs     6.08
out()                                                           1       6       2    5.584µs     4.40
count()                                                         1       2       1      375ns     0.30
                                                      TOTAL:             execute:  126.792µs    15.94
=====================================================================================================
```

The `Bulk` column (shown when some traverser carries a bulk above 1) is the number of traversers
the stream stands for; `Out` is the number of traverser objects. On the tiny modern graph the merge
pass costs more than it saves. On the `large` graph (about 110k vertices) it cuts a three-hop
expansion from about 896 ms to about 282 ms (a debug build):

```console
$ graphersal --graph large -e 'g.v().both().both().both().count().profile()'
Traversal Metrics
Step                                                         Call      In     Out    Bulk       Time    % Dur
=============================================================================================================
v()                                                             1       0  111110  111110    5.236ms     1.86
both()                                                          1  111110  222200  222200   52.350ms    18.58
lazy_barrier()                                                  1  222200  111110  222200   72.081ms    25.58
both()                                                          1  111110  222200 1444100   41.948ms    14.89
lazy_barrier()                                                  1  222200  111110 1444100   69.180ms    24.55
both()                                                          1  111110 4884000 4884000   37.058ms    13.15
count()                                                         1 4884000       1       1      500ns     0.00
                                                      TOTAL:                     execute:  281.776ms    98.61
=============================================================================================================
Optimizer rules applied: lazy_barrier

$ graphersal --graph large -e 'g.with("optimizer.disabled", ["lazy_barrier"]).v().both().both().both().count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0  111110    5.487ms     0.61
both()                                                          1  111110  222200   41.880ms     4.67
both()                                                          1  222200 1444100  203.851ms    22.75
both()                                                          1 1444100 4884000  644.628ms    71.95
count()                                                         1 4884000       1      500ns     0.00
                                                      TOTAL:             execute:  895.971ms    99.99
=====================================================================================================
```

(Timings are from a debug build and vary between runs; the traverser counts do not.)

With full path recording the merge point is inserted but passes everything through:

```console
$ graphersal -e 'g.v().out().out().path().by("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v() [path: full]                                                1       0       6    8.458µs     1.71
out() [path: full]                                              1       6       6    7.625µs     1.54
lazy_barrier() [path: full]                                     1       6       6    2.250µs     0.46
out() [path: full]                                              1       6       2    1.417µs     0.29
path().by("name")                                               1       2       2  189.292µs    38.33
                                                      TOTAL:             execute:  493.791µs    42.33
=====================================================================================================
Optimizer rules applied: lazy_barrier
```

## Results

The same multiset of results with and without the rule (`2` for the count above, `4884000` on the
large graph). Only the order of duplicates may change, because merged traversers are expanded
together:

```console
$ graphersal -e 'g.v().out().in().values("name").to_list()'
"marko"
"peter"
"peter"
"peter"
"josh"
"josh"
"josh"
"marko"
"marko"
"marko"
"marko"
"josh"
$ graphersal -e 'g.with("optimizer.disabled", ["lazy_barrier"]).v().out().in().values("name").to_list()'
"marko"
"peter"
"josh"
"marko"
"marko"
"josh"
"peter"
"josh"
"marko"
"peter"
"josh"
"marko"
```

## Interaction with other rules

Runs last, on the final shape of every level: steps that earlier rules fused or removed are no
longer between two adjacency steps. Turn it off for one query with
`g.with("optimizer.disabled", ["lazy_barrier"])`, or merge nothing at all with
`g.with("bulk.merge", false)`. The [path requirement analysis](path_analysis.md) runs after it and
decides whether the inserted steps can merge (`[path: full]` disables merging).
