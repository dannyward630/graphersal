# Local Scope

Most steps work on the whole stream: `count()` counts traversers, `sum()` adds up every number
that flows past. TinkerPop's `Scope` token changes that. With `Scope.local` a step works on the
collection that **each single traverser holds** — a list from `fold()`, a map from `group()`, a
stored list property — and emits one result per traverser.

## Quick reference

On the modern graph:

```rhai
g.V().fold().count(Scope.local)                          // 6: the size of the one folded list
g.V().group().by(T.label).count(Scope.local)             // 2: the number of map entries
g.V().count(Scope.local)                                 // 1, 1, 1, 1, 1, 1: a vertex is one element
g.V().values("age").fold().sum(Scope.local)              // 123
g.V().values("age").fold().min(Scope.local)              // 27
g.V().values("age").fold().max(Scope.local)              // 35
g.V().values("age").fold().mean(Scope.local)             // 30.75
g.V().values("foo").fold().sum(Scope.local)              // nothing: the list is empty
g.V("1").values("age").sum(Scope.local)                  // 29: a scalar is a one-element list
g.V().where(__.out().fold().count(Scope.local).is(P.gt(2))).values("name")   // "marko"
g.V().values("name").fold().limit(Scope.local, 2)        // ["marko", "vadas"]
g.V().values("name").fold().skip(Scope.local, 4)         // ["ripple", "peter"]
g.V().values("name").fold().tail(Scope.local)            // ["peter"]: still a list
g.V().values("name").fold().range(Scope.local, 1, 3)     // ["vadas", "lop"]
g.V("1").valueMap("name", "age").tail(Scope.local, 1)    // {age: [29]}: a map stays a map
g.V("1").values("age").range(Scope.local, 20, 30)        // 29: a scalar passes through
g.V().hasLabel("person").fold().order(Scope.local).by("age")   // [v[vadas], v[marko], v[josh], v[peter]]
g.V().values("age").fold().order(Scope.local).by(Order.desc)   // [35, 32, 29, 27]
g.V("1").elementMap().order(Scope.local).by(Column.keys)       // {age, id, label, name}: entries sorted by key
g.V().out().in().values("name").fold().dedup(Scope.local)      // ["marko", "peter", "josh"]
g.V().values("name").order().fold().toUpper(Scope.local)       // ["JOSH", "LOP", "MARKO", "PETER", "RIPPLE", "VADAS"]
g.V().values("name").order().fold().length(Scope.local)        // [4, 3, 5, 5, 6, 5]
g.V().hasLabel("person").values("age").order().fold().asString(Scope.local)   // ["27", "29", "32", "35"]
g.V().hasLabel("software").values("name").order().fold().replace(Scope.local, "p", "g")  // ["log", "riggle"]
g.V().hasLabel("software").values("name").order().fold().substring(Scope.local, 1, 4)   // ["op", "ipp"]
g.V().values("name").toUpper(Scope.local)                      // "MARKO", "VADAS", ...: a string is one string
```

In Rust the scoped form of a step is `<step>_scoped(scope)`:

```rust,ignore
use graphersal::prelude::*;

let graph = GraphSource::tinkerpop_modern();
let lock = graph.read();
let total = lock.traversal().v(None).values("age").fold().sum_scoped(Scope::Local).to_list();
```

## What counts as a collection

| The traverser holds | `Scope.local` works on |
|---|---|
| a list: `fold()`, `cap()` of an aggregate, a stored list property (`values("list")`) | its elements, in order |
| a map: `group()`, `group_count()`, `project()`, `select("a", "b")`, `value_map()`, a stored map property | its entries |
| a path (`path()`) | its objects (`count(Scope.local)` is the path length; the `limit`/`skip`/`tail`/`range` family fails with "not implemented" on a path) |
| anything else: a number, a string, a vertex, an edge, a map entry | a one-element collection holding the value |

## Supported steps

