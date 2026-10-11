# Recursive Traversals

See the [Execution Options Reference](execution_options_reference.md) for the full table of every
`g.with()` key, including `repeat.order` and `repeat.max_loops` below, and how a host can lock
either one so a query cannot override it.

`repeat()` loops a traversal: the output of each iteration is the input of the next. With it you
can write variable-length paths, walk hierarchies, compute a transitive closure, and search
breadth first or depth first. Graphersal follows the semantics of TinkerPop's
`RepeatStep`, including where the modulators `times()`, `until()` and `emit()` are written.

```rhai,graph=modern
g.V("1").repeat(__.out()).times(2)                                  // two hops away
g.V("1").repeat(__.out()).until(__.has("name", "ripple")).path()    // the route to ripple
g.V("1").emit().repeat(__.out("knows")).times(2)                    // marko, josh, vadas
```

The script DSL and the Rust API use the same step names, with these Rust spellings for the
overloads Rust cannot express:

| Script | Rust |
|---|---|
| `repeat(__.out())` | `repeat(__::out(None))` |
| `repeat("a", __.out())` | `repeat_named("a", __::out(None))` |
| `times(2)` | `times(2)` |
| `until(__.has("name", "x"))` | `until(__::has("name", "x"))` |
| `emit()` | `emit()` |
| `emit(__.hasLabel("person"))` | `emit_with(__::has_label("person"))` |
| `loops()` | `loops()` |
| `loops("a")` | `loops_named("a")` |

