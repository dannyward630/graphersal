# UUID Values

A UUID is a value type of its own in Graphersal, not a string that happens to look like one. This
page covers how UUIDs are stored, how a query writes one, and how they compare.

## Storage model

At runtime a UUID is `ElementProperty::Uuid(u128)`: 16 bytes stored inline, with no heap
allocation. A 36-character string would not fit `CompactString`'s inline buffer. Equality and
ordering are single integer comparisons.

A UUID stays a UUID end to end, including in the Rhai DSL. A value that `next()` or `to_list()`
returns to a script has the Rhai type `Uuid` (`type_of(x) == "Uuid"`), so you can pass it back
into a filter and it still matches:

```
let x = g.v().values("uid").next();   // a Uuid, not a string
g.v().has("uid", x).count()            // 1
```

It prints as its canonical string (`print(x)`, `` `${x}` ``, `x.to_string()`, `x.as_string()`).

Graphersal never infers a type from a string's content. A string that looks like a UUID stays a
string until a query or a schema declaration converts it explicitly (see below).

## Literals

| Notation | Literal |
|---|---|
| Java Gremlin | `UUID.fromString("550e8400-e29b-41d4-a716-446655440000")` |
| snake_case (Rhai, dot) | `UUID.from_string("550e8400-e29b-41d4-a716-446655440000")` |
| Rhai static module | `UUID::fromString("…")`, `UUID::from_string("…")` |
| Rust API | `ElementProperty::uuid_from_string("…")?` |

Only the canonical form parses: 8-4-4-4-12 **lowercase** hex digits, no braces. An uppercase or
malformed literal is a cast error, and the error's help shows the canonical form:

```
UUID.fromString("550E8400-E29B-41D4-A716-446655440000")   // Cast exception: expected type [uuid]
```

`UUID()` with no argument is a random UUID (version 4), the Gremlin grammar's `UUID()` and Java's
`UUID.randomUUID()`: `g.inject(UUID())`. It draws from the operating system's entropy; the
`random.seed` option does not apply to it.

`UUID` is a reserved name in the script scope, so a script parameter cannot be called `UUID`.

To convert a stream of canonical strings, use `cast(GType::UUID)`. To go the other way, use
`as_string()`:

```
g.inject("550e8400-e29b-41d4-a716-446655440000").cast(GType::UUID)
g.v().values("uid").as_string()
```

## Strict equality

A `Uuid` never equals a `String`. This is the same as TinkerGraph, where `UUID.equals(String)` is
false. A filter never raises an error on a type difference; it just does not match:

```
g.v().has("uid", "550e8400-e29b-41d4-a716-446655440000")                   // finds nothing
g.v().has("uid", UUID.fromString("550e8400-e29b-41d4-a716-446655440000"))  // finds the vertex
```

The same holds for `P.eq`, `P.neq`, `P.within`, `P.without` and `is(..)`. Each of them accepts a
UUID literal. In the Rust API the predicate value is `PValue::Uuid(bits)`.

Reads never coerce, even when a schema declares the field `uuid`. If a `has(..)` on a UUID field
finds nothing, check whether the query passes a plain string where it should pass
`UUID.fromString(..)`.

`order()` sorts UUIDs by their `u128` value. That order is the same as the lexicographic order of
their canonical strings. `dedup()` and `group_count()` key by the UUID value. A `group_count()`
result is a map whose keys keep their type: `m[UUID.fromString(..)]` reads an entry, and the table
prints a UUID key in its canonical form (see [Maps With Non-String Keys](value_keyed_maps.md)).

## Coercion on write

JSON has no UUID type. `property_json(#{uid: UUID.fromString(..)})` therefore writes the canonical
string. What gets stored depends on the schema mode:

- In `None` mode (no schema), the string is stored as a `String`.
- In `Open` or `Closed` mode, a field declared `uuid` converts a canonical string to a `Uuid`
  before validation. This applies to `property_json` and to `property(..)` alike. A non-canonical
  string is rejected.

## GraphSON import and export

[GraphSON](graphson.md) has a UUID type: export writes a `Uuid` as `{"@type": "g:UUID", "@value":
"<canonical text>"}` and import reads it back as a `Uuid` without any schema (an upper-case payload
is accepted). A plain string stays a string unless the target graph's `open`/`closed` schema
declares the property `uuid`, which coerces it on write like any other write.

## GraphML import and export

GraphML has no UUID type either. Export writes a `Uuid` as its canonical string under a key with
`attr.type="string"`. Import restores a `Uuid` only from a declaration: load the schema into the
graph before importing the file (`set_schema(..)`), and every `<data>` value of a property that the
schema declares `uuid` is converted, whatever the schema mode. For a vertex with several labels the
first label that declares the property decides. Without a declaration the value stays a `String`;
the importer never guesses a type from the text. A declared `uuid` whose text is not canonical
fails the import with an `InvalidAttributeValue` error that names `uuid` as the expected type.

GraphML is an exchange format and carries **no schema**: export writes the elements only, and
import never reads a schema from the file (graph-level `<data>` values are ignored). The schema
travels separately as its own JSON text ([the schema format](schema_enforcement.md#the-format)): save it next to the GraphML file
and set it on the target graph *before* importing, then the declared types come back:

```rust,ignore
let mut target = GraphSource::empty();
target.write().set_schema(GraphSchema::from_json(&schema_json)?)?;   // 1. the schema
target.import_graphml_reader(std::io::Cursor::new(graphml_text))?;       // 2. the graph
```

In Python the schema is a keyword of the load (a mapping or JSON text; `mode` replaces its own mode), or
set it on a live graph and import into it:

```python
typed = graphersal.Graph.from_graphml("items.graphml", schema=json.load(open("items.schema.json")))
# or: graph.set_schema(schema); graph.import_graphml("items.graphml")   # one unit, all or nothing
```

The command line does the same with `graphersal --graph items.graphml --schema items.schema.json`.
The browser playground's Save menu downloads these two files (Data as GraphML, Schema), and its "Load from
file" dialog takes both: the schema file is set on the new graph before the GraphML file is imported.

The other value types are written as their own text: dates and times as their canonical text
(restored like a `uuid`, from a declared format), `true`/`false`, integers as `long` keys
(GraphML's `int` is 32-bit), floats as `double` keys, and arrays and objects as JSON text under a
`string` key. A property whose type differs between labels is written under a `string` key, so an
integer there comes back as the string of its digits. A property the target's schema declares
`array` or `object` (also as a member of a union) is read back from that JSON text and must
conform to the declared type (`IOError::InvalidJsonValue` names the property when the text is not
JSON); without a declaration the JSON text stays a string. A union of a container and a text type
(`string`/`uuid`/a date or time type, e.g. `union<string|array<int64>>`) cannot be imported, because its GraphML text
could be either (`IOError::AmbiguousContainerImport`): declare one of the two. `NaN` and infinite
floats cannot be exported.

## String operations do not apply

The string steps `to_lower()`, `to_upper()`, `trim()`, `l_trim()`, `r_trim()`, `substring()`,
`replace()` and `length()` raise a cast exception on a UUID (`got type 'uuid'`). Convert with
`as_string()` first if you really want string operations.

The text predicates (`TextP.containing`, `starting_with`, `ending_with` and their negations)
evaluate to `false` on a UUID and do not raise an error. TinkerPop behaves the same way, because a
text predicate never matches a non-string value.