| Step | List | Map | Anything else |
|---|---|---|---|
| `count(Scope.local)` | the number of elements | the number of entries | `1` |
| `sum(Scope.local)` | the sum of the elements | error | the value itself |
| `min(Scope.local)` / `max(Scope.local)` | the smallest / largest element | error | the value itself |
| `mean(Scope.local)` | the mean of the elements, a double | error | the value as a double |
| `limit(Scope.local, n)` | the first `n` elements, as a list | the first `n` entries, as a map | the value itself |
| `skip(Scope.local, n)` | all but the first `n` elements | all but the first `n` entries | the value itself |
| `tail(Scope.local, n)` | the last `n` elements (`tail(Scope.local)` = `n = 1`) | the last `n` entries | the value itself |
| `range(Scope.local, low, high)` | the elements `[low, high)` (`high = -1`: to the end) | the entries `[low, high)` | the value itself, even out of range |
| `order(Scope.local)` + `by(...)` | the elements, sorted | the entries, sorted (still a map) | the value itself |
| `dedup(Scope.local)` | the elements without duplicates, first occurrence first | the map itself (not a list) | the value itself |
| `toUpper`/`toLower`/`trim`/`lTrim`/`rTrim(Scope.local)` | each string transformed, as a list of the same length | error, as the unscoped step | a string: transformed as by the unscoped step; `null`: `null` |
| `length(Scope.local)` | the length of each string | the map's size, as `length()` | a string: its length; `null`: `null` |
| `replace(Scope.local, from, to)` / `substring(Scope.local, start[, end])` | each string transformed | error, as the unscoped step | a string: transformed as by the unscoped step; `null`: `null` |
| `asString(Scope.local)` | each element converted to a string (a `null` element is an error) | error, as the unscoped step | converted as by `asString()` |

The reducers `sum`/`min`/`max`/`mean` skip `null` elements. A list that holds no number at all
(empty, or only nulls) emits nothing, so the traverser is dropped — exactly like the global
`sum()` on an empty stream. They use the numeric rules of the global steps, described next.

### Numeric rules

These rules hold for the global `sum()`/`min()`/`max()` and their `Scope.local` forms alike.

- An integer stream stays an integer and stays exact; overflow of `int64` raises an error (there
  is no `BigInteger`, see [TinkerPop Deviations](../for_developers/tinkerpop_deviations.md)).
