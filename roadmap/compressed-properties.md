# Compressed properties

Status: IMPLEMENTED in 0.1.0, including the session, the playground, the dev server and MCP (see
"Implementation notes" for where the code differs from the decisions).

## Goal

Large text values that queries rarely read (a CV, a description, a document body) take most of a
graph's memory. A user declares *where* such values live (element kind + label + property path,
e.g. vertex `customer`, `details.career.cv`), and the graph keeps them **compressed in memory**.
A traversal that reads one gets the **plain** string, decompressed on the fly into the traverser
value; the graph itself never holds a decompressed copy. Files on disk (snapshots, WAL, GraphSON,
GraphML) always hold the **plain** value: the persistence layer already compresses whole chunks
and large WAL records, compressing single values twice would only cost CPU.

Any number of rules: several paths on one label, the same path on different labels, vertex and
edge labels.

## Decisions

| # | Decision |
|---|---|
| D1 | The configuration is a **catalog definition**: new `DefinitionKind::COMPRESSION = 3` (`"compression"`), body `DefinitionBody::Compression(CompressionDefinition)`. It inherits persistence (manifest table, WAL op 10), undo, change capture, forks, backups, rollback. Not critical (flag bit 0 clear): the data on disk is plain, so a version that does not know the kind loses nothing but the compression. |
| D2 | `CompressionDefinition { element: ElementKind, label, path: JsonPath, codec: Codec, min_bytes: u32, dictionary: bool, extra }`. `path` is a jpath of **names only** (`cv`, `details.career.cv`; an index or `[-1]` is a validation error, arrays of strings are out of scope for v1). Defaults: `codec = lz4`, `min_bytes = 256`, `dictionary = true`. Two definitions with the same `(element, label, path)` are an error (`set_definition` fails, nothing changes). The label need not exist yet. |
| D3 | **In memory: a storage-only value variant** `ElementProperty::Compressed(Box<CompressedText>)` (own type, `value/compressed.rs`; `size_of::<ElementProperty>()` does not grow). Like `Vertex`/`Edge` are stream-only, `Compressed` is storage-only: it never appears in a traverser value, a script value, a `ChangeSet`, an export or a file. `CompressedText { len: u32 (plain bytes), dict: Option<Arc<CompressionDictionary>>, bytes: Box<[u8]> }`: a value is self-contained (carries its dictionary by `Arc`), so decompression needs no graph context and an undo-log copy stays readable after the rule changed. |
| D4 | **Equality, ordering and hashing are logical**: `ElementProperty`'s `PartialEq`/`Eq`/`Ord`/`Hash` (whatever exists today) treat `Compressed` as the string it holds (`Compressed("x") == String("x")`), so no comparison anywhere can silently differ. Compare by decompressing into a thread-local scratch buffer (no per-item allocation, rule 4); two `Compressed` with the same dictionary `Arc` and equal bytes are equal without decompressing. |
| D5 | **The boundary is one helper**: `ElementProperty::logical(&self) -> Cow<'_, ElementProperty>` (borrowed for every scalar and for an `Object`/`Array` without a compressed leaf; deep-decompressed owned copy otherwise) plus `with_text(&self, f: impl FnOnce(&str) -> R)` for reads that only look at a string (predicates, regex, `has` pushdown into `v()`). Every place where a stored value leaves the graph goes through it: `values`, `properties`, `value_map`, `element_map`, jpath reads (`descend_property` leaves), `has`/`where`/predicates (incl. the folded `has(key, value)` in `StepV`/`StepE`), `order().by`, `group`/`dedup` keys, `project`/`select().by`, materialization `into_node_property`, `subgraph()`/`to_graph()` copies, visualizers, the physical codec (`lower`/`write_json`), GraphSON/GraphML export, the snapshot encoder, change capture (before/after values), `dry_run`, schema validation scans (`maxLength`, `pattern`), `infer_schema` (type `string`), `diff`. `match`es on `ElementProperty` in `graphersal` stay exhaustive (rule 15), so the compiler lists every site; a new `Compressed` arm that is not a boundary must say why. |
| D6 | **The invariant** (a derived state, like an index): after every completed `GraphStorage` call, commit, rollback, load and replay, a stored **string** leaf at a path matching a rule of ANY of the element's labels (vertices: label set; edges: label) is `Compressed` iff `plain bytes >= min_bytes` AND the compressed form is at most 90 % of the plain size; every other stored value is plain. Non-string leaves at a rule path are left as they are. If two rules (different labels of one vertex) match the same path, the first matching label in label order wins (same rule as schema coercion). |
| D7 | **Write path** encodes: `add_vertex`/`add_edge`, `set_*_property`, `set_*_property_path` (also a write of an `Object` that contains a rule path below it), `apply_change_set`/`replay` (they go through the write path), GraphSON/GraphML import. Only the touched top-level key is re-encoded; with no rule for the element's labels it costs one map lookup (empty rules: one `is_empty` check). A **label change** (`add_vertex_label`/`remove_vertex_label`) re-encodes that vertex's rule paths (compress newly matched, decompress no longer matched). |
| D8 | **Explicit pass, no laziness**: `set_definition`/`remove_definition` of kind `compression` (also a redefinition with other parameters) re-encodes, in the same unit, every value of that `(element, label)` at the old and the new path: compresses what now matches, decompresses what no longer does. `g.recompress(name)` re-runs the pass (rebuilding the dictionary) on demand; it changes no logical data, so it emits no mutation: a unit holding only a `recompress` captures nothing and leaves `commit_seq` unchanged. |
| D9 | **Dictionary** (`dictionary: true`): LZ4 block compression with an external dictionary (`lz4_flex::block::compress_with_dict`/`decompress_into_with_dict`, already a dependency). The dictionary is built by the explicit pass from a deterministic sample of the rule's current values (id order, fill up to 64 KiB, the LZ4 window): no training algorithm in v1, just sampled text; it lives in the rule's in-memory state (`Arc`) and is never persisted (load rebuilds it in its own pass). Values written before a dictionary exists (rule defined on an empty label) are compressed without one until the next `recompress`/load. The implementation measures ratio and throughput with and without dictionary on a generated CV-like corpus and reports the numbers (no `cargo bench`; one `--release` measurement test marked `#[ignore]`). |
| D10 | **Undo**: the undo log keeps stored representations (compressed copies are cheap and self-contained); a restored property value is normalized under the rules current at that moment, and undoing a compression definition re-runs its pass, so rollback always ends in the invariant. `ChangeSet` derivation (`capture.rs`) decompresses before/after values; the explicit pass itself emits only the `SetDefinition` mutation. |
| D11 | **Load**: snapshot load (`load_outside_units`, store open, recovery, fork, restore, repair, `reload_in_place`), `GraphSource::from_*` loads: after the elements and the catalog are in place, ONE pass encodes all rule paths (and builds dictionaries). The merge checkpoint and every encoder write plain values (D5 via the codec). |
| D12 | **Codec** `lz4` only in v1 (`Codec` enum `#[non_exhaustive]`, `zstd` later); `lz4_flex` becomes a non-optional dependency of `graphersal` (pure Rust, wasm32-safe, already in the tree; justification per rule 10). Decompression errors (impossible for our own bytes, still) map to `GraphError`/`TraverserError`, never panic (rule 1). |
| D13 | **Diagnostics**: `MemoryStats` gets `compressed_values`, `compressed_plain_bytes`, `compressed_stored_bytes` (incl. dictionaries in a separate `compression_dictionary_bytes`); `property_bytes` counts the stored (compressed) size. `.profile()` unchanged. |
| D14 | **Authorization**: `Resource::Definition { kind: compression, name }` with Create/Update/Delete for define/drop/recompress, Read for listing; no `Data` request (logical data does not change). The saved-query read-only authorizer and `AccessPolicy::read_only()` deny defining. |

