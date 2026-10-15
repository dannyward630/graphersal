# Vectors

A **vector** is a fixed-length list of numbers stored as one value: an embedding of a text, an
image or a node, as a language or vision model computes it. A property holds a `vector`, not a
list that happens to contain numbers, and the engine stores it compactly, converts it between
element kinds and compares it as one value. Vectors are a Graphersal extension: TinkerPop has no
vector type.

**Embeddings are computed outside the engine.** A step that called an embedding model would make
a query depend on a network service, could not be rolled back (a paid call stays paid) and would
not run in the browser playground. The host computes them, in Python, in an MCP tool or in a
server, and writes them as ordinary property values.

```rhai,graph=empty
type_of(vector([0.12, -0.5, 0.33]))   // "Vector"
vector([0.12, -0.5, 0.33]).dimension   // 3
vector([0.12, -0.5, 0.33], "i8").element   // "i8"
g.inject(vector([0.5, 1.0])).is(P.typeOf(GType.VECTOR)).count().next()   // 1
```

## Element kinds and quantization

A vector has 1 to 65 536 components, all of one **element kind**:

| Element | A component | Bytes per component | 768 components |
|---|---|---|---|
| `f32` (default) | an IEEE single-precision float | 4 | 3 KiB |
| `f16` | an IEEE half-precision float | 2 | 1.5 KiB |
| `i8` | an 8-bit code times one f32 scale per vector (`value = code * scale`) | 1 | 768 B |
| `bit` | one sign bit: `1.0` for a set bit, `-1.0` otherwise | 1/8 | 96 B |

A list of 768 numbers as a plain `array` takes about 24 KiB; the same vector as `f32` 3 KiB.

**Quantization** converts a vector to another element kind, to save memory at a small cost in
precision:

- **f32 to f16** rounds every component to the nearest half value (ties to even). A component
  beyond the half range (65 504) is an error, never an infinity.
- **f32 to i8** is symmetric scalar quantization: `scale = max |x| / 127`, `code = round(x /
  scale)` in -127..=127. A zero vector has scale 0.
- **f32 to bit** keeps the sign: a component `> 0` sets its bit (binary quantization).
- Every other conversion goes through the f32 values, so `quantize("f32")` dequantizes.

```rhai,graph=empty
vector([0.5, -1.0, 0.25]).quantize("bit").to_list()   // [1.0, -1.0, 1.0]
vector([0.5, -1.0, 0.25]).quantize("f16").to_list()   // [0.5, -1.0, 0.25]
g.inject(vector([0.5, -1.0, 0.25])).quantize("i8").next().element   // "i8"
vector([1e10], "f16")   // fails: the component is beyond the range of an f16 vector
```

All four kinds are part of the value type and of every file format from the start, so storing a
property quantized is a choice of the data, never a format change.

## Building vectors

- `vector(list)` builds an f32 vector of a list of numbers; `vector(list, kind)` quantizes it to
  `"f16"`, `"i8"` or `"bit"`.
- The `as_vector()` step converts a list of numbers in a traversal (a stored list, a `fold()`);
  a vector passes, `null` stays `null`.
- A schema that declares the property as a vector converts a written list on write (below).

A vector never holds NaN or an infinity (it would break ordering and comparison); `-0.0` is kept
and equals `0.0`. An empty list, a component that is not a number and more than 65 536
components are errors:

```rhai,graph=empty
g.inject([0.5, 1]).as_vector().next()   // vector([0.5, 1.0])
g.inject(1, 2, 3).fold().as_vector().next().dimension   // 3
vector([])   // fails: a vector has 1 to 65536 components
vector([1, "x"])   // fails: component 1 is not a number
```

A vector prints as its DSL literal, which evaluates back to the same vector (an `i8` or `bit`
vector shows its dequantized values and its kind): `vector([0.5, 1.0])`, `vector([0.503937,
-1.0], "i8")`. `to_string()` and the step names in `.profile()` cut it after 16 components, like
a list. In a script a vector has the getters `element` and `dimension` and the methods
`quantize(kind)` and `to_list()` (the values as floats).

## Writing, filtering and ordering

A vector is written like any other value, and `has()` compares it by value: the same element
kind, the same dimension and equal components. A list is never equal to a vector, and an `f16`
vector never equals an `f32` one, even with the same values:

```rhai,graph=empty,script
g.add_v("doc").property("title", "Graphs").property("emb", vector([0.1, 0.9])).iterate();
g.add_v("doc").property("title", "Vectors").property("emb", vector([0.8, 0.2])).iterate();
g.v().has("emb", vector([0.1, 0.9])).values("title").next();   // "Graphs"
g.v().has("emb", [0.1, 0.9]).count().next();   // 0
g.v().has_label("doc").order().by("emb").values("title").to_list()   // ["Graphs", "Vectors"]
```

Vectors order (a Graphersal extension) after the dates and before strings; among themselves by
element kind (`f32`, `f16`, `i8`, `bit`), then by dimension, then component by component. A
vector is one value in the local steps (`count(Scope.local)` is 1, `unfold()` passes it), and
`min()`/`max()` refuse it.

