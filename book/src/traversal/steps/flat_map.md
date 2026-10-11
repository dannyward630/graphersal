# Step - flatMap

`flatMap(traversal)` (also `flat_map`) replaces each traverser by **every** result of a child
traversal run on it (TinkerPop `FlatMapStep`). A traverser for which the child yields nothing is
dropped.

```rhai,graph=modern
g.V("1").flatMap(__.out().values("name")).to_list()    // ["josh", "lop", "vadas"]
g.V().flatMap(__.out().out()).path().by("name").to_list()
// [["marko", "ripple"], ["marko", "lop"]]
g.V().local(__.out().out()).path().by("name").to_list()
// [["marko", "josh", "ripple"], ["marko", "josh", "lop"]]
```

The last two queries show the difference to `local()`: `flatMap()` hides the child's steps from
`path()` (each result is one new position after the incoming path), while `local()` continues the
child's own path.

- The child sees the incoming traverser with its path, sack and labels.
- Bulk is kept: the results of a merged traverser carry its bulk.

For only the first result, use [`map()`](map.md).

Rust: `GraphTraversalSource::flat_map(traversal)`, `__::flat_map(traversal)`.
