# Upserts: mergeV and mergeE

An *upsert* finds an element and creates it only when it does not exist yet. Graphersal follows
TinkerPop's `mergeV()` / `mergeE()` steps: one map describes the element to look for, and
`option(Merge.onCreate, ..)` / `option(Merge.onMatch, ..)` say what to write in either case.

Every example on this page was run with `graphersal` (`--graph empty` where noted, otherwise the
default modern graph); the line after `//` is its real output.

## Quick reference

```rhai
// --graph empty: the first call creates, the second one matches and updates
g.mergeV(#{"T.label": "person", "name": "marko"}).option(Merge.onCreate, #{"age": 29}).elementMap().toList()
// #{"age": 29, "id": "1", "label": "person", "name": "marko"}
g.mergeV(#{"T.label": "person", "name": "marko"}).option(Merge.onCreate, #{"age": 29}).option(Merge.onMatch, #{"age": 30}).elementMap().toList()
// #{"age": 30, "id": "1", "label": "person", "name": "marko"}

// modern graph
g.mergeV(#{"T.id": "1"}).values("name").toList()                       // "marko"
g.mergeE(#{"T.label": "knows", "Direction.OUT": "1", "Direction.IN": "2"}).elementMap().toList()
// #{"IN": #{"id": "2", "label": "person"}, "OUT": #{"id": "1", "label": "person"}, "id": "0", "label": "knows", "weight": 0.5}: the existing edge
g.mergeE(#{"T.label": "knows", "Direction.OUT": "2", "Direction.IN": "1"}).option(Merge.onCreate, #{"weight": 0.1}).elementMap().toList()
// #{"IN": #{"id": "1", "label": "person"}, "OUT": #{"id": "2", "label": "person"}, "id": "6", "label": "knows", "weight": 0.1}: created
```

In Rust the steps are `merge_v(map)`, `merge_v_by(traversal)`, `merge_e(map)`, `merge_e_by(traversal)`
and `option_merge(Merge::OnCreate, map)`, `option_merge_by(Merge::OnMatch, traversal)`,
`option_merge_with(Merge::OnCreate, map, Cardinality::Single)` on `GraphTraversalSource`,
`AnonymousTraversal` and `__`. A map is an `ElementProperty::Object` with the same string keys as
in the DSL.

## Map keys: properties and reserved token keys

Rhai map keys are strings, so a Gremlin token such as `T.label` cannot be a key. Graphersal
reserves four string keys spelled exactly like the token:

| Gremlin key | DSL key | Meaning |
|---|---|---|
| `T.id` | `"T.id"` | the element id |
| `T.label` | `"T.label"` | the label |
| `Direction.OUT` (`Direction.from`) | `"Direction.OUT"` | the out-vertex (`mergeE()` only) |
| `Direction.IN` (`Direction.to`) | `"Direction.IN"` | the in-vertex (`mergeE()` only) |

Every other key is a property name. The engine reads these keys in every map a merge step
receives, whether it is written as a literal, injected, taken from `select()` or built by
`project()`, so a map keeps its meaning when it travels through a traversal:

```rhai,graph=empty
// --graph empty
g.inject(#{"T.label": "person", "name": "marko"}, #{"T.label": "person", "name": "stephen"}).mergeV().elementMap().toList()
// #{"id": "1", "label": "person", "name": "marko"}
// #{"id": "2", "label": "person", "name": "stephen"}
```

**Why not `"id"` and `"label"`?** Those are ordinary property names: `elementMap()` uses them for
the id and label entries, but a vertex may also carry a property called `id`, which then shadows
the entry, and `[id: 1]` in Gremlin is the property `id` as well:

```rhai,graph=empty
// --graph empty
g.addV("person").property("id", 5).property("label", "x").elementMap().toList()
// #{"id": 5, "label": "x"}
```

