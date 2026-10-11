# tree("key") as a Side Effect

This page lists where `tree("key")` differs from Apache TinkerPop 3.8.2. Everything not listed
follows `Tree.feature`.

`tree()` without a key is a reducing barrier that emits the tree. `tree("a")` is a side effect: every
traverser passes through unchanged and its path is added to the tree stored under `a`. A second
`tree("a")` adds to the same tree, `cap("a")` and `select("a")` read it, and `by()` works on both forms:

```rhai,graph=modern
g.V().out().tree("a").select("a").count(Scope.local)   // six rows of 3
g.V("1").out().out().tree("a").by("name").both().both().cap("a")
```

## Deviations

- **A later step does not see a half-built tree.** The engine runs each step over the whole batch
  before the next one starts, so `g.V().out().order().by("name").local(__.tree("a")).select("a")` gives
  the finished tree on every row, where TinkerPop's streaming execution gives the growing tree
  (`1, 1, 1, 2, 2, 3`). Everything inside one `local()` child is evaluated per traverser and is
  progressive: `g.V().out().local(__.tree("a").select("a").count(Scope.local))` matches TinkerPop. Reading
  the tree before the traversers are done needs a streaming executor.
- **Bulk is ignored**, as in TinkerPop: a path reached many times is one branch.
- **A key that already holds something other than a tree is an error** (`aggregate("a").tree("a")`).
  TinkerPop fails with its own cast error.
- **An empty stream still leaves an empty tree** that `cap("a")` returns as `{}`.