## API

Rust (`GraphTraversalSource`): `define_compression(name, CompressionDefinition)` / `set_definition(Definition::compression(name, def))`, `list_definitions(Some(DefinitionKind::COMPRESSION))`, `remove_definition`, `recompress(name)`, `compression_stats(name)`. `CompressionDefinition` has a builder (`CompressionDefinition::new(ElementKind::Vertex, "customer", "details.career.cv")?.with_min_bytes(512).with_dictionary(false)`). `GraphStorage` gains `recompress` and `compression_stats` (defaults: `Ok(false)`, `None`).

Rhai, both spellings: `g.define_compression(#{name, label, path, element?: "vertex"|"edge", codec?: "lz4", min_bytes?, dictionary?})`, `g.compressions()` (list of maps with the stats), `g.drop_compression(name)`, `g.recompress(name)`. `path` accepts a string (`"details.career.cv"`, parsed as a jpath here because it IS a path, documented) or a `jpath(..)`. Doc entries in `register/docs.rs`, surface snapshot updated. `g.memory_usage()` has the four new counters.

Python `Graph`: `define_compression(name, label, path, element="vertex", min_bytes=256, dictionary=True)`, `compressions()`, `drop_compression(name)`, `recompress(name)`.

Persisted payload (format spec 6.3/9.3/16): property map `{element: "vertex"|"edge", label, path (jpath text), codec, min_bytes, dictionary, ...extra}`; book `persistence/format.md` lists kind 3.

