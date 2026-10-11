# Step - union

`union(a, b, ...)` runs every child traversal on each traverser and emits all their results.
As a start step (`g.union(..)`) the children start the traversal themselves.

```
g.V("1").union(__.out("knows"), __.out("created")).values("name")   // ["josh", "vadas", "lop"]
g.union()                                                         // nothing
```

`union()` with no child produces nothing.

Rust: `union(vec![a, b])`.
