# Property Elements

This page lists where Graphersal's property elements differ from Apache TinkerPop 3.8.2.
Everything not listed follows `Dedup.feature` and `AsString.feature`.

`properties()` yields property elements; `properties(jpath(..))` yields a path property element, see
[Path Keys (jpath)](jpath_keys.md#propertiesjpath).

`properties()` yields property elements. A property element is not its value: it has an identity,
`key()`/`value()` and a text form. `values()`, `id()`, `label()` and `labels()` yield plain
values. `dedup()`, `as_string()` and set side effects (`withSideEffect("a", Set.of())`) tell the
two apart:

```rhai,graph=modern
g.V().properties("name").dedup()          // by identity: three "josh" vertices stay three
g.V().values("name").dedup()              // by value: one "josh"
g.V().properties("name").asString()       // "vp[name->marko]"
g.E().properties("weight").asString()     // "p[weight->0.5]"
g.V().asString()                          // "v[1]"
g.E().asString()                          // "e[0][1-knows->2]"
```

## Labels and ids of property elements

The label of a vertex property is its key, as in TinkerPop, where a vertex property is an element:

```rhai
g.V().properties().label()                // "name", "age", ...
g.V().properties().hasLabel("name")       // the name properties
g.V().properties().hasLabel(())           // empty: a null label matches nothing
g.V().properties().id()                   // error: property elements have no id
g.V("1").properties("name").element().id()  // "1": the owning vertex
```

Only elements from `properties()` have a label. A plain value (`values("name")`, or the output of
`id()`/`label()`) and an edge property are not elements, and `label()`/`hasLabel()` fail on them with a
cast error. Other label readers (`has(T.label, ..)` and the token predicates) do not take property
elements and are unchanged.

## by() tokens over property elements

`by(T.label)` over `properties()` is the key, like `label()`, so it works in `order()`, `group()`,
`dedup()`, `project()` and the other `by()` steps:

```rhai,graph=modern
g.V().properties().order().by(T.label)               // sorted by key: age, lang, name
g.V().properties().group().by(T.label).by(__.count())
```

`by(T.id)` over property elements is an error with the same help as `id()` (there are no property ids).
`by(T.label)` or `by(T.id)` over a value that is not a vertex, edge or vertex property (`values("name")`)
is an error too, as in TinkerPop (`TokenTraversal` throws `IllegalStateException`, read from the 3.7.2
bytecode; the 3.8.2 feature files agree): it is never a silently dropped traverser. A missing property
key, or a child traversal that yields nothing, still drops the traverser (TinkerPop 3.6+).

`by(T.key)` and `by(T.value)` read a property element's key and value, as in TinkerPop 3.8:

```rhai,graph=modern
g.V().properties().order().by(T.key, Order.desc).key()   // name, name, ..., lang, lang, age, ...
g.V().properties("name").dedup().by(T.value).count()       // distinct names
```

Both also read a map entry (`unfold()` of a map: its key and its value) and a `properties(jpath(..))`
value (its path and its leaf). Any other value is a cast error, as in TinkerPop: a vertex, a number,
and also a `values(..)` result, which is the plain value, not a property element. To order or
deduplicate plain values, use `by()` with no argument:

```rhai,graph=modern
g.V().values("age").order().by(T.value)    // cast error: expected a property element or a map entry
g.V().values("age").order().by()           // 27, 29, 32, 35
```

## has() on property elements

`has(key)`, `has(key, value)`, `has(key, P)` and `hasNot(key)` on a property element address the
property's **own value**: the key inside an object-valued property, never another property of the
owning element. Graphersal has no meta-properties, so this is where `has()` looks instead:

```rhai
// vertex: a = 100, meta = {a: 1, b: "x"}
g.V().properties("meta").has("a", P.eq(1))      // matches: meta.a is 1
g.V().properties("meta").has("a", P.eq(100))    // empty: the top-level `a` is not read
g.V().properties("meta").has("a")               // meta has a key `a`
g.E().properties("info").has("w", P.gt(1))      // the same for edge properties
```

A missing nested key filters the traverser out; a property whose value is not an object is a cast
error (`expected type [object], got type 'string'`). For a deeper path use a jpath key on the
element, `has(jpath("meta.a"), P.eq(1))`, see [Path Keys (jpath)](jpath_keys.md).

## valueMap tokens

`valueMap(true, keys..)` starts each map with the element's `T.id` and `T.label` entries, which are
plain values, not lists (written `"id"` and `"label"` outside the engine; a property named `id` or
`label` is an entry of its own, see [Properties Named id or label](id_and_label_keys.md)). `by()` modulates only the property lists, never the tokens. A leading `false` is the
plain `valueMap(keys..)`. A `null` key (`()` in Rhai) matches no property and is dropped.

```rhai,graph=modern
g.V("1").valueMap(true, "name")           // {id: "1", label: "person", name: ["marko"]}
g.V("1").valueMap(true).by(__.unfold())   // {id: "1", label: "person", age: 29, name: "marko"}
g.V("1").valueMap("name", ())             // {name: ["marko"]}
```

In Rust: `value_map_with_tokens(args)`.

## Deviations

- **A vertex property's identity is the vertex and the key.** A graph stores one value per key, so
  that pair is the property id. TinkerPop gives every property instance its own id; there is no
  multi-property (`Cardinality.list`/`set`) here.
- **`dedup()` of an edge property compares key and value**, as TinkerPop's `Property.equals` does:
  the same weight on two edges is one result.
- **A set side effect keeps an edge property by identity** (its edge and key). TinkerPop compares
  edge properties by key and value, so `withSideEffect("a", Set.of()).E().properties()` keeps the
  same weight of two edges twice here and once there. `values()` is by value in both.
- **`has(key, ..)` on a property element reads the key inside the property's object value.**
  TinkerPop tests the property's meta-properties there (and an edge property has none). Graphersal
  has no meta-properties, so `properties("meta").has("a", P.eq(1))` addresses `meta.a`, never the
  owning element's top-level `a`; a non-object value is a cast error.
- **`key()` and `value()` also accept a plain value that came from `values(key)`.** TinkerPop
  rejects them there, because `values()` yields no property. Graphersal keeps the lazy handle that
  `values(key)` produces for speed and lets these two steps read it.
- **Number text follows the string cast, not Java's `toString`.** The value inside `vp[..]`/`p[..]`
  is `as_string()`'s text of the stored value: `29` for an integer, and a float with no fractional
  part prints without `.0` (TinkerPop: `p[weight->1.0]`).
- **`as_string()` of a map or a list is not Java's `Map.toString`.** `valueMap().asString()` does
  not give `{name=[marko]}`; a map or list is a cast error. `as_string(local)` over a list converts
  each element with the rules above.
- **A property whose value is missing or null renders `null` inside the brackets** (`vp[name->null]`); TinkerPop never
  holds such a property.
- **Property elements have no id.** Graphersal has no property ids, so `properties().id()` is an error whose
  help names `key()` and `element().id()`; TinkerPop returns an opaque property id. No synthetic id is
  invented: it could never equal TinkerPop's and could not be fed back into `V()` or `hasId()`.
  `Orderability::g_V_properties_order_id` stays failing for this reason (it is a deliberate incompatibility).
- **`order()` of property elements sorts by value only.** TinkerPop orders `Property` elements by their own
  total order (key, then value, with cross-type rules) and by property id; here `properties().order()` compares
  the values, so `Orderability::g_E_properties_order_value`, its `by(desc)` twin and the two
  `path().order().by(..)` scenarios differ. Those scenarios, and `g_V_properties_order_id`, are
  [deliberate incompatibilities](../for_developers/tinkerpop_compliance.md#deliberate-incompatibilities).
- **`label()` and `hasLabel()` work for vertex-property elements only.** Edge properties are plain
  properties in TinkerPop and stay errors, as do value handles from `values()`.
- **`valueMap(null)` alone means all keys.** In Java the single-null varargs call is ambiguous; here a null
  key is dropped wherever it stands, so a lone null leaves no key and selects every property.
