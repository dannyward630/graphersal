# Branch Children and Global State

`union()`, `choose()` and `branch()` run their child traversals over a **stream** of traversers.
This page lists where Graphersal differs from Apache TinkerPop 3.8.2 in how it feeds that stream
to a child.

## The rule

A child traversal is *stateful* when one of its top-level steps holds global state: a barrier
(`count`, `sum`, `min`, `max`, `mean`, `fold`, `group`, ...) or a stream-wide step (`order`,
`dedup`, `limit`, `range`, `tail`, `skip`-like steps).

- **Stateful child:** every traverser routed to the child is fed into **one** run of it, as in
  TinkerPop's `BranchStep`. `union(__.limit(1), __.limit(2))` over six vertices returns 3 results,
  and `union(__.outE().count())` returns one total, not one count per vertex.
- **Stateless child:** each traverser runs through the child on its own. The multiset of results is
  the same as in TinkerPop.
- `local(...)` is the per-traverser form of a stateful child:
  `union(__.outE().count(), __.local(__.inE().count()))` returns one global count and then one
  count per vertex.

For `choose()` and `branch()` the selector still runs per traverser; the traversers it routes to an
option form that option's stream. A traverser dropped by the selection joins no stream; one that
`choose()` passes through unchanged is emitted after the option streams. The option keys and the
unmatched-traverser rules are on [Option Keys](option_semantics.md).

## Deviations

- **Output order.** With a stateful child the branches run one after the other, so the output is
  branch-major (all results of the first branch, then of the second); for `choose()`/`branch()` it is
  option-major (declaration order). Without a stateful child the output stays traverser-major.
  TinkerPop does not promise an order for either, so only the order of results can differ.
- **What counts as stateful is decided from the optimized plan, by step kind.** `repeat()`,
  `barrier()` and `discard()` do not make a child stateful (a loop streams in TinkerPop), except a
  `repeat()` whose body holds a `dedup()`: its seen-set lives for the whole loop, so the child runs
  as one batch. A `limit` inside a `repeat()` body is not looked at here: `repeat()` keeps its own
  frontier semantics.
- **Bulk.** A stateful child receives the stream with its bulks (`bulk.merge`, on by default), so a
  `count()` is bulk-weighted; results are the same with merging on or off.
- **Side effects.** A child that writes side effects runs once for the whole stream, not once per
  traverser.

```rhai,graph=modern
g.V().union(__.limit(1), __.limit(2)).toList().len()                       // 3
g.V("1","2").union(__.outE().count(), __.inE().count()).toList()           // 3, 1
g.V().choose(__.values("age").is(P.lte(30)),
             __.out().order().by("name").limit(1),
             __.out().order().by("name").limit(2)).count().next()          // 3
```