For tree-shaped walks, such as a directory tree matched against a pattern like
`**/src/*.rs`, the [`glob_path()`](../traversal/steps/glob_path.md) step is shorter than a
`repeat()` loop: it matches the pattern level by level, prunes branches that cannot match, emits
each match once and stops on cycles. Where a loop would use `times(n)`, `glob_path("**", #{max_depth: n})`
bounds the walk to `n` levels below the start, and `prune: __...` stops it below matching vertices
the way `until()` stops a loop (see [Options](../traversal/steps/glob_path.md#options)). It is a
Gremlin extension, not part of TinkerPop.

## Stopping and emitting

- `times(n)` stops after `n` iterations. It is `until(__.loops().is(n))`.
- `until(t)` stops a traverser once `t` yields a result for it. The traverser is output.
- `emit()` also outputs every traverser the loop visits, not only those it ends with.
  `emit(t)` outputs only the traversers for which `t` yields a result.
- Without `times()`/`until()`, a traverser goes round until the body yields nothing for it.
  Without `emit()`, such a traverser is dropped, not output.

## Placement: before or after `repeat()`

Where a modulator is written decides when it is checked, exactly as in TinkerPop:

- **After** `repeat()` (do-while): the condition is checked after each iteration, so the body runs
  at least once. `repeat(__.out()).times(0)` runs the body once.
- **Before** `repeat()` (while-do): the condition is checked before each iteration, the first one
  included. `times(0).repeat(__.out())` returns its input unchanged, and
  `emit().repeat(__.out())` outputs the start vertex too.

| Written | Meaning |
|---|---|
| `repeat(__.out()).times(2)` | two iterations |
| `times(2).repeat(__.out())` | two iterations, checked first |
| `emit().repeat(__.out()).times(2)` | the start and every visited vertex, two iterations |
| `repeat(__.out()).emit().until(__.has(...))` | every visited vertex, until a match |
| `repeat(__.out()).times(2).emit().repeat(__.in())` | the `emit()` belongs to the first loop; the second `repeat()` is a new loop |

When a traverser meets both `emit` and `until` at the same check, it is output once. Setting a
modulator twice keeps the last one. A modulator with no `repeat()` before or after it, as in
`g.V().times(2)`, fails with `RepeatWithoutBody`.

## Loop counters: `loops()`

`loops()` is the number of iterations the enclosing loop has completed for the traverser:

- `0` before the first iteration and inside its body;
- `k + 1` after the body of iteration `k`, which is when postfix `until()`/`emit()` run;
- `0` outside any loop, including after a traverser left its loop.

So `repeat(__.out()).until(__.loops().is(2))` is `repeat(__.out()).times(2)`, and
`emit(__.loops().is(P.gt(0))).repeat(__.out())` emits everything except the start.

## Nested and named loops

A `repeat()` can appear in the body, in `until()`/`emit()`, or in any child traversal of another
`repeat()`. `loops()` reads the innermost loop. To read an outer loop, name it:

```rhai,graph=modern
g.V("1").repeat("a", __.out().repeat("b", __.in()).until(__.loops("b").is(1))).times(2)
g.V("1").repeat("a", __.out().repeat("b", __.in()).until(__.loops("a").is(0))).times(1)
```

`loops(name)` reads the nearest enclosing loop with that name. A name no enclosing loop has
fails with `UnknownLoopName`, which lists the active loop names.

## Cycles and limits

On a graph with cycles, `g.V().repeat(__.both())` never runs out of traversers. Three tools keep
loops finite:

- `simple_path()` in the body drops every traverser that returns to an element it already
  visited: `g.V("a").repeat(__.out().simplePath()).emit().path()`.
- `times()`/`until()` bound or stop the loop.
- `repeat.max_loops` (10 000 by default) stops a loop without `times()` with
  `RepeatLimitExceeded`: `g.with("repeat.max_loops", 100)`. A loop with `times(n)` is never
  limited. `evaluationTimeout` bounds the wall-clock time of the whole traversal. See
  [Query Limits](query_limits.md).

## Breadth first and depth first

By default a loop runs **breadth first**: the body runs once per iteration on all traversers of
that iteration, and everything iteration `k` outputs comes before what iteration `k + 1` outputs.
Within an iteration, traversers output before the body (prefix `emit`/`until`) come before those
output after it.

`g.with("repeat.order", "dfs")` walks the loop **depth first** instead, in pre-order: a traverser
is followed to the end of its loop before its siblings. The walk uses an explicit stack, so deep
loops cannot overflow the call stack.

```
// a tree, children in the order out() returns them: r -> (b -> b1, a -> (a2, a1))
g.V("r").emit().repeat(__.out())                              // r, b, a, b1, a2, a1
g.with("repeat.order", "dfs").V("r").emit().repeat(__.out())  // r, b, b1, a, a2, a1
```

Both orders return the same results for a body without barrier steps; only the order differs.
TinkerPop does not define the output order of `repeat()` in the same way, so a query that depends
on order should sort its results.

## `dedup()` and other barriers in the body

A `dedup()` in the body keeps one seen-set for the whole loop, like in TinkerPop: an element met
again in a later iteration, or reached by another traverser, is dropped. `V().repeat(dedup()).times(2)`
passes every vertex in the first iteration and nothing in the second, and
`g.V("s").repeat(__.out().dedup()).emit()` is a visited set. Both orders (`repeat.order`) agree, and
merging the frontier stays invisible (survivors have bulk 1).

- Every `dedup()` step of the body has its own set; a nested `repeat()` starts a fresh set each time
  it runs.
- A `dedup()` inside a per-traverser child of the body (`where(...)`, `not(...)`, `local(...)`,
  `by(traversal)`, the `until()`/`emit()` conditions) starts fresh for every traverser, like
  TinkerPop's reset children. A `union()`/`choose()` child of the body is not reset.
- `simplePath()` is still the way to stop cycles.

Other barriers in the body (`limit()`, `range()`, `tail()`, `count()`) run once per iteration in
breadth-first order, like TinkerPop 3.8.2: its feature files expect
`g.V().repeat(both().limit(1)).times(2)` to return one traverser and
`repeat(union(constant('y').limit(1), identity())).times(2)` to return `y` twice, which a loop-wide
counter would not give. In depth-first order (`g.with("repeat.order", "dfs")`, which TinkerPop
does not have) such a barrier sees one traverser at a time, so a `limit()` there limits each
traverser's walk, not the iteration. Only `dedup()` keeps loop-wide state.

## Merging the frontier

In breadth-first order the loop merges equal traversers of the frontier (the same vertex reached by
different routes) into one traverser with a bulk, before `until()`/`emit()` are checked. The next
iteration then expands each distinct vertex once, so `repeat(__.out()).times(8).count()` on a dense
graph stays cheap. The results are the same multiset as without merging; only the order of
duplicates can change. Merging does not happen in depth-first order, when the plan records full
paths (`path()`, `simplePath()`), or when it mutates the graph. This is not a visited set:
unlike `dedup()`, every route still counts. A sack merge operator or `withBulk(false)` turns the
frontier merge off; write `repeat(__.out().barrier())` to merge per iteration. See
[Bulk and Barriers](bulk_and_barriers.md) and
turn it off with `g.with("bulk.merge", false)`.

## Paths and labels

Every step of the body extends the path of its traversers, so `path()` after a loop shows the
whole route. `as("x")` in the body labels the position of every iteration, and `select("x")`
returns the last one. The path requirement analysis treats the body as loop-carried; see
[Path Requirement Analysis](../optimizer/path_analysis.md).

## Profiling

`.profile()` shows the loop as written, with the body and the `until()`/`emit()` traversals as
children. Their counts and times add up over all iterations. The loop summary line reports the
number of body executions, the deepest iteration reached, and the average loop count of the
output traversers:

```text
repeat(__.out()).times(2)            1   1   2
[loops: 2, max_depth: 2, avg_loops: 2.0]
  \> out()                           2   4   5
```

## repeat() and inject()

`inject()` cannot be a **direct** step of a `repeat()` body, because the injected values would
enter again in every iteration. Graphersal raises `TraverserError::RepeatInject` when the
traversal executes (not when it is built). An `inject()` nested deeper, for example inside
`union(..)`, is allowed:

```rhai,graph=modern
g.V("1").repeat(__.inject(1)).times(2).toList()                         // error
g.V().repeat(__.union(__.identity(), __.inject("y"))).times(2)           // fine
```

A `repeat()` body that updates a sack (`sack(Operator.sum).by(..)`) keeps the sack of each traverser through every
iteration; see [Sack and Operators](sack_and_operators.md#updating-a-sack).