- A float joining an integer stream promotes the result to a float, whatever the order: `sum` of
  `1, 2.5` is `3.5`, `min` of `3, 2.5, 1` is `1.0`, and `max` of `29, 0.5` is `29.0` even though
  the maximum came from an integer (as in TinkerPop's `NumberHelper`). Integers beyond 2^53 lose
  precision once widened. `mean` always computes in floating point.
- `min`/`max` also compare strings (`str` order), booleans (`false < true`) and UUIDs: `g.V().values("name").max()`
  is `"vadas"`. Numbers compare with numbers and text with text; a stream that mixes kinds (a
  number and a string, a boolean and a number) fails with a clear error naming both kinds.
  Read the property as one kind first, with `as_string()` or `as_number(GType.LONG)`.
- Graph elements, paths and collections are not ordered by `min`/`max` and fail the same way.
  TinkerPop orders vertices and edges by id here; this engine orders elements only inside `order()`.
  `sum` and `mean` accept numbers only.

The `limit`/`skip`/`tail`/`range` family always returns a collection of the kind it got: a list
stays a list even when one element (or none) is left, as in TinkerPop 3.8
(`limit(Scope.local, 1)` on `[1, 2, 3]` is `[1]`, not `1`), and a map stays a map in entry order.
Bounds follow the global `range()`: `low` and every count must be zero or more and `high` must be
`-1` or at least `low`; otherwise the traversal fails with an "Invalid range" error. In Rust the
steps are `limit_scoped(scope, n)`, `skip_scoped(scope, n)`, `tail_scoped(scope, n)` and
`range_scoped(scope, low, high)`, all taking `i64`.

`order(Scope.local)` takes the same `by()` clauses as the global `order()`: a property key,
`T.label`/`T.id`, a traversal (its first result), `Order.asc`/`Order.desc`, and several `by()`s as
tiebreakers. The sort is stable, and an element whose `by()` yields nothing (a vertex without the
property) is dropped from the list, as in the global step. A `by(__...)` traversal runs on each
element with the path of the traverser that holds the list, so `by(__.select("x"))` sees the
outer step labels. In Rust: `order_scoped(Scope::Local).by("age")`.

`dedup(Scope.local)` compares vertices and edges by identity and everything else by value, and
keeps the elements as they were (a folded vertex list stays a list of vertices). Like TinkerPop,
`dedup(Scope.local, "x", "y")` ignores the labels — upstream passes them only to the global step.
The global label form (`dedup(Scope.global, "x")`, `dedup("x", "y")`) is not implemented. In Rust:
`dedup_scoped(Scope::Local)` and `dedup_scoped_labels(Scope::Local, ["x", "y"])`.

`count(Scope.local)` is a per-traverser step, so the `count_pushdown` optimizer rule never folds
it into the source step: `.profile()` shows it as `count(Scope.local)`.

## String steps

The per-value string steps `toUpper`, `toLower`, `trim`, `lTrim`, `rTrim`, `length`, `replace`,
`substring` and `asString` take a `Scope` as their first argument. With `Scope.local` a list is
transformed element by element into a new list of the same length. A `null` element stays
`null` (`["a", null, "b"]` → `["A", null, "B"]`), and so does a `null` input. Any other element
fails with TinkerPop's message, for example `The trim(local) step can only take string or list of
strings`. The help shows how to convert or filter the elements first
(`...fold().unfold().as_string().fold().trim(Scope.local)`). `asString(Scope.local)` converts
numbers, booleans and UUIDs, but a `null` element has no string form and fails ("Can't parse").
A single string (not a list) is handled exactly as by the unscoped step, and a map fails with the
unscoped step's cast error.

`substring` also takes a single index: `substring(2)` cuts from the third character to the end,
`substring(-3)` keeps the last three characters. Indices count characters, a negative index
counts from the end, and every index is clamped to the string, so `substring(-4, 2)` on `"lop"`
is `"lo"` and `substring(1, 0)` is `""`.

In Rust: `to_upper_scoped(Scope::Local)`, `to_lower_scoped`, `trim_scoped`, `l_trim_scoped`,
`r_trim_scoped`, `length_scoped`, `as_string_scoped`, `replace_scoped(scope, from, to)`,
`substring_from(start)` and `substring_scoped(scope, start, Option<end>)`. `.profile()` shows
the scope only when it is local: `to_upper(Scope.local)`, `substring(Scope.local, 1, 4)`,
`replace(Scope.local, "h", "j")`.

## Sorting maps with `by(Column.keys)` / `by(Column.values)`

`Column.keys` and `Column.values` are `order()` sort keys, in both scopes. On a map entry (from
`unfold()` of a map) they read the entry's key or value; `order(Scope.local)` applies them to each
entry of the map it sorts:

```rhai,graph=modern
g.V().hasLabel("person").group().by("name").by(__.outE().values("weight").sum()).
  order(Scope.local).by(Column.values)                  // {peter: 0.2, josh: 1.4, marko: 1.9}
g.V().hasLabel("person").group().by("name").by(__.outE().values("weight").sum()).
  unfold().order().by(Column.values, Order.desc)        // marko=1.9, josh=1.4, peter=0.2
```

In Rust: `.by(Column::Values)`. Every other step rejects `by(Column...)` with an "invalid
modulator" error whose help shows these forms; to read a map's keys or values as data, use
`select(Column.keys)` / `select(Column.values)`.

## `Scope.global`

`Scope.global` is the ordinary stream-wide step: `sum(Scope.global)` is exactly `sum()`, with the
same plan and the same errors, `tail(Scope.global, 2)` is exactly `tail(2)`,
`order(Scope.global)` / `dedup(Scope.global)` are `order()` / `dedup()`, and
`toUpper(Scope.global)` is `toUpper()` (it fails on a list; use `Scope.local`).

## Spelling

The token is written `Scope.local` / `Scope.global` (also `Scope.Local` / `Scope.Global`) or
`Scope::local` / `Scope::global`. There are no bare `local` / `global` tokens: `local(...)` is the
`local()` step, and `global` is reserved by the script engine. `Scope` is a reserved name, so a
query parameter cannot be called `Scope`.

Feeding a list to a global reducer is a common slip. The error names the fix:

```rhai
g.V().values("age").fold().sum()
// Cast exception: expected type [...], got type 'array'
// The value is a list (for example from fold()): reduce each traverser's list with Scope.local,
// for example g.v().values("age").fold().sum(Scope.local) ...
```