So `#{"id": "1"}` in a merge map searches for a *property* named `id`. Note the consequence for
round trips: `elementMap()` returns `"id"`/`"label"`, so its result cannot be passed to `mergeV()`
unchanged (TinkerPop's `elementMap()` returns `T.id`/`T.label` keys; see the backlog).

**Reserved prefixes.** In a merge map and in `property(map)`, a key that starts with `T.` or
`Direction.` but is not one of the four keys above, or a key that starts with `~`, is an error,
never a silent property name:

```rhai
// --graph empty
g.mergeV(#{"~id": 1}).toList()
// Error: Property key can not be a hidden key: ~id (in the merge() map of mergeV())
// Help: Write the id with the reserved key "T.id" instead of "~id", for example g.merge_v(#{"T.id": "1"}). ...
g.mergeV(#{"T.key": 1}).toList()
// Error: unsupported token key "T.key": a merge map takes only the token keys "T.id", "T.label", "Direction.OUT" and "Direction.IN" (in the merge() map of mergeV())
```

A property literally named `"T.label"` can still be written one key at a time:

```rhai,graph=empty
// --graph empty
g.addV("person").property("T.label", "x").valueMap().toList()     // #{"T.label": ["x"]}
```

## mergeV

`mergeV(map)` searches for vertices whose id equals `"T.id"` (if given), whose label set contains
`"T.label"` (if given) and whose every other entry equals the stored property, with the same
equality as `has(key, value)`. Every match is emitted. `null` (`()` in Rhai) and `#{}` match every
vertex. `mergeV(traversal)` takes the map from the first result of the traversal, and `mergeV()`
with no argument uses the incoming traverser itself as the map (the `inject()` example above).

- **onMatch** runs on each match. Its map is written with `property()` semantics; `null` / `#{}`
  change nothing. An onMatch traversal receives the *matched* vertex and must yield a map.
  `"T.id"`/`"T.label"` are not allowed in onMatch.
- **onCreate** runs only when nothing matched. The new vertex gets the merge map **plus** the
  onCreate map (inheritance). An onCreate traversal receives the incoming traverser.
- **Override rule.** A key present in both maps must have the same value; onCreate can only add
  keys. With two constant maps the check runs before execution, even when a match exists:

```rhai
// --graph empty
g.mergeV(#{"T.label": "person", "name": "marko"}).option(Merge.onCreate, #{"T.label": "dog"}).toList()
// Error: option(onCreate) cannot override values from merge() argument: T.label is "person" in mergeV() but "dog" in option(onCreate)
// Help: option(Merge.onCreate, ..) can only add keys to the merge() map: a key present in both must have the same value there. ...
```

```rhai,graph=empty
// --graph empty: inheritance, the label comes from onCreate
g.mergeV(#{"name": "marko"}).option(Merge.onCreate, #{"T.label": "person", "age": 29}).elementMap().toList()
// #{"age": 29, "id": "1", "label": "person", "name": "marko"}
g.mergeV(()).option(Merge.onCreate, #{"T.label": "person", "name": "marko"}).elementMap().toList()
// #{"id": "1", "label": "person", "name": "marko"}

// modern graph: an onMatch traversal
g.mergeV(#{"name": "marko"}).option(Merge.onMatch, __.project("visits").by(__.constant(1))).valueMap("name", "visits").toList()
// #{"name": ["marko"], "visits": [1]}
```

**Start step or mid-traversal.** As the first step, `mergeV()` runs once. In the middle of a
traversal it runs once per incoming traverser and emits every match each time; an empty stream
merges nothing:

```rhai,graph=modern
g.mergeV(#{}).count().next()                                         // 6
g.V().hasLabel("software").mergeV(#{}).count().next()                // 12: 2 traversers × 6 vertices
g.V().hasLabel("software").mergeV(#{"T.label": "person", "name": "vadas"}).values("name").toList()
// "vadas"
// "vadas"
```

Graphersal keeps its own defaults where they differ from TinkerPop: a vertex created without
`"T.label"` is unlabeled (as with `addV()`), ids are strings, and a `null` property value is
stored.

## mergeE

`mergeE()` takes the same forms and options, plus the keys `"Direction.OUT"` and `"Direction.IN"`.
Their value is an existing vertex id, a vertex, or the placeholder `Merge.outV` / `Merge.inV`,
which `option(Merge.outV, ..)` / `option(Merge.inV, ..)` resolve **before** the search:

```rhai,graph=modern
g.V("1").as("a").V("3").as("b").mergeE(#{"T.label": "likes", "Direction.OUT": Merge.outV, "Direction.IN": Merge.inV}).option(Merge.outV, __.select("a")).option(Merge.inV, __.select("b")).project("label", "out", "in").by(T.label).by(__.outV().values("name")).by(__.inV().values("name")).toList()
// #{"in": "lop", "label": "likes", "out": "marko"}
g.mergeE(#{"T.label": "knows", "Direction.OUT": "2", "Direction.IN": Merge.inV}).option(Merge.inV, #{"T.label": "person", "name": "josh"}).elementMap().toList()
// #{"id": "6", "label": "knows"}
```

An `option(Merge.outV|inV, map)` map is a vertex search (`"T.id"`, `"T.label"`, properties) that
must find exactly one vertex; an option traversal receives the incoming traverser and must yield a
vertex or a vertex id.

- **Search.** Edges with the given id, label, out-vertex, in-vertex and properties. The incoming
  vertex is *not* an implicit endpoint: `g.V().hasLabel("person").mergeE(#{"T.label": "knows"}).count().next()`
  returns `8` (4 traversers × 2 `knows` edges).
- **Create.** Needs both vertices after onCreate inheritance:

```rhai
// --graph empty
g.mergeE(#{"T.label": "knows"}).toList()
// Caused by: Out Vertex not specified
// Help: An edge merge_e() creates needs both vertices: give "Direction.OUT" and "Direction.IN" an existing vertex id, ...

// modern graph
g.mergeE(#{"T.label": "knows", "Direction.OUT": "1", "Direction.IN": "100"}).toList()
// Caused by: Vertex id could not be resolved from mergeE: 100
```

- The override rule covers `"Direction.OUT"`/`"Direction.IN"` too. `"Direction.BOTH"` is rejected
  (an edge has two named ends), and so is every `Cardinality`: edge properties have none.
  `"T.id"` sets the id of a created edge.

## Cardinality

Graphersal stores **one value per property key**. Of TinkerPop's three cardinalities only
`Cardinality.single` (replace the current value) is supported:

```rhai
g.V("1").property(Cardinality.single, "age", 30).values("age").toList()   // 30
g.V("1").property(Cardinality.list, "age", 30).toList()
// Caused by: Cardinality.list is not supported: Graphersal stores one value per property key (property() would write a second value)
// Help: ... To keep several values, store them as one array value: g.v("1").property("tags", ["a", "b"]).
g.V("1").property("tags", ["a", "b"]).values("tags").toList()             // ["a", "b"]
```

`Cardinality.list` and `Cardinality.set` are accepted as tokens and fail with
`TraverserError::UnsupportedCardinality` only **when a value would be written**, because that is
where a second value would silently replace the first. A call that writes nothing succeeds, as in
TinkerPop:

```rhai,graph=empty
// --graph empty
g.addV("person").property(Cardinality.set, #{}).count().next()           // 1
```

The token works everywhere TinkerPop takes it: as the first argument of `property()`, as the
default of a map (`property(Cardinality.single, map)`,
`option(Merge.onCreate, map, Cardinality.single)`) and per value with `Cardinality.single(v)`:

```rhai,graph=modern
g.mergeV(#{"name": "marko"}).option(Merge.onCreate, #{"age": Cardinality.single(29)}, Cardinality.single).values("age").toList()
// 29
```

On an edge any cardinality is an error ("Property cardinality can only be set for a Vertex").

### property(map), property(T.id), property(T.label)

`property(map)` writes one property per entry; `null` / `#{}` write nothing. Its keys are plain
property names (the reserved prefixes above are errors). The id and label of a vertex that
`addV()` is creating are set with `property(T.id, v)` / `property(T.label, v)`, anywhere among the
`property()` calls that follow it:

```rhai
// --graph empty
g.addV("person").property(#{"name": "marko", "age": 29}).valueMap().toList()
// #{"age": [29], "name": ["marko"]}
g.addV("person").property(T.id, "p1").property("name", "marko").elementMap().toList()
// #{"id": "p1", "label": "person", "name": "marko"}
g.addV("person").property(T.label, "animal").label().toList()             // "animal"

// modern graph: the id of an existing element cannot change
g.V("1").property(T.id, "p1").toList()
// Caused by: property(T.id, ..) can only set the id of an element addV()/addE() is creating
```

## Direction, to/toE/toV and has(label, key, value)

`Direction.OUT`, `Direction.IN` and `Direction.BOTH` (aliases `Direction.from` = `OUT`,
`Direction.to` = `IN`) are the merge-map keys above and the argument of the generic navigation
steps: `to(Direction, labels..)` is `out/in/both(labels..)`, `toE(Direction, labels..)` is
`outE/inE/bothE`, and `toV(Direction)` is `outV/inV/bothV`. In Rust: `to_direction`, `to_e`,
`to_v`.

```rhai,graph=modern
g.V("1").to(Direction.OUT, "knows").values("name").toList()              // "josh", "vadas"
g.V("1").toE(Direction.OUT, "created").toV(Direction.IN).values("name").toList()   // "lop"
g.V("4").to(Direction.BOTH).values("name").toList()                      // "ripple", "lop", "marko"
```

The three-argument `has(label, key, value)` (and `has(label, key, P)`) combines `hasLabel()` and
`has()`; in Rust `has_labeled` / `has_labeled_p`:

```rhai,graph=modern
g.V().has("person", "name", "marko").values("age").toList()              // 29
g.V().has("person", "age", P.gt(30)).values("name").toList()             // "josh", "peter"
```

## Confirming the lookup with profile()

A merge step looks elements up through the storage indexes, not through a nested traversal.
`.profile()` names the strategy after the step: `id` (`"T.id"`), `label` (`"T.label"`),
`out-edges` / `in-edges` (`mergeE()` with a known vertex), `scan` (only property keys) or
`dynamic` (the map comes from a traversal). Add the missing key to turn a `scan` into an index
lookup.

```rhai,graph=modern
g.mergeV(#{"T.id": "1"}).profile()
// merge_v(#{"T.id": "1"}) [lookup: id]                            1       0       1  360.416µs    40.60
g.mergeV(#{"T.label": "person"}).profile()
// merge_v(#{"T.label": "person"}) [lookup: label]                 1       0       4    1.375µs    20.75
g.mergeV(#{"name": "marko"}).profile()
// merge_v(#{name: "marko"}) [lookup: scan]                        1       0       1    1.125µs    27.83
g.mergeE(#{"Direction.OUT": "1"}).profile()
// merge_e(#{"Direction.OUT": "1"}) [lookup: out-edges]            1       0       3   12.250µs    73.13
```

The three-argument `has()` folds its label into the source step:

```rhai,graph=modern
g.V().has("person", "name", "marko").profile()
// v(labels: ["person"])                                           1       0       4    3.833µs    12.92
// has("name", "marko")                                            1       4       1   11.166µs    37.64
// Optimizer rules applied: source_filter_pushdown
```

Two limits of the profile output: a long step name is cut, so the `[lookup: ..]` suffix of a large
map may not be visible, and the option traversals of one merge step share one nested row.

## Not supported yet

- `Cardinality.list` / `Cardinality.set` writes and meta-properties
  (`property(key, value, metaKey, metaValue)`): Graphersal has one value per key.
- Removing a property by writing `null` (TinkerPop's `@DisallowNullPropertyValues` mode):
  Graphersal stores the `null`. This is deliberate: `null` is a real value of the property model, and
  the four `@AllowNullPropertyValues` scenarios, which expect it to be stored, pass; removing on `null`
  would trade them for the three `@DisallowNullPropertyValues` ones. Settling it would need a
  graph-level null mode (not scheduled). Remove a property explicitly with
  `remove_property()` instead, see [Removing Properties](removing_properties.md).
- `PartitionStrategy` and the other traversal strategies.
- Multi-label merge (`"T.label"` as a list).
