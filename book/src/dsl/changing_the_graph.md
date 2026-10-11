# Changing the Graph

`add_v()`, `add_e()`, `property()`, `drop()`, `remove_property()` and the upsert steps
`merge_v()`/`merge_e()` write to the graph. Whatever writes, the same guarantees hold:

- **One traversal is one unit.** It commits when it succeeds; when any step fails (a schema
  violation, a limit, a timeout) nothing it wrote stays. A whole script, or several traversals in
  Rust, can be one unit too.
- **A schema is enforced on every write**, when the graph has one in mode `open` or `closed`.
- **Writes need permission** when the host restricts queries: `Create`, `Update` or `Delete` on
  the data, checked before the traversal runs.

```rhai,graph=modern,script
g.add_v("person").property("name", "ann").as("a")
  .v().has("name", "marko").add_e("knows").to("a")
  .iterate();
g.merge_v(#{name: "ann"}).iterate();                // finds ann, creates nothing
g.v().has("name", "ann").remove_property("age").iterate();
```

| Page | What it covers |
|---|---|
| [Transactions](../users_guide/transactions.md) | units, whole-script units, dry runs, change capture, commit hooks |
| [Upserts: mergeV and mergeE](../users_guide/upserts.md) | find-or-create in one step |
| [addE() Endpoints](../users_guide/add_edge_endpoints.md) | every way to name the ends of a new edge |
| [Dropping Elements](../users_guide/dropping_elements.md) | `drop()` and what an old reference sees afterwards |
| [Removing Properties](../users_guide/removing_properties.md) | `remove_property()` |
| [Compressed Properties](../users_guide/compressed_properties.md) | keeping large texts compressed in memory |
