# Step - discard

`discard()` filters out every traverser. It is TinkerPop 3.8's name for the step that older
versions (and Graphersal before 0.1.0) called `none()`; `none(P)` is now a
[list predicate](none.md).

```
g.V().hasLabel("person").discard().to_list()          // []
g.V().discard().fold().to_list()                      // [[]]: fold() still emits its seed
g.V().project("x").by(__.coalesce(__.values("age").is(P.gt(29)), __.discard())).select("x").to_list()
// [32, 35]
```

The last query uses `discard()` as the "drop it" branch of `coalesce()`: a vertex younger than 30
(or without an age) produces nothing, so `project()`'s `by()` is unproductive and the vertex is
dropped. `discard()` is also the usual `option(Pick.none, __.discard())` of `choose()`.

Rust: `GraphTraversalSource::discard()`, `__::discard()`.
