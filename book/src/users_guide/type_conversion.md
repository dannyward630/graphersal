# Type Conversion and length()

Deviations of `length()`, `as_string()`, `as_bool()`, `as_number()` and `cast()` from Apache
TinkerPop 3.8.2.

## length() accepts strings only

- **TinkerPop:** `length()` is a string step; any other input is a cast error.
- **Graphersal:** the same. A list, map, path, vertex, edge or number raises a cast error, in
  both scopes; `null` stays `null`. Use `count(Scope.local)` for the size of a collection.
  Earlier Graphersal versions also measured lists and paths; that was removed.

```rhai,graph=modern
g.V().fold().length().toList()            // error: expected type [string], got type 'array'
g.V().fold().count(Scope.local).toList()  // 6
```

- **Deviation:** the length counts Unicode scalar values (Rust `chars().count()`). TinkerPop
  counts Java `char`s (UTF-16 code units). The two agree for every character in the Basic
  Multilingual Plane (`"é"` is 1, `"日本語"` is 3, in both). They differ for astral-plane
  characters such as emoji: `"😀"` is 1 here and 2 in TinkerPop. The same applies to
  `length(Scope.local)` and to `length()` of a label.

## null: asString() and asBool() raise, asNumber() and cast() keep it

- **TinkerPop:** `asString()` and `asBool()` reject `null` ("Can't parse null"); `asNumber()`
  keeps it.
- **Graphersal:** the same, per step. `as_string()` and `as_bool()` raise a cast error on `null`
  (the configured `CastPolicy` still applies); `as_number()` and the Graphersal extension
  `cast()` keep `null` as `null`. The behaviour is deliberately not uniform.

```rhai,graph=modern
g.inject(()).asString().toList()   // error: expected type [string], got type 'null'
g.inject(()).asNumber().toList()   // null
```

## asBool() and asNumber() are explicit conversions

`as_bool()` and `as_number(GType.INT | GType.LONG)` follow TinkerPop's own rules, not the schema
coercion matrix. The matrix (used when a declared property is written) and the Graphersal
extension `cast()` stay strict: `cast(GType.BOOLEAN)` of `2` or of `"TRUE"` and
`cast(GType.INT)` of `5.43` still fail, because a lossy conversion there would corrupt stored data.

`asBool()`:

| Input | Result |
|---|---|
| `true`, `false` | unchanged |
| `0`, `0.0`, `-0.0`, `NaN` | `false` |
| any other number (`1`, `-1`, `3.14`, `Infinity`) | `true` |
| the texts `true` / `false` in any ASCII letter case (`"tRUe"`, `"FALSE"`) | `true` / `false` |
| any other text (`"hello"`, `"1"`, `""`, `" true"`), `null`, a uuid, a collection | error |

`asNumber(GType.INT)` and `asNumber(GType.LONG)` (the same `int64` target):

| Input | Result |
|---|---|
| an integer | unchanged |
| a float | truncated toward zero (`5.67` is `5`, `-5.67` is `-5`) |
| a text that parses as a whole integer (`"5"`), a boolean | `5`, `1`/`0` |
| `null` | `null` |
| `"5.7"`, `NaN`, infinity, a float outside the `int64` range | error |

- **Deviation (asNumber):** TinkerPop's `Integer` target wraps or narrows to its own width; Graphersal
  has one integer width, so a `NaN`, infinite or out-of-range float is an error instead.
- **Deviation (asBool):** only the two literals `true` and `false` convert from text, never `"1"` or
  `"yes"`: a string is not interpreted by its content.

```rhai,graph=modern
g.inject(-1, 0.0, "tRUe").asBool().toList()        // [true, false, true]
g.inject(5.67, -5.67).asNumber(GType.INT).toList() // [5, -5]
g.inject(5.43).cast(GType.INT).toList()            // error: stays strict
```

## trim(), lTrim() and rTrim()

- **Deviation:** the three steps strip Unicode whitespace (the White_Space property, Rust
  `str::trim`), while Java's `String.trim()` strips only characters up to U+0020. A no-break
  space (U+00A0) or an ideographic space (U+3000) at the end of a string is removed here and kept
  by TinkerPop. No vendored scenario depends on it.
