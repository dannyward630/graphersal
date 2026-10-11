# Maps With Non-String Keys

In TinkerPop a map key is any value: `groupCount().by("age")` returns `{29=1, 27=1}`, and
`group().by(out())` has vertices as keys. Graphersal's logical value model holds such a map as
`ElementProperty::Map(Box<ValueMap>)`. A map whose keys are all strings stays an
`ElementProperty::Object`, so everything that exists today (JSON shape, Rhai object maps, schemas,
storage) keeps seeing `Object`.

This page describes the value type and where a query produces it. `group()`, `groupCount()`, `tree()`
and `unfold()` of such a map keep the type of their keys: `g.V().groupCount().by("age")` is
`{29: 1, 27: 1, 32: 1, 35: 1}` with `int64` keys, `next()[29]` reads `1`, a list key stays a list, a float
stays a float, a UUID stays a UUID, and a vertex key stays a vertex. A result whose keys are all strings
is unchanged (an `Object`, a Rhai object map, a plain JSON object). Builders for the Rust API are
`ElementProperty::from_entries` and `ValueMap::into_property`.

```rhai,graph=modern
g.V().groupCount().by("age").next()[29]                    // 1
g.V().group().by(__.out("created")).by("name").next()      // vertex keys
g.V().groupCount().by("age").unfold().select(Column.keys)  // 29, 27, 32, 35 as ints
```

## The rule

- `ElementProperty::from_entries(entries)` and `ValueMap::into_property()` are the only
  constructors that decide the variant: all keys strings gives `Object`, any other key gives `Map`.
  A `Map` always has at least one non-string key.
- A `Map` is **never storable** as a property, in every schema mode: `addV().property("k", map)` fails
  with `InvalidPropertyValue` (the help shows the string-key form and `to_json()`).
- `type_of(map)` is `object` (declared fields only describe string keys, so a `Map` carries none) and
  `GType.MAP` matches it.

## Key equality

Keys follow Java `equals`, as TinkerPop does:

| Pair | Same key? |
|---|---|
| `1` and `1.0` | no (`P.eq(1)` still matches `1.0`; key equality is stricter than predicate equality) |
| `NaN` and `NaN` | yes |
| `"1"` and `1` | no |
| `[1, 2]` and `[2, 1]` | no |
| two maps with the same entries in a different order | yes |

Map equality and hashing ignore entry order: two equal maps built in a different order hash alike.

## Output

- **JSON** (`to_json()`): a `Map` is written in the GraphSON 3 shape
  `{"@type": "g:Map", "@value": [k1, v1, k2, v2]}`, keys and values lowered recursively. A string-keyed
  map is always a plain JSON object. The tag is output only: reading JSON back never interprets
  `@type` (a `Map` cannot be stored, so it never needs to be read back).
- **Table and tree:** keys print through the display renderer (string unquoted, `29`, `2.0`, `null`,
  `true`, a vertex `v[1]`, a list as compact JSON). When two keys of one map print the same text
  (`"29"` and `29`) they stay separate columns, and the string key's header is shown quoted.
- **Console / `to_string`:** `{29: 1, "a": 2}`, strings quoted.

## In scripts

A `Map` reaches Rhai as a `Map` value (Rust type `ScriptMap`, public so hosts can downcast it):

| Rhai | Meaning |
|---|---|
| `m[29]`, `m["name"]` | read an entry by any key; a missing key is `()` |
| `m.name` | the same as `m["name"]` for a string key |
| `m.len()`, `m.is_empty()` / `m.isEmpty()` | size |
| `m.keys()`, `m.values()` | arrays in insertion order, keys keep their type |
| `m.contains(k)`, `m.containsKey(k)` / `m.contains_key(k)` | membership |
| `for pair in m` | `[key, value]` arrays |
| `m.to_map()` / `m.toMap()` | a Rhai object map; an error when a key is not a string |

`select(Column.keys)` and `unfold()` on a materialized `Map` return typed keys.
`json_path()` and `math()` address string keys only; a non-string entry is unreachable by path.

## Deviations from TinkerPop

- **`0.0` and `-0.0`** are the same key (Graphersal canonicalizes `-0.0` when a number is built;
  Java treats them as different keys).
- **`asString()` of a map** fails, for a `Map` as for a string-keyed map (see
  [TinkerPop Deviations](../for_developers/tinkerpop_deviations.md)).
- **A missing key** in a script reads as `()`, the script's `null`.
- **Vertex and edge keys are snapshots**, not live elements (see above).
- **Not storable:** a map with a non-string key cannot be a property value (a string-keyed map can), because
  stored maps are plain JSON-like objects and GraphML and schemas only describe string keys.

## Vertex and edge keys are snapshots

TinkerPop's key is the live element, compared by id. Graphersal materializes a vertex or edge key once,
when the result is built, into an `ElementProperty::Vertex`/`Edge` snapshot (its element map). Two
keys of one result that are the same vertex are equal (same snapshot), and a later graph change does not
alter an already returned key. Rendered as a table or tree a vertex key prints as `v[1]`; in JSON it is the
whole element. Entries of `unfold()` on a map are materialized as one-entry maps (`{29: 1}`), not as a
separate entry type.

## What stays a string

Where the key space is text by definition a key is still text: `T.label` and the label-keyed
`groupCount` pushdown, the `select("a", "b")` and `project(..)` names, and GraphML or schema data. A
`group()` key that merely looks like a number (`"29"`) stays a string and is a different key from `29`.
