# Properties Named id or label

Every vertex and edge has an **id** and a **label**. They are not properties: `id()` and
`label()` read them, and the tokens `T.id` and `T.label` name them. A property may still be
called `id` or `label` (or `labels`); like TinkerPop, Graphersal accepts these keys and keeps such
a property apart from the element's own id and label.

The rule in one line: **a string names a property, a token names the element's id or label.**

## Writing

`property()`, `property_json()`, `add_v()`/`add_e()` with properties and `mergeV()`/`mergeE()`
maps store a property named `id` or `label` like any other. It changes neither the element's
id nor its label:

```rhai,graph=modern
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").label().next()       // "person"
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").values("label").next() // "CUSTOM"
```

To set the id or the label, use `add_v(label)`, `add_e(label)`, `set_label()` (edges),
`add_label()`/`drop_label()` (vertices) or the `"T.id"`/`"T.label"` keys of a `mergeV()` map.

## Reading

| Reads the property named `id` / `label` | Reads the element's id / label |
|---|---|
| `values("id")`, `values("label")`, `values()` | `id()`, `label()`, `labels()` |
| `properties("label")`, `properties()` | |
| `has("label", "x")` | `has_id(..)`, `has_label(..)` |
| `by("id")`, `by("label")` | `by(T.id)`, `by(T.label)` |
| `select("id")`, `select("label")` on a map | `select(T.id)`, `select(T.label)` on an `element_map()` map |

```rhai,graph=modern
g.add_v("person").property("label", "CUSTOM").values().to_list()                            // ["CUSTOM"]
g.add_v("person").property("label", "CUSTOM").project("p", "t").by("label").by(T.label).next() // #{"p": "CUSTOM", "t": "person"}
g.V("1").project("p").by("label").to_list()                                                 // [#{}]: marko has no property label
g.V().group_count().by(T.label).next()                                                      // #{"person": 4, "software": 2}
```

`by("label")` is unproductive on an element without a property named `label`, like any other
missing property (`group_count().by("label")` counts nothing). Use `by(T.label)` for the label.

## element_map() and value_map(true)

As in TinkerPop, the map `element_map()` builds holds the id and the label under the tokens
`T.id` and `T.label`, and an edge's endpoints under `Direction.OUT` and `Direction.IN` (each a
map of the endpoint's `T.id` and `T.label`). The properties follow under their names, so a
property named `label` is an entry of its own. `value_map(true)` holds `T.id` and `T.label` the
same way.

```rhai,graph=modern
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").element_map().count(Scope.local).next() // 4
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").element_map().select("label").next()    // "CUSTOM"
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").element_map().select(T.label).next()    // "person"
g.V("1").element_map().select("label").to_list()                                                               // []: no property label
g.E("0").element_map().select(Direction.OUT).next()                                                            // #{"id": "1", "label": "person"}
```

`select(T.id)`, `select(T.label)`, `select(Direction.OUT)` and `select(Direction.IN)` are a
Graphersal extension (TinkerPop's `select()` takes no token): each takes one token alone and
reads that entry of the map; a value without it is filtered. `by(T.id)`/`by(T.label)` read the
same entries (`element_map().order().by(T.label)`).

### When the map leaves the engine

Outside the engine (a script value, a table, JSON in the playground, Python, MCP) a map has string
keys only. The tokens are then written `"id"`, `"label"`, `"OUT"` and `"IN"`, the way TinkerPop's
untyped JSON writes them:

```rhai,graph=modern
g.E("0").element_map().next()
// #{"IN": #{"id": "2", "label": "person"}, "OUT": #{"id": "1", "label": "person"}, "id": "0", "label": "knows", "weight": 0.5}
```

When a property has the same name, **the property wins**: the written map shows the property's
value under `"id"` or `"label"`, and the element's own value is not in it. Naming a property `id`
or `label` is taken as the wish to see it there. To get the element's id or label in that case,
read it with a token (`select(T.label)`, `project("l").by(T.label)`) or with `id()`/`label()`, or
give the property another name.

```rhai,graph=modern
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").element_map().next()
// #{"id": "7", "label": "CUSTOM", "name": "Bill"}: the property, not the label person
g.add_v("person").property("name", "Bill").property("label", "CUSTOM").element_map().unfold().to_list()
// [#{"id": "7"}, #{"label": "person"}, #{"name": "Bill"}, #{"label": "CUSTOM"}]: both entries
```

Steps that work on the map keep the tokens apart from the names: `unfold()`, `count(Scope.local)`,
`order(Scope.local)`, `select()`. Only an entry key taken out of the map as a value
(`select(Column.keys)`, an unfolded entry's key) is the written string `"label"`.

## Vertices and edges returned as values

A vertex or an edge returned by a terminal (`to_list()`, `next()`) is written as one map of its
id, label and properties today. That map still mixes the two kinds: for a vertex a property named
`id` replaces the id, while the label stays the element's; for an edge the id and the label stay
the element's. Read `id()`, `label()` or `element_map()` when the difference matters. Vertex and
edge objects that keep id, label and properties apart are planned.
