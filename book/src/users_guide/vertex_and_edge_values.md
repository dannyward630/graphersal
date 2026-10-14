# Vertices and Edges as Values

A traversal yields vertices and edges as live references into the graph. When a terminal returns
them (`to_list()`, `next()`, `execute()`), each becomes a **value**: a detached copy of the element,
as in TinkerPop. The copy keeps the element's own id and label apart from its properties, so a
property named `id` or `label` never takes their place (see
[Properties Named id or label](id_and_label_keys.md)).

| | Vertex | Edge |
|---|---|---|
| Own fields | id, label (the primary one), labels (the full set) | id, label, outgoing vertex, incoming vertex (each an id and a label) |
| Properties | a snapshot of every property | a snapshot of every property |
| Prints as | `v[1]` | `e[0][1-knows->2]` |
| Equal to | another vertex of the same id | another edge of the same id |

Property values may be nested objects and lists, on edges exactly as on vertices.

## In a script

A vertex is a `Vertex`, an edge an `Edge`. The getters read the element; the index reads the
properties:

```rhai,graph=modern
g.V("1").next().id                       // "1"
g.V("1").next().label                    // "person"
g.V("1").next().labels                   // ["person"]
g.V("1").next()["name"]                  // "marko"
g.V("1").next().properties               // #{"age": 29, "name": "marko"}
g.E("0").next().out_v.id                 // "1"  (also outV; in_v / inV)
g.E("0").next()["weight"]                // 0.5
```

`v.name` works too, for a name that is not one of the getters (`id`, `label`, `labels`,
`properties`, and on an edge `out_v`/`outV`/`in_v`/`inV`): it falls back to the index. So
`v.label` is always the label and `v["label"]` always the property:

```rhai,graph=modern
g.addV("person").property("label", "CUSTOM").next().label        // "person"
g.addV("person").property("label", "CUSTOM").next()["label"]     // "CUSTOM"
```

A missing property reads as `()`. An edge's endpoints (`e.out_v`, `e.in_v`) are `VertexRef`s: id
and label only, printed `v[1]`.

### Passing a value back

A vertex or an edge value stands for its element when it goes back into a query, as a detached
element does in TinkerPop: `g.V(v)`, `has_id(v)` and `g.inject(v)` reach the stored element of its
id.

```rhai,graph=modern,script
let v = g.V("1").next();
g.V(v).values("name").next();            // "marko"
g.inject(v).out().count().next()         // 3
```

A value is a snapshot: it does not change when the graph does, and reading it reads no graph
data. To read the current state, pass it back into a query; that query asks `Read` on the
element's data like `V()` does ([Permissions](permissions.md)).

## In the results: tables and JSON

A table shows a vertex as the columns `id`, `label` and one column per property; an edge as `id`,
`label`, `outV`, `inV` and its properties. A property named like one of those columns gets its own
column headed `<name> (property)`:

```text
$ graphersal -e 'g.addV("person").property("name", "Bill").property("label", "CUSTOM")'
╭────┬────────┬──────────────────┬──────╮
│ id │ label  │ label (property) │ name │
├────┼────────┼──────────────────┼──────┤
│ 7  │ person │ CUSTOM           │ Bill │
╰────┴────────┴──────────────────┴──────╯
```

JSON (the playground, the dev server, MCP, `to_json()`) writes the untyped GraphSON 3 shape:

```json
{"id": "1", "label": "person", "properties": {"name": "marko", "age": 29}}
{"id": "0", "label": "knows", "inVLabel": "person", "outVLabel": "person",
 "inV": "2", "outV": "1", "properties": {"weight": 0.5}}
```

A vertex with more than one label adds `"labels": [..]` after `"label"` (a Graphersal extension:
TinkerPop has one label per vertex). `"properties"` holds each value directly: Graphersal keeps one
value per key and has no property ids, so there is no list of vertex-property objects as in
GraphSON (`{"name": [{"id": .., "value": "marko"}]}`).

## In Python

The same values are `graphersal.Vertex` and `graphersal.Edge`, like gremlin-python's:

```python
v = graph.query('g.V("1").next()')
v.id, v.label, v.labels      # ('1', 'person', ['person'])
v["name"], v.properties      # ('marko', {'name': 'marko', 'age': 29})
e = graph.query('g.E("0").next()')
e.out_v, e.in_v.id, e["weight"]   # (v[1], '2', 0.5)
graph.query('g.V(v).values("name").toList()', params={"v": v})   # ['marko']
```

`v["missing"]` raises `KeyError`. A `Vertex`/`Edge` can be a parameter; it stands for its element.

## In Rust

A terminal returns `ElementProperty::Vertex(Box<VertexValue>)` and
`ElementProperty::Edge(Box<EdgeValue>)`: `id()`, `label()`, `labels()` (vertex), `out_v()` /
`in_v()` (edge, each a `VertexRef`) and `properties()`.