`graphersal store info` / `StoreInfo::definitions` count the catalog, `StoreInfo::definitions_by_kind` per kind (`definitions 2 (compression 1, query 1)`).

**Session, playground, dev server, MCP**:

- `graphersal-session` `compression.rs`: `list_compressions` (entries with `stats`), `get_compression`, `define_compression` (spec `{name, label, path, element?, codec?, minBytes?, dictionary?, replaces?}`; `{ok, rule}` or `{ok: false, errors: [{field, message, help?}]}`, the saved queries' `FieldError` shape), `drop_compression`, `recompress`, `compression_count`; `Session::info()` has `compressionRules` and `catalog` (counts per kind); `views::memory_usage` the new counters.
- **One catalog file for every kind**: `Session::save_catalog_text` / `load_catalog` (was `save_queries_text` / `load_queries`), wasm `saveCatalog` / `loadCatalog`, dev server `GET /api/catalog/export`, `POST /api/catalog/load` (was `/api/queries/export|load`), file name `graphersal-catalog-<stamp>.json`. Loading a compression rule runs its pass in the same commit.
- `graphersal-wasm`: `listCompressions`, `defineCompression`, `dropCompression`, `recompress`.
- Dev server: `GET /api/compression`, `POST /api/compression/define|drop|recompress`; a define or drop is a commit and lands in the change feed with its client; a `recompress` commits nothing.
- MCP: `list_compressions` (also `--mcp-read-only`), `define_compression`, `drop_compression`, `recompress` (refused read-only); the instructions mention them. The read-only authorizer now denies every catalog write (it denied only data and schema writes before, so a read-only `query` could define a saved query).
- Playground: ONE top-bar dropdown **Catalog ▾** (`js/components/catalog.js`, model `js/compression.js`): one item per catalog kind with its count (Saved queries, Compression; a later kind such as indexes is one more entry, no placeholder now), each opening its manager, plus **Save catalog (.json)** and **Load catalog…** (moved out of the Save menu; the start page's card is **Load catalog**). The Saved queries manager lists the queries with Run / Edit / Delete / New; the Compression manager lists the rules with their stats and has the form (name, element, label with the graph's labels as suggestions, path, min size, dictionary) with field errors, Recompress and Drop. The Query panel has no separate saved-queries menu: it would duplicate the Catalog manager.
 The Store menu shows the catalog per kind; the Statistics tab shows the compressed strings and dictionaries.

## Tests

`crates/graphersal/tests/all/compressed_property_tests.rs` (+ unit tests in `value/compressed.rs`, `graph/compression.rs`):

1. **Representation**: round trip (ASCII, multi-byte UTF-8, empty, exactly `min_bytes - 1`/`min_bytes`), incompressible random text stays plain (90 % rule), `size_of::<ElementProperty>()` unchanged, logical `==`/`Ord`/`Hash` between `Compressed` and `String` (unit).
2. **Differential read battery**: two graphs with the same data, one with rules (several rules: top-level `bio` and nested `details.career.cv` on `customer`, `notes` on edge `bought`, a rule on a second label of a multi-label vertex). The same list of queries on both must give identical results (`to_list`, `to_json`, table rendering): `values`, `value_map`, `element_map`, `properties`, jpath `values`, `has` eq / `containing` / `starting_with` / regex, folded `v().has(key, value)`, `order().by`, `dedup().by`, `group().by`, `project`, `path().by`, `select`, `subgraph`/`to_graph`, `infer_schema` (57 queries).
3. **Invariant** helper (`#[doc(hidden)] pub fn TraversalGraph::check_compression_invariant() -> Result<(), String>`) asserted after every step of: writes (`property`, folded `add_v`, `set_*_property_path` into the nested path, write of the whole parent object, overwrite with a short value -> plain, `remove_property`, `drop`, `mergeV`), label add/remove on a multi-label vertex (Rust API: the DSL has no label steps), define / redefine (other `min_bytes`) / drop / `recompress`.
4. **Memory**: `memory_usage()` new counters; a graph of 10k CV-like texts uses clearly less `property_bytes` with the rule than without.
5. **Transactions**: a failing traversal, `transaction` rollback mixing writes and a rule change, nested savepoint, `dry_run`: invariant holds and logical data equals the oracle.
6. **Change capture**: a commit hook sees plain before/after strings; defining a rule yields only `SetDefinition`; `recompress` alone commits nothing; replay and `apply_change_set` on a replica compress.
7. **Validation**: index in path, `[-1]`, empty path, unknown codec, `min_bytes` 0 / above `traversal.max_string_bytes`, duplicate `(element, label, path)`, bad name; nothing changed after the error.
8. **Persistence** (`definition_store_tests.rs`, every Store backend): `write_snapshot` holds plain text (written with `Codec::None` and searched for the text), reload -> invariant holds again; Store WAL replay, merge checkpoint, fork, backup + open, ZIP restore, repair, read-only open, `rollback_to`, attic restore.
9. **Exchange**: GraphSON/GraphML export of a compressed graph equals the export without rules; import into a graph with rules compresses.
10. **Schema**: `set_schema` open scan with `maxLength` and `pattern` on a compressed field (violation found / passes), a write still validated and compressed.
11. **Permissions**: define/drop/recompress denied under `AccessPolicy::read_only()` and inside a saved query; listing allowed.
12. **Randomized**: random units (writes, label changes, rule define/drop/recompress) committed or rolled back on a graph with rules vs. an oracle without; the invariant and logical equality after every step (fast budget in `cargo test`; `COMPRESSION_FUZZ_CASES` raises it).
13. **Rhai and Python**: the DSL functions in both spellings, Python `tests/test_graph.py`.
14. **Measurement** (`#[ignore]`, `--release`): ratio and throughput, LZ4 with and without dictionary, on a generated CV corpus (2-20 KB texts); numbers in the book page.

Front ends: session unit tests (`compression.rs`: define/list/recompress/rename/drop, field errors), wasm (`lib.rs` and `tests/web.rs` under wasm-bindgen), dev server `compression_rules_over_http`, MCP read-only lists (`mcp.rs` unit test, `mcp_read_only_token_and_off`), CLI `store info` per kind, playground `tests/compression.test.mjs` (dropdown items and counts, Store menu text, rules table, form draft/spec/errors, label suggestions), `statistics.test.mjs`, `http-engine.test.mjs`, `save-load.test.mjs` (the catalog file and the restore point).

## Docs

Book `users_guide/compressed_properties.md` (what, when it pays off, cost model, the explicit pass, dictionaries, the API table across front ends, the measured numbers), `SUMMARY.md`, `persistence/format.md` kind 3, `memory_and_profiling.md`, `web_playground.md` (the Catalog), `saved_queries.md`, `persistence/dev_server.md` (endpoints, MCP tools), `persistence/store_cli.md`, `playground/README.md`, CLAUDE.md (**Compressed properties** paragraph, the definition kind, the catalog file, the MCP tools).

## Implementation notes

Where the code differs from (or refines) the decisions above:

- **D3/D12, errors**: every compressed value is verified once when it is made (decompressed and compared with its input); its bytes are immutable afterwards, so a read cannot fail and the read API is infallible (`with_text`, `text`, `logical` return values, not `Result`). Equality, hashing and ordering could not return an error anyway. A value that still failed to decompress (memory corruption) reads as `""` and is logged; there is no panic.
- **D5, the boundary**: besides `logical()` there are `to_logical()` (owned copy), `into_logical()`, `holds_compressed()`, `is_text()` and `text() -> Option<Cow<str>>`; `with_text` returns `Option<R>` (`None` for a non-string). The `From<ElementProperty>`/`From<&ElementProperty>` conversions to `TraverserValue`, `into_node_property`, the physical codec, the persist codec and change capture (`Mutation::make_logical`) decompress, so a stored value reaches no consumer compressed. Helpers on recursive paths are out of line (`#[inline(never)]`): the debug-build 2 MiB test stack had no room for larger frames in the deep-value tests.
- **D6**: compressed values live only below objects (a rule path has names only), never in arrays.
- **D9, dictionary sample**: up to 1 KiB of each value in id order until 64 KiB, so many values contribute; values decompress with the `Arc` dictionary they carry.
- **D10**: a restored value is normalized when it is restored (property, dropped vertex/edge image, label set); an undone rule change re-runs the pass for the old and new label.
- **D11**: the load pass runs at the end of `read_snapshot` (so every store path: open, recovery, fork, backup, repair, `reload_in_place`) and of a salvage; a replayed WAL compresses through the write path; `replay_lenient`'s direct catalog insert runs the pass too.
- **D14**: also enforced in the dev server's MCP read-only mode, whose authorizer now denies `Create`/`Update`/`Delete` on definitions of every kind.
- Validation of a duplicate `(element, label, path)` is `DefinitionError::Conflict { name, existing, reason }` with a help naming `drop_compression`.
- The session's JSON uses camelCase (`minBytes`) like the rest of the session; the DSL, Python and the persisted payload use `min_bytes`; the MCP tool accepts both.