A [property index](property_indexes.md) never holds vectors: `has(key, vector)` on an indexed
key reads the elements by a scan, with the same result.

**Quantize an existing property** with one write per element, then declare the new element in the
schema (`g.patch_schema(..)`, below):

```rhai,graph=empty,script
g.add_v("doc").property("emb", vector([0.5, -1.0, 0.25])).iterate();
g.v().has_label("doc").has("emb").property("emb", __.values("emb").quantize("i8")).iterate();
g.v().values("emb").next().element   // "i8"
```

## Vectors in a schema

A schema declares a vector property as an `array` with the custom format `vector`:

```json
{"type": "array", "format": "vector", "element": "i8", "dimension": 768, "metric": "cosine"}
```

- `element` (default `f32`) and `dimension` (optional: any dimension) are checked exactly: a
  stored vector of another kind or length does not conform. `metric` (`cosine`, `dot`, `l2`,
  `hamming`) is an annotation: the metric the vectors are meant to be compared with.
- `items` is not allowed next to the format (the components are numbers of the element kind).
- Another JSON Schema validator ignores the unknown format and keywords and sees an array of
  numbers, which is what a vector exports as. The schema visualizer's per-label JSON Schema
  (`visualize("jsonschema")`) spells out `items` and `minItems`/`maxItems` for it.

In mode `open` or `closed`, a **list of numbers written to the property becomes a vector of the
declared element**, and a vector of another element is quantized to it: a property is stored
quantized from the start, whatever the host sends. The dimension is never changed:

```rhai,graph=empty,script
g.set_schema(#{mode: "open", vertices: #{doc: #{schema: #{type: "object", properties: #{
  emb: #{type: "array", format: "vector", element: "i8", dimension: 3}}}}}});
g.add_v("doc").property("emb", [0.5, -1.0, 0.25]).iterate();
g.add_v("doc").property("emb", vector([1.0, 2.0, 3.0])).iterate();
g.v().values("emb").to_list().map(|v| v.element)   // ["i8", "i8"]
```

On that graph, `g.add_v("doc").property("emb", [1.0, 2.0])` fails with "Type mismatch on vertex
'doc.emb': expected vector<i8, 3>, got vector<i8, 2>", and its help names the conversion
(`values("emb").as_vector().quantize("i8")`).

`infer_schema()` reports a vector as such, with its dimension when every vector of that element
at the position has the same one. `diff_schema()` counts a change of `element` or `dimension`
(and a list turned into a vector, or back) as incompatible, since the stored data no longer
conforms; dropping `dimension` (any dimension) is compatible. A saved query takes a vector
parameter the same way: `params: #{q: #{schema: #{type: "array", format: "vector", element:
"f32", dimension: 768}}}`; a list argument is converted to the declared vector.

## Storage and exchange

| Format | A vector |
|---|---|
| Graphersal's binary format (snapshots, the Store, the journal) | its own value tag: the element kind, the dimension, for `i8` the scale, then the packed components ([format spec 3](../persistence/format.md#3-value-encoding)) |
| Serialized `ChangeSet` | exact, tagged: `{"@type": "graphersal:Vector", "@value": {"element": "f32", "data": [..]}}` ([Transactions](transactions.md)) |
| JSON results, `to_json()` | the list of its values (an `i8` or `bit` vector dequantized) |
| GraphSON export | a `g:List` of `g:Float`, which TinkerGraph loads |
| GraphSON import | a list stays a list; it becomes a vector where the target schema declares one (mode `open`/`closed`); the exact `graphersal:Vector` tag is read too ([GraphSON](graphson.md)) |
| GraphML | the JSON text of the list; a vector again where the target graph's schema declares one (any mode) |

Exporting an `i8` or `bit` vector as its values loses nothing that matters: a declared `i8`
property quantizes them back to the same codes. `memory_usage()` counts a vector's components
exactly (plus a small header), and the memory budget of a query estimates them the same way.

## From Python

A vector property is a `graphersal.Vector` in Python, built from a list of numbers or from any
one-dimensional `float32`/`float64` buffer such as a `numpy` array (the extension does not depend
on `numpy`):

```python
import numpy as np
from graphersal import Graph, Vector

g = Graph()
emb = np.array([0.12, -0.5, 0.33], dtype=np.float32)      # model.encode(text) in practice
g.execute('g.add_v("doc").property("emb", e).iterate()', params={"e": Vector(emb)})
v = g.query('g.v().values("emb").next()')                # graphersal.Vector
v.element, v.dimension, v.tolist()                       # ('f32', 3, [0.12, -0.5, 0.33])
Vector(emb, element="i8")                                # quantized, 1 byte per component
```

See the Python binding's README for the details.

## In the playground and the CLI

The result table shows a vector as `vector[768 f32]` with an expandable preview of its first
values; the JSON view holds all of them. The property editor edits a vector as a JSON list of
numbers (a declared vector property offers the type `vector`). The CLI's tables show the kind and
the first values (`vector[768 f32] [0.012, -0.5, 0.33, 0.1, …]`), and a final vector prints as its
literal.
