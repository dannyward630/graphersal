# Step - index

`index()` maps a collection to the list of its `[element, position]` pairs, positions counted from
0 (TinkerPop `IndexStep` with its default list indexer).

```rhai,graph=modern
g.V().hasLabel("software").values("name").fold().index().to_list()   // [[["lop", 0], ["ripple", 1]]]
g.V("1").values("age").index().to_list()                             // [[[29, 0]]]
g.V("1").valueMap("name", "age").index().to_list()
// [[[{"name": ["marko"]}, 0], [{"age": [29]}, 1]]]
```

- A list and a path are indexed element by element.
- A map is indexed entry by entry; each entry is a one-entry map.
- Any other value (a number, a string, a vertex, `null`) is a one-element collection, so
  `g.V().hasLabel("software").index().unfold()` yields `[v[3], 0]` and `[v[5], 0]`.

`with(WithOptions.indexer, WithOptions.map)` (see [with(WithOptions..)](with_options.md)) builds a
`{position: element}` map instead:

```rhai,graph=modern
g.V().hasLabel("person").values("name").fold().order(Scope.local).
  index().with(WithOptions.indexer, WithOptions.map).next()
// {0: "josh", 1: "marko", 2: "peter", 3: "vadas"}
```

Rust: `GraphTraversalSource::index()`, `__::index()`, followed by
`with_option(WithOptions::Indexer, Some(WithOptions::Map))`.
