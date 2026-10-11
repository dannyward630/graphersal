# Property index (backlog)

Status: BACKLOG. The source-side `has` folding is in 0.1.0; the index itself comes later.
Nothing here is decided beyond what the "Already in place" section describes.

## Why

`v().has_label("person").has("title", "CEO")` on an org chart of 546 500 `person` vertices with 500
CEOs reads every `person` vertex and compares its `title`: ~11 ms after the folding below, ~26 ns per
vertex, almost all of it dependent memory loads (vertex slot -> its own property map -> hash table ->
entry). No change to the filter makes that much cheaper; only not reading the 546 000 non-matching
vertices does. An index `(label, key) -> value -> vertices` answers it with 500 lookups.

## Already in place (0.1.0)

- `source_filter_pushdown` folds `has(key, value)` (literal name) into `v()`/`e()` as a
  `PropertyFilter` (`traversal/step/mod.rs`), checked while the source scans:
  `v(labels: ["person"]).has("title", "CEO")`. This is the shape an index plugs into: when an index
  covers a folded condition, the source reads the index instead of the label index and checks the
  remaining conditions on the candidates.
- The catalog reserves definition kind `2` (`PROPERTY_INDEX`) for index definitions
  (`graph/definition.rs`, format spec 6.3/16; `roadmap/saved-queries.md`).
- Count/group-count pushdown skip a source with property filters (they read the statistics).

## Shape (proposal)

- Declared, never automatic: `g.create_index(label, key)` / `drop_index`, stored as a catalog
  definition (kind 2), so it travels with the store (WAL op `SetDefinition`, snapshots, backups,
  forks) like saved queries. The entries are rebuilt from the data at load (not persisted) in the
  first version.
- Maintained in the raw helpers the undo log replays (`insert_vertex_raw`, `remove_vertex_raw`,
  property set/remove, label-set changes), like the label index and the statistics, so rollback,
  replay, load and `apply_change_set` keep it exact; a conformance item compares it with a rebuild.
- Executor: a `v()` with folded conditions picks the most selective covering index (statistics per
  index: distinct values, entries) and checks the other conditions on its candidates; `.profile()`
  shows the access path (e.g. `v(labels: ["person"]).has("title", "CEO") [index: person.title]`).

## Open decisions

1. Hash (equality, `P.within`) or ordered (also `P.gt`/`P.between`, range scans, `order()` help).
2. Value equality must equal `has()`'s exactly: `int64` vs `float64` (`has("age", 29.0)`), `uuid`
   vs its canonical string, `null`. Index keys by the logical value, or require a declared schema
   type for the key?
3. Multi-label vertices: one index per label, a vertex in every index of its labels.
4. Edges too (`e().has_label("knows").has("weight", 0.4)`)?
5. Memory budget and `memory_usage()` accounting of index entries; the wasm32 playground limits.
6. Unique indexes (constraint) as a later extension or never.
