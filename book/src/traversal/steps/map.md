# Step - map

`map(traversal)` replaces each traverser's value by the **first** result of a child traversal run
on it (TinkerPop `TraversalMapStep`). A traverser for which the child yields nothing is dropped.

```rhai,graph=modern
g.V("1").out().map(__.values("name")).to_list()                       // ["josh", "lop", "vadas"]
g.V().map(__.out("created").values("name")).to_list()                 // ["lop", "ripple", "lop"]
g.V().as("a").out("created").map(__.select("a").values("name")).to_list()
// ["marko", "josh", "josh", "peter"]
```

- The child sees the incoming traverser with its path, sack and labels, so `select("a")` inside
  the child reads an `as("a")` before `map()`.
- The result is **one** new path position: the child's own steps do not show up in a later
  `path()`.
- `null` is a result like any other: `g.V().map(__.constant(()))` yields six `null`s.
- Bulk is kept: a merged traverser maps once and keeps its bulk.

For every result of the child instead of the first, use [`flatMap()`](flat_map.md). `withPath()`
is accepted on `g` for Gremlin compatibility and changes nothing: Graphersal records the path
positions a query reads on its own. Like `withSack()`, it is a source step: inside a child traversal
(`__.withPath()`) it fails before execution with `SourceStepInChild`, as `__.withSack(1)` does.

Rust: `GraphTraversalSource::map(traversal)`, `__::map(traversal)`.
