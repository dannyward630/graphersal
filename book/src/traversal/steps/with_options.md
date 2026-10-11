# Step-level with(WithOptions..)

`with(WithOptions.key)` / `with(WithOptions.key, WithOptions.value)` after a step configures that step
(TinkerPop's `Configuring` steps). Two steps take an option:

| Step | Option | Values |
|---|---|---|
| `valueMap()` | `WithOptions.tokens` | none given or `all`: id and label; `ids`; `labels`; `none` |
| `index()` | `WithOptions.indexer` | `list` (default): `[element, position]` pairs; `map`: `{position: element}` |

```rhai,graph=modern
g.V("1").valueMap("name", "age").with(WithOptions.tokens, WithOptions.labels).next()
// {"label": "person", "name": ["marko"], "age": [29]}
g.V().hasLabel("person").values("name").fold().order(Scope.local).
  index().with(WithOptions.indexer, WithOptions.map).next()
// {0: "josh", 1: "marko", 2: "peter", 3: "vadas"}
```

- `WithOptions.keys`/`WithOptions.values` select the key and value tokens of a vertex-property map
  in TinkerPop; `valueMap()` here reads vertices and edges only, so they add no token.
- Any other step, an option the step does not know, or a `with()` with no step before it fails
  with `InvalidModulator` before the traversal runs.
- `with("key", value)` with a **string** key is something else: an execution option of the whole
  traversal (`g.with("random.seed", 7)`), see the options reference.

The DSL spells the tokens `WithOptions.tokens` and `WithOptions::tokens`.

Rust: `with_option(WithOptions::Tokens, Some(WithOptions::Labels))`,
`with_option(WithOptions::Indexer, Some(WithOptions::Map))`.
