# Step - fail

`fail()` and `fail(message)` stop the whole traversal with an error as soon as a traverser reaches
the step (TinkerPop `FailStep`). Use it to assert that a branch is never taken.

```
g.V().fail("msg").to_list()
// Error: ... Caused by: msg (fail() reached by v[1])

g.V().coalesce(__.values("name"), __.fail("no name")).count().next()   // 6: fail() is never reached
g.V().union(__.out(), __.fail()).to_list()
// Error: ... Caused by: fail() step triggered (fail() reached by v[1])
```

- The error is `TraverserError::Fail`, with the message (`fail() step triggered` without one) and
  a short rendering of the value that reached the step (`v[1]`, `e[7]`, or the JSON text of a
  value). Its `help()` shows how to guard the step with `coalesce()` or `choose()`.
- Like every failing traversal, a traversal that changed the graph before `fail()` is rolled back
  as a whole (see [Transactions](../../users_guide/transactions.md)).
- `fail()` is a side-effect step: the optimizer never moves a filter across it, so only the
  traversers that reach it as written fail the query.

Rust: `GraphTraversalSource::fail(None | Some(message))`, `__::fail(..)`.
