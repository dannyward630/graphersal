# Step - dedup

`dedup()` keeps the first traverser of each distinct object, in stream order; one `by()` makes the
key a projection of the object. `dedup(Scope.local)` removes duplicates inside the list each
traverser holds. The rules are in the rustdoc of `GraphTraversalSource::dedup`.

## `dedup("a", "b", ...)`

With step labels the key is the list of the objects last labelled so on each traverser's path,
each projected by the `by()` when there is one (TinkerPop's `DedupGlobalStep` with dedup labels).

```rhai,graph=modern
g.V().as("a").both().as("b").dedup("a", "b").by(T.label).select("a", "b").by("name")
// {"a": "marko", "b": "josh"}, {"a": "marko", "b": "lop"}, {"a": "lop", "b": "peter"}
```

- One traverser survives per distinct combination; its bulk is reset to 1, as for `dedup()`.
- The `by()` runs on each labelled object as a fresh traverser.
- A traverser whose `by()` yields nothing is filtered out. **Deviation from TinkerPop:** a traverser
  that lacks one of the labels is filtered out too, where TinkerPop raises an error.
- `dedup(Scope.local, "a")` ignores the labels, as TinkerPop does.

Rust: `dedup_scoped_labels(Scope::Global, ["a", "b"])`.
