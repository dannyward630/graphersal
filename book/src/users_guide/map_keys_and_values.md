# Map Keys and Values

`group()`, `group_count()`, `project()`, `element_map()` and `select("a", "b")` produce maps.
This page covers how a query reads a map's keys and values generically: `unfold()` turns a map
into its entries, and `select(Column.keys)` / `select(Column.values)` (TinkerPop
`select(Column)`) reads keys or values.

## Quick reference

On the modern graph:

```rhai,graph=modern
g.V().group().by("name").by(__.out().count())                                 // one map
g.V().group().by("name").by(__.out().count()).unfold()                        // 6 entries: marko=3, vadas=0, ...
g.V().group().by("name").by(__.out().count()).unfold().select(Column.keys)    // "marko", "vadas", "lop", ...
g.V().group().by("name").by(__.out().count()).unfold().select(Column.values)  // 3, 0, 0, 2, 0, 1
g.V().group().by("name").by(__.out().count()).select(Column.keys)             // ONE list: ["marko", "vadas", ...]
g.V().group().by("name").by(__.out().count()).select(Column.keys).unfold()    // "marko", "vadas", "lop", ...
```

The order is the map's own order. `group()` keeps first-seen order. The index-backed
`group_count().by("label")` pushdown does not guarantee any order.

## `unfold()` of a map

`unfold()` emits one traverser per map entry. It still unrolls a list into its elements, and any
other value passes through unchanged. An entry is its own kind of stream value (TinkerPop's
`Map.Entry`). When an entry reaches a terminal (`to_list()`, `next()`, a table), it becomes a
single-key map, which is how the Gremlin console prints `marko=3`:

```rhai,graph=modern
g.V().groupCount().by("label").unfold().toList()
// #{"person": 4}
// #{"software": 2}
```

`unfold()` of an entry passes it through unchanged. An entry is not a collection.

## `select(Column.keys)` / `select(Column.values)`

| Input | `select(Column.keys)` | `select(Column.values)` |
|---|---|---|
| a map (`group()`, `group_count()`, `project()`, `element_map()`, a nested object property) | one list of all keys | one list of all values, types kept |
| an entry (from `unfold()` of a map) | the key | the value |
| a path (from `path()`) | one list holding the step labels of each position | not implemented: cast error |
| anything else | cast error | cast error |

**Map or entry?** The rule depends on the kind of value, not on how many keys it has. A map is
always a map, even with one key:

```rhai,graph=modern
g.V("1").groupCount().by("name").select(keys).toList()            // [["marko"]]: one list
g.V("1").groupCount().by("name").unfold().select(keys).toList()   // ["marko"]: the key itself
```

This follows TinkerPop, where `Map` and `Map.Entry` are different types. A rule based on the
number of keys ("a single-key map is an entry") would make one-group `group()` results behave
differently from every other `group()` result. Only `unfold()` creates entries. A single-key map
that went through a terminal and came back into a query (for example through `inject()`) is a
map again, and `values(k)`, `select(k)` and `has(k, v)` read its keys like those of a `project()`
result:

```
let m = #{name: "x", age: 3};
g.inject(m).values("name").toList()        // ["x"]
g.inject(m).select("name", "age").toList() // [#{age: 3, name: "x"}]
g.inject(m).has("age", P.gt(2)).count()    // 1
```

**A path.** `select(Column.keys)` of a path lists the step labels of its positions, one list per
position in position order (TinkerPop's `Path.labels()`); an unlabelled position is an empty list,
and the labels of one position keep the order of their `as()` calls:

```rhai,graph=modern
g.V("1").as("a", "b").out().as("c").path().select(Column.keys).toList()   // [[["a", "b"], ["c"]], ...]
```

`select(Column.values)` of a path lists its objects in position order (TinkerPop's
`Path.objects()`; after `path().by(..)` the modulated values), so keys and values line up by index:

```rhai,graph=modern
g.V("1").out("knows").path().select(Column.values).toList()   // [[v[1], v[4]], [v[1], v[2]]]
g.V("1").out("knows").path().by("name").select(Column.values) // [["marko", "josh"], ["marko", "vadas"]]
```

`select(Column...)` has nothing to do with label-based `select("a")`. It reads the current value
only, never the traverser path. So an `as()` label before it records nothing, and `.profile()`
shows the step as `select(Column.keys)`.

## Spellings

| Notation | Keys | Values |
|---|---|---|
| Gremlin (dot) | `select(Column.keys)` | `select(Column.values)` |
| Gremlin console static import | `select(keys)` | `select(values)` |
| Rhai static module | `select(Column::keys)` | `select(Column::values)` |
| Rust API | `.select(Column::Keys)` | `.select(Column::Values)` |

The bare tokens `keys` and `values` are script constants. They do not interfere with the
`values("age")` step, because Rhai keeps variables and methods in separate namespaces.
`Column`, `keys` and `values` are reserved names and cannot be used as script parameters.

There is no `keys()` step. It does not exist in TinkerPop either. `g.V().group().keys()` fails
with Rhai's own `Function not found: keys` error. Use `select(Column.keys)` instead.

## Errors

A map is not a graph element. So `properties()`, `key()`, `value()`, `id()` and `out()` on a map
or an entry stay errors, as in TinkerPop. The help text shows how to read the map instead:

```
g.V().group().by("name").by(__.out().count()).properties().key()
// Cast exception: expected type [vertex or edge], got type 'object'
// Help: ... Read its keys or values with unfold().select(Column.keys) / unfold().select(Column.values), ...
```

`select(Column.keys)` on something that is not a map fails with
`expected type [map or entry]`. The help names the steps that produce maps, and it names
`unfold()`.
