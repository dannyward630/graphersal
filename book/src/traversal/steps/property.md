# Step - property

`property(key, value)` sets a property on the incoming vertex or edge; the value can be a constant
or a child traversal (its first result; no result leaves the property untouched). The key can be
a `jpath(..)` ([Path Keys](../../users_guide/jpath_keys.md)), and `property(Cardinality, key, value)` and
`property(map)` exist as well.

## `property(traversal, value)`

The key is the first result of a child traversal run on the incoming traverser (TinkerPop's
`AddPropertyStep` with a key traversal):

```rhai,graph=modern
g.withSideEffect("a", "name").addV().property(__.select("a"), "marko").values("name")   // "marko"
```

The key must be a string; no result or another value is a cast error. The value may be a
traversal too (`property(__.select("k"), __.values("age"))`).

Rust: `property_key_by(key_traversal, value)`, `property_key_by_traversal(key_traversal,
value_traversal)`.
