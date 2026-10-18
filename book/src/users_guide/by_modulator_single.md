# The by() Modulator: Single-by Steps

`aggregate()`, `store()`, `groupCount()` and `valueMap()` take **one** `by()`.

- **TinkerPop:** a second `by()` on these steps is an error.
- **Graphersal:** the same; the second `by()` fails with `InvalidModulator` when the traversal
  executes, and the `help()` text shows how to project several values in one traversal.
- `valueMap().by(...)` applies its one `by()` to every key; it never cycles through several
  `by()` modulators.
- **Unaffected:** `group()` takes a key `by()` and a value `by()`; `project()`, `order()` and
  `path()` keep their own `by()` rules.

```rhai
g.V().aggregate("x").by("name").by("age").toList()
// Caused by: `aggregate()` cannot use its by() #2: aggregate() and store() take a single by()

g.V().aggregate("x").by(__.values("name", "age").fold()).cap("x")   // one traversal instead
```
