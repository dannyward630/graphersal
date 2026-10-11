# Generic front ends: the DSL, saved queries, the session and the Store over a foreign `GraphStorage`

Status: IMPLEMENTED in 0.1.0 (steps E02-E09 of section 8). The design questions and their resolution are
recorded in section 9 and folded into the design below. The user documentation is the book page
`for_developers/custom_storage.md`. When this design and the code disagree, the code wins.

Goal: in 0.1.0 the Rhai DSL, saved queries, `graphersal-session` and the persist `Store` work over a
`GraphStorage` other than `TraversalGraph`, so that a server in another repository can bring its own
in-memory storages. The engine (`GraphTraversalSource<'g, S>`) was already generic. Everything above it was
bound to `pub type Graph = parking_lot::RwLock<Box<TraversalGraph>>`.

Module paths below are the paths at design time; the public facade modules may name the items differently.
The concepts do not depend on the paths.

## 1. Summary of the proposal

- **Hybrid (approach c).** The engine stays generic and statically dispatched. The dyn boundary sits at the
  front ends, with one virtual call per *operation*: a terminal, a schema method, a catalog call, a view. It is
  never one call per element.
- **`Graph<S = TraversalGraph>` becomes a struct.** It replaces the `Graph` alias and the public
  `parking_lot::RwLock` re-export. It is a lock newtype with `read()`/`write()` guards that deref to `S`, so
  `Arc<Graph>` keeps meaning "a shared `TraversalGraph`".
- **`AnyGraph` (object-safe, sealed).** It is blanket-implemented for every `Graph<S>` and is the only thing the
  script layer and the session hold: `Arc<dyn AnyGraph>`. Each method locks, monomorphises the engine for `&S`
  or `&mut S` *inside* the impl, and returns storage-independent data. The script AST (`Vec<Step>`) is already
  storage-independent, so the 728 registered `ScriptGraphTraversalSource` functions are registered once. They
  are not registered once per storage type.
- **New `GraphStorage` methods with defaults (rule 15):** `mark`, `set_unit_metadata`, `discard_unit`. A new
  opt-in trait **`PersistentStorage: GraphStorage`** covers what the Store needs: lineage id, commit position,
  id sequences, a load sink outside units, clear, and the journal slot.
- **Stays `TraversalGraph`-only, documented as such:** scratch/private graphs (`GraphSource::*`, `__`, `_g`,
  `subgraph()`, `toGraph()`, `schema.toGraph()`), compressed properties, the undo log and its change-capture
  derivation, the snapshot encoder's petgraph-order fast path, and the merge checkpoint's and salvage's internal
  working graphs (they never touch the host's storage).
- **Measured cost of the rejected per-element facade (approach b):** +50-58 % on adjacency-heavy traversals and
  +5-15 % on property filters (section 3.4). The hybrid's cost is one indirect call per terminal.

## 2. Inventory: what each layer needs from the graph

The counts are lines from `grep` at design time (non-test code). Each entry says how the use is resolved: by the
trait **(T)**, by the new persistence trait **(P)**, by the dyn boundary **(D)**, or kept **TraversalGraph-only
(K)**.

### 2.1 Script layer (`crates/graphersal/src/script/`, 60 lines in 8 files; `terminals.rs` 24, `eval.rs` 21)

| Need | Where today | Resolution |
|---|---|---|
| Run a plan read-only or mutating: lower `Vec<Step>` with `ast_convert_to_gts::<&TraversalGraph>` or `<&mut TraversalGraph>` under `g.read()` or `g.write()`. The terminals are `to_list`, `next`, `to_typed_list`, `into_list_with_graphs`, `profile(_with)`, `execute(_with)`, the visualizers and `to_graph`. | `terminals.rs`, 6 copies of the lock-and-fallback pattern | (D) `AnyGraph::run_plan(&ScriptPlan, Terminal) -> TerminalOutput`; the lowering is monomorphised in the blanket impl |
| Fallback empty graph when `g` is `None` | `terminals.rs` (`TraversalGraph::new()`) | (K) a process-wide empty scratch `Graph<TraversalGraph>` |
| Schema methods (get/set/patch/validate/validate_patch/infer/diff) | `source/mod.rs` via `GraphTraversalSource<S>` | (T) already generic; (D) one `AnyGraph` method per call |
| Catalog: saved queries (`define_query(s)`, `query`, `queries`, `get/drop/move/describe_query`), compression rules | `saved_query.rs`, `compression.rs`; `define_queries` uses the inherent `TraversalGraph::transaction` | (T) generic `transaction` (2.5); (D) `definitions`, `definition`, `set_definitions` (one unit), `remove_definition`, `recompress`, `compression_stats` |
| `statistics()`, `memory_usage()` | `source/mod.rs` | (T) already trait methods; (D) cloned out (`Option<GraphStatistics>`, `Option<MemoryStats>`) |
| `g.mark(name)` | inherent `TraversalGraph::mark`, goes to the journal slot | (T) new `GraphStorage::mark`, default `GraphError::MarkUnavailable` |
| `import_graphml`/`import_graphson`, `export_graphml`/`export_graphson` | `io::*` (already generic over `S`) | (D) `AnyGraph::import_*`/`export_*` taking `&mut dyn BufRead` / `&mut dyn Write` |
| `export_snapshot(path)` | inherent `TraversalGraph::write_snapshot` | (P) generic snapshot writer (2.4); (D) `AnyGraph::write_snapshot`, `Unsupported` unless `S: PersistentStorage` (see 5.3) |
| `eval_value_atomic` / `eval_value_dry_run`: the private-lock trick. The shared write lock is held, `std::mem::take` moves the graph into a private `Arc<Graph>`, the script runs on it, the graph is swapped back (`RestoreGraph`, also on unwind), and returned traversals are retargeted (`Arc::ptr_eq`). | `eval.rs` | (D) `AnyGraph::run_unit(&self, UnitKind::{Atomic, DryRun}, &mut dyn FnMut(&Arc<dyn AnyGraph>) -> ..)`, which does the move inside the impl; needs `S: Default` (5.4) |
| `GraphSource::*` private graphs (`empty`, `tinkerpop_modern`, `file_tree`, `file`), `__`, `_g`, `schema.toGraph()`, `subgraph()`/`cap()`/`toGraph()` results, `TraverserValue::Graph(Arc<Graph>)`, `ListItem::Graph` | `source/mod.rs`, `traversal/side_effect.rs`, `value/mod.rs` | (K) always `Graph<TraversalGraph>`, wrapped as `Arc<dyn AnyGraph>` where the script holds them |
| `withSideEffect(name, graph)` subgraph target (`ScriptGraphTarget.graph: Arc<Graph>`) | `source/mod.rs`, `step_with_side_effect.rs` | (K) only a `Graph<TraversalGraph>` target (downcast through `AnyGraph::as_any`); a foreign target fails with `GraphError::Unsupported` (decision 3) |
| Access, authorizer, presets, cancel token, limits, the Rhai engine and registry | `access.rs`, `engine.rs`, `register/` | unchanged: storage-independent |

### 2.2 Session (`crates/graphersal-session/src/`, ~80 lines in 10 files)

| Need | Where | Resolution |
|---|---|---|
| Hold the served graph, run scripts (`eval_value_atomic`), cancel | `session.rs` (`graph: Arc<Graph>`) | (D) `Arc<dyn SessionGraph>` (a session-crate trait, `SessionGraph: AnyGraph`, blanket for `Graph<S>`; trait upcasting is stable since 1.86, MSRV is 1.88) |
| Read views: `schema_view`, `graph_view` (CompactGraph), `statistics_view`, `memory_usage`, `element_of`/`node_of`/`edge_of`, `neighbourhood`, `label_sample`, `meta_graph` | `views.rs`, `display.rs`, `selection.rs` take `&TraversalGraph` | (T) rewrite as `fn x<S: GraphStorage>(g: &S, ..)` (they use only the read API); (D) one `SessionGraph` method each |
| Edit/create/drop with preview: `dry_run` + `transaction` | `edit.rs`, `create.rs` | (T) generic `Transactional` (2.5); `dry_run` needs `discard_unit` (2.3) |
| Saved queries and compression managers, the catalog file (`carry_catalog`) | `queries.rs`, `compression.rs` | (T) generic `transaction` + catalog trait methods |
| Load a graph (GraphML/GraphSON/`.gsnap`, with schema) into a NEW graph; `load_shared`, `replace` | `session.rs` (`read_snapshot`, `import_with_schema`) | (D) a `GraphFactory` (object-safe: `empty() -> Arc<dyn SessionGraph>`, `create_store`, `open_store`) chosen by the host; the default builds `TraversalGraph`; IO is generic already; `.gsnap` needs `PersistentStorage` (P) |
| Memory stores (every loaded graph becomes a `Store` on a `MemDir`), the Store menu, the database list | `store.rs` (`Store::create_with(MemDir, graph)`, `Arc<Store>`) | (D) `Box<dyn SessionStore>` (session trait, blanket for `Store<S>`) |
| Layout | `layout.rs` | works on `CompactGraph`, unchanged |

### 2.3 Persist (`crates/graphersal/src/persist/`, ~92 lines; `Store` has ~20 touchpoints on its live graph)

| Need | Where | Resolution |
|---|---|---|
| Lineage id `graph_id()`, `with_graph_id`, commit position `last_commit_seq`/`last_committed_at`, `with_last_commit_seq`, id sequences | `TraversalGraph` inherent (`graph/mod.rs`) | (P) `PersistentStorage::{graph_id, set_graph_id, last_committed_at, id_sequences, raise_position}` (`raise_id_sequences` exists on the trait already) |
| Journal slot: `undo.hooks.has_journal()`, `set_journal(Box<dyn JournalHook>)`; the journal runs after every user hook's `before_commit` and nothing fallible follows it | `journal.rs` | (P) `PersistentStorage::attach_journal(JournalSlot)`; a PUBLIC `CommitHooks` helper that a foreign storage embeds and calls at its commit point, so the ordering contract is implemented once (5.2) |
| Marks | `hooks.rs` `CommitHooks::mark`, `Store::mark` | (T) `GraphStorage::mark` (the storage forwards to its embedded `CommitHooks`) |
| Snapshot write from the live graph: `SnapshotEncoder::new(&TraversalGraph)` sorts `(id, NodeIndex)`, reads petgraph weights, `graph.definitions`, `graph.schema` | `encoder.rs`, `snapshot.rs`, `snapshot_dir.rs`, `from_memory.rs`, `Store::create_with`, `checkpoint_from_graph` | (P) the encoder takes `&S` and sorts `(id, S::VertexId)` through the read API; TraversalGraph keeps a petgraph fast path (K) only if E06b measures a difference |
| Snapshot read: `insert_vertex_raw`/`insert_edge_raw`, `index.get_node`, `*graph.schema.write() = ..`, `definitions.insert`, `compression_load_pass` | `snapshot.rs` | (P) load sink `load_vertex`/`load_edge`/`load_schema`/`load_definition`/`finish_load` (outside units, unlogged, no hooks, no schema scan) |
| WAL replay: `replay_verified` (before images checked), `replay_lenient` (salvage) | `recover.rs`, `damage.rs` | (P) generic replay over the trait write methods outside units plus `raise_position`; before images are verified through the read API. Salvage stays on an internal `TraversalGraph` (K): it writes a NEW store, so it never needs the host's type |
| Merge checkpoint: delta `TraversalGraph` decoded from base chunks + WAL | `store/merge.rs` | (K) unchanged: it never reads the live graph |
| `reload_in_place` (rollback, attic restore): rebuild, swap inside the lock, adopt user hooks | `store/mod.rs` | (P) `PersistentStorage::clear()` + load into the SAME instance, so hooks stay (no hook transfer API needed) |
| Guard hooks: read-only, damage, maintenance, backup (vetoing `CommitHook`s) | `store/mod.rs` | (T) `register_commit_hook` (exists) |
| `Store::graph() -> &Arc<Graph>` | `store/mod.rs` | `Store<S>::graph() -> &Arc<Graph<S>>` (5.5) |

### 2.4 Units, hooks and replay used by every layer

| Need | Today | Resolution |
|---|---|---|
| `transaction(|g| ..)`, `transaction_with(metadata, ..)` | inherent on `TraversalGraph` | (T) a blanket extension trait `Transactional` for every `GraphStorage` (`begin_unit`/`commit_unit`/`rollback_unit` + `close_on_unwind`); metadata through a new `set_unit_metadata` (default: ignored) |
| `dry_run(|g| ..) -> (T, ChangeSet)` | inherent, uses `discard_undo_unit` + `derive_mutations_since` | (T) new `GraphStorage::discard_unit(mark) -> Result<ChangeSet, GraphError>` ("roll back without hooks, tell me what it changed"), default `Unsupported`; `Transactional::dry_run` on top |
| `apply_change_set`, `unit_changes`, `register_commit_hook`, `last_commit_seq`, `capabilities`, `statistics`, `memory_usage`, `execution_policy`, schema, catalog | trait methods already | unchanged |
| Building a `ChangeSet` (a foreign storage with `change_capture` must produce them) | `ChangeSet::new` is `pub(crate)`; `Mutation` variants are `#[non_exhaustive]` | public constructors (`ChangeSet::builder(stamp)`, one constructor per `Mutation` kind) |

## 3. Approaches compared

### 3.1 (a) Generic `S` through the script layer

`ScriptGraphTraversalSource<S>`, `graph_scope::<S>`, one registry/engine per storage type.

- Rhai registers concrete types. 728 of the 3569 surface lines take `ScriptGraphTraversalSource`, so every
  storage type duplicates them. That is one more `Registry` module set per thread, one more `Engine` per type,
  and each closure monomorphised again, which costs build time and binary size.
- Every `Dynamic::is::<ScriptGraphTraversalSource>()` site (rendering, materializing, retargeting, saved-query
  checks, the session visualizer) would have to know every host type. graphersal cannot list a host's types,
  so the host would register its own copies, and the surface snapshot would then hold per-host entries.
- One script mixes graphs. `g` is the host storage, while `GraphSource::empty()`, `__`, `_g`, `subgraph()` and
  `toGraph()` are `TraversalGraph`. These would be two Rhai types with the same method names. Passing `__.out()`
  into `g.where(..)` or comparing sources would then need conversions everywhere.
- Gain: none at run time over (c), because (c) dispatches statically below the terminal as well.

Verdict: rejected. It has the highest build-time and complexity cost and no runtime benefit.

### 3.2 (b) An object-safe facade under the engine (`dyn GraphHandle`, boxed iterators)

`GraphStorage` is not dyn-compatible: it has RPITIT iterators and associated `VertexId`/`EdgeId` types. A facade
would fix the id types (e.g. a `u64` or an opaque handle) and box every iterator. The engine would be compiled
once, for `dyn`.

- It is the simplest for the front ends, which see one concrete type.
- Every element access becomes an indirect call, and every adjacency expansion allocates a box. The
  measurement is in 3.4.
- Fixed id types break storages whose natural id is not a small integer (see `custom_id_storage_tests.rs`), or
  force a mapping table.
- Build time is lower than today's, but the price is paid on every query of every user, including
  `TraversalGraph` users if the front ends used the facade for it too.

Verdict: rejected for the engine path. It remains the right shape for coarse, non-hot operations, which is
what (c) uses it for.

### 3.3 (c) Hybrid: generic engine, dyn boundary per operation

The front ends hold `Arc<dyn AnyGraph>`. `AnyGraph` methods are coarse (run a plan, read the schema, define
queries, build a view). The blanket `impl<S> AnyGraph for Graph<S>` contains the generic code, so the engine is
monomorphised for `&S` and `&mut S` once per storage type the host actually uses. That is the same thing that
already happens for `&TraversalGraph` and `&mut TraversalGraph`.

- Runtime cost: one indirect call per terminal or method, with no per-element dyn.
- Build cost: one engine instantiation pair per storage type the host instantiates. graphersal itself still
  compiles only the `TraversalGraph` pair, so its build is unchanged. This is measured in the proof (E05b)
  with `cargo build --timings` on `graphersal-storage-tests`, and is NOT a bench.
- Rhai: one registry and one surface for all storages, and the surface snapshot does not change.
- Mixed graphs in one script are free: every source holds `Arc<dyn AnyGraph>`, whether the host's or a scratch
  `TraversalGraph`.
- Cost: `AnyGraph` grows with the front ends' needs. It is sealed (only `Graph<S>` implements it), so adding a
  method is never a breaking change, and methods tied to a feature (`script`, `persist`, `io`) can be
  feature-gated without breaking additivity. No outside crate implements it.

Verdict: **chosen.**

### 3.4 Measurement (cheap micro test, not the bench suite)

Setup: a scratch crate outside the repository, release build on the developer Mac, best of 15 runs after 2
warm-up runs, on `GraphSource::large_110k()` (about 111k vertices and 111k edges, a tree of fan-out 10). It
compares `GraphTraversalSource::new(&TraversalGraph)` with `GraphTraversalSource::new(DynRead(&dyn ObjGraph))`.
`ObjGraph` is the facade of approach (b): the read subset of `GraphStorage` with `Box<dyn Iterator>` returns,
implemented for `TraversalGraph`. `DynRead` is a `GraphStorage` forwarding every call through the vtable. The
same plan and the same optimizer ran in both cases. Two runs agreed within 2 %.

| Traversal | static | dyn facade | dyn/static |
|---|---:|---:|---:|
| `g.v().out().out().count()` | 4.44 ms | 6.99 ms | **1.56** |
| `g.v().both().both().count()` (1.44 M traversers) | 10.78 ms | 16.22 ms | **1.52** |
| `g.v().has("level", "4").count()` | 2.65 ms | 2.78 ms | 1.05 |
| `g.v().out().has("level", "4").count()` | 6.66 ms | 7.61 ms | 1.14 |
| `g.v().out().id().dedup().count()` | 11.82 ms | 13.00 ms | 1.11 |

Reading: adjacency expansion (a boxed iterator per `out_edges` call plus a vtable call per edge) costs about
+55 %, and property-heavy steps cost +5-15 %. The hybrid's boundary costs one indirect call per terminal. A
traversal costs at least microseconds, so that call is not measurable and was not measured further. The
measurement code is reproduced in the appendix (section 10) so it can be re-run.

**Re-measured after the script layer moved onto `AnyGraph` (E04b, measured in E05b).** The same scratch crate (section 10, plus the read-only
mutators the trait gained since: `add_vertex_label`/`remove_vertex_label`, `unreachable!()`), same Mac, same
protocol (release, best of 15 after 2 warm-ups, two runs within 3 %), with a third column: the `Forwarding`
storage of `graphersal-storage-tests` (own newtype handles, every method forwarded to a `TraversalGraph`) read
through `GraphTraversalSource::new(&*Graph<Forwarding>::read())`, i.e. a host storage behind the hybrid's
`Graph<S>`. All three gave the same results.

| Traversal | static | dyn facade | Forwarding | dyn/static | Forwarding/static |
|---|---:|---:|---:|---:|---:|
| `g.v().out().out().count()` | 4.48 ms | 7.00 ms | 4.54 ms | 1.56 | 1.01 |
| `g.v().both().both().count()` | 11.47 ms | 16.99 ms | 11.30 ms | 1.48 | 0.99 |
| `g.v().has("level", "4").count()` | 2.59 ms | 2.53 ms | 2.64 ms | 0.97 | 1.02 |
| `g.v().out().has("level", "4").count()` | 6.34 ms | 7.84 ms | 6.58 ms | 1.24 | 1.04 |
| `g.v().out().id().dedup().count()` | 11.55 ms | 12.77 ms | 11.63 ms | 1.11 | 1.01 |

Reading: the facade's adjacency cost is unchanged (+48-56 %); a storage behind the hybrid runs within 1-4 % of
`TraversalGraph` itself (the second run: 1.02-1.04), i.e. the newtype handles and the forwarding are inlined away
by the monomorphised engine. Build cost: `cargo build --timings -p graphersal-storage-tests` on a clean target
dir (dev profile, no features) took **9.9 s** in total, of which the `graphersal` unit **5.2 s** and the
`graphersal-storage-tests` lib 0.5 s; building its test targets on top (`--all-targets`, where the conformance
suite instantiates the engine for `Forwarding` and the `TraversalGraph` wrappers) took the `conformance` test
unit 2.8 s. `graphersal`'s own build does not instantiate anything for a foreign storage.

## 4. What becomes a trait method, what stays `TraversalGraph`-only

### 4.1 New `GraphStorage` methods (all with defaults, rule 15)

| Method | Default | `TraversalGraph` | Why on `GraphStorage` |
|---|---|---|---|
| `mark(&mut self, name: &str) -> Result<Mark, GraphError>` | `Err(MarkUnavailable)` | its journal slot (today's inherent `mark`, removed) | `g.mark` in the DSL goes through `AnyGraph`, which cannot ask "is `S: PersistentStorage`" without specialization |
| `set_unit_metadata(&mut self, CommitMetadata)` | ignored | `undo.metadata` (merge rules of `transaction_with`) | generic `transaction_with` |
| `discard_unit(&mut self, mark: UnitMark) -> Result<ChangeSet, GraphError>` | rollback, then `Err(Unsupported)` | `derive_mutations_since` + `discard_undo_unit` (no hooks, `commit_seq` 0) | generic `dry_run`; requires `change_capture` (debug assertion like the existing one) |

Plus the blanket read-only view `impl<G: GraphStorage> GraphStorage for &G` replaces today's concrete
`impl GraphStorage for &TraversalGraph`. It keeps the same behaviour: reads forward, writes fail with
`Unsupported`, it claims no capabilities, and it refuses hooks. The read terminals need `&S: GraphStorage` for
every host `S`, and a host cannot get it otherwise without writing the impl itself.

Contract additions, each with a conformance item in `graphersal-storage-tests`:
- A mutation outside any unit records nothing and calls no hook. `TraversalGraph` already behaves this way.
  Generic replay relies on it.
- `discard_unit` leaves the data, schema and catalog as before `begin_unit`, and keeps the id sequences.

### 4.2 New trait `PersistentStorage: GraphStorage + Send + Sync + 'static` (EXPERIMENTAL like `GraphStorage`)

This trait is what the Store and the snapshot functions need. A storage that does not implement it can still
be served by the DSL and the session, but it cannot be stored. The methods:

- `graph_id(&self) -> u128`, `set_graph_id(&mut self, u128)`.
- `last_committed_at(&self) -> i64`, `id_sequences(&self) -> (u64, u64)`.
- `raise_position(&mut self, commit_seq: u64, committed_at: i64)`: raise only, never lower (the `replay`
  rule). Today this is `with_last_commit_seq` plus the time.
- The load sink, used outside units only (unlogged, no hooks, no schema validation):
  - `load_vertex(&mut self, id, labels, properties) -> Result<Self::VertexId, GraphError>`
  - `load_edge(&mut self, id, label, from_id: &str, to_id: &str, properties) -> Result<Self::EdgeId, GraphError>`
  - `load_schema(&mut self, GraphSchema)` (no scan)
  - `load_definition(&mut self, Definition)`
  - `finish_load(&mut self)`: `TraversalGraph` runs its compression load pass and shrinks here; the default
    does nothing.
- `clear(&mut self)`: empties the data, schema and catalog, and KEEPS the registered hooks and the journal
  slot. `reload_in_place` uses it, so no API to transfer hooks is needed.
- `attach_journal(&mut self, JournalSlot) -> Result<(), GraphError>`. `JournalSlot` is a public opaque type
  that wraps the crate-internal `JournalHook`. A storage keeps it in its `CommitHooks` (5.2).
- `replay(&mut self, &ChangeSet, verify: bool) -> Result<(), String>`. The default runs over the trait's write
  methods outside units plus `raise_position`; `verify` checks before images through the read API.
  `TraversalGraph` overrides it with today's raw path (`replay_verified`). The lenient form stays
  `TraversalGraph`-only (K), because salvage runs on an internal graph.

Precondition, checked at attach: `capabilities()` must claim `atomic` and `change_capture`. Without them a
journal cannot honour a veto, and `Store::create_in`/`open_in` fail with `PersistError` naming the missing
capability.

### 4.3 Stays `TraversalGraph`-only (documented on the item and in the new book page)

- Scratch and private graphs: `GraphSource::*`, `__`, `_g`, `subgraph()`/`cap()`/`toGraph()`,
  `schema.toGraph()`, `TraverserValue::Graph`. These graphs are created by the script, not by the host
  (decision 2: they stay `TraversalGraph`; a host-chosen scratch type is not planned).
- Compressed properties (`ElementProperty::Compressed`, `CompressionRules`, `compression_load_pass`).
  Decision 4: a foreign storage keeps compression definitions in its catalog like any other definition
  (defined, listed, persisted, dropped). Nothing applies them: `recompress` fails with `GraphError::Unsupported`,
  the compression pass (`compression_after_change` on define/drop/undo, the load pass) never runs for it, and
  `compression_stats` stays `None`. The session's compression manager lists and edits the rules and says
  "not supported by this storage" for recompress and the statistics. Other storage types are a later, separate
  question.
- The undo log, `derive_mutations_since` and the shadow-based change capture. Decision 5: a foreign storage
  brings its own capture and builds `ChangeSet`s with the new public constructors; there is no reusable undo-log
  or capture component in 0.1.x.
- ~~The snapshot encoder's petgraph-order fast path (only if E06b measures one; otherwise it is removed).~~
  Removed (E06b): on `GraphSource::large_110k()` (release, best of 5, scratch program, interleaved
  runs on a busy machine) the generic walk over `&S` measured write LZ4 50.5 ms vs 50.7 ms petgraph order,
  write `Codec::None` 35.0 ms vs 35.5 ms, read 54.1 ms vs 53.2 ms: within noise, so there is no fast path.
- The merge checkpoint's delta graph, salvage's working graph, and repair. All three are internal and write
  files, never the host graph.
- Sample graphs (`tinkerpop_modern`, `file_tree`, `large_110k`) and `TraversalGraph::with_execution_policy`.
  The trait method `execution_policy()` already makes the policy generic.

## 5. How a host plugs a storage in

### 5.1 The shared graph type

```rust
// replaces `pub type Graph = parking_lot::RwLock<Box<TraversalGraph>>` and `pub use parking_lot::RwLock`
pub struct Graph<S: GraphStorage = TraversalGraph> { /* parking_lot::RwLock<S>, private */ }

impl<S: GraphStorage> Graph<S> {
    pub fn new(storage: S) -> Self;
    pub fn read(&self) -> GraphRead<'_, S>;        // Deref<Target = S>
    pub fn write(&self) -> GraphWrite<'_, S>;      // DerefMut
    pub fn try_write(&self) -> Option<GraphWrite<'_, S>>;
    pub fn get_mut(&mut self) -> &mut S;
    pub fn into_inner(self) -> S;
}
```

- `Arc<Graph>` in type position keeps meaning `TraversalGraph`, because default type parameters apply there.
  Expressions get `S` from the argument (`Graph::new(storage)`).
- The `Box` goes away. Call sites change from `&mut **g.write()` to `&mut *g.write()`, and from
  `RwLock::new(Box::new(x))` to `Graph::new(x)`. That is 33 files outside `src`, all mechanical.
- `parking_lot` leaves the public API; the guard newtypes hide it.

### 5.2 The dyn boundary

```rust
/// Object-safe, sealed: implemented only by `Graph<S>`. What the front ends hold.
pub trait AnyGraph: Send + Sync + 'static + sealed::Sealed {
    fn as_any(&self) -> &dyn std::any::Any;                 // downcast to Graph<S>
    fn storage_type_name(&self) -> &'static str;            // diagnostics
    fn capabilities(&self) -> StorageCapabilities;
    fn last_commit_seq(&self) -> u64;
    fn statistics(&self) -> Option<GraphStatistics>;
    fn memory_usage(&self) -> Option<MemoryStats>;
    // schema (the seven methods), catalog (list/get/set-in-one-unit/remove/recompress/stats), mark
    // io (feature `io`): import_graphml/graphson(&mut dyn BufRead, presets), export_*(&mut dyn Write, ..)
    // persist (feature `persist`): write_snapshot(&mut dyn Write) -> Unsupported unless S: PersistentStorage (5.3)
    // script (feature `script`):
    fn run_plan(&self, plan: &ScriptPlan<'_>, terminal: Terminal) -> TerminalOutput;
    fn run_unit(&self, kind: UnitKind, body: &mut dyn FnMut(&Arc<dyn AnyGraph>) -> UnitBody) -> UnitResult;
}

impl<S> AnyGraph for Graph<S> where S: GraphStorage + Default + Send + Sync + 'static { .. }

/// What every front-end entry point accepts: `Arc<Graph<S>>` and `Arc<dyn AnyGraph>`.
pub trait IntoAnyGraph { fn into_any_graph(self) -> Arc<dyn AnyGraph>; }
```

- `ScriptPlan` carries what the terminals read today: the steps, the options, `access`, `private`, and the
  mutating flag. `Terminal`/`TerminalOutput` are the terminal kinds and their storage-independent results
  (materialized `ElementProperty`, `ListItem`, `TraversalMetrics`, `Execution`, `RenderedText`, a scratch
  graph). Both are crate-internal types, possible because the trait is sealed.
- `run_unit` is the private-lock trick moved inside the impl. It takes the write lock, uses
  `std::mem::take(&mut *guard)` to move `S` into a private `Arc<Graph<S>>` (hence `S: Default`), runs the body
  with it as `Arc<dyn AnyGraph>`, and swaps back on return and on unwind. The atomic variant uses
  `begin_unit`/`commit_unit`/`rollback_unit`; the dry-run variant uses `discard_unit`. Retargeting compares
  `Arc` data pointers.

### 5.3 What `write_snapshot` and `g.export_snapshot` do without specialization

`AnyGraph` cannot test `S: PersistentStorage`. The blanket impl therefore takes an optional function pointer,
filled by a second constructor: `Graph::new_persistent(s)` where `S: PersistentStorage` sets it, and
`Graph::new(s)` leaves it `None`. `Store` always builds its graph with `new_persistent`, and so does every
`Graph<TraversalGraph>` that graphersal constructs: `GraphSource::*` and the scratch graphs. Today's
`g.export_snapshot` on them therefore keeps working. This is the one place where the trick is needed. Every other persistence entry point is generic over `S: PersistentStorage`
directly.

As implemented (E06b): `Graph::new_persistent` stores the writer and a whole-script unit's private graph inherits
it (`Graph::new_like`). Instead of sweeping every `Graph::new(TraversalGraph)` site, `AnyGraph::write_snapshot`
keeps a downcast to `Graph<TraversalGraph>` as the fallback when no writer is stored: a `TraversalGraph` is always
persistent, so a host's plain `Graph::new(TraversalGraph)` must not silently lose `export_snapshot`. Any other
storage wrapped by `Graph::new` is `Unsupported`.

### 5.4 Signatures (after the change)

```rust
// script (graphersal::script)
pub fn graph_scope<'a>(g: impl IntoAnyGraph) -> Scope<'a>;
pub fn graph_scope_with_options<'a>(g: impl IntoAnyGraph, options: &[(&str, ElementProperty)],
                                    authorizer: Arc<dyn Authorizer>) -> Scope<'a>;
pub fn eval_value(graph: impl IntoAnyGraph, script: &str) -> ..;              // and every eval_* entry
pub fn eval_value_atomic(graph: impl IntoAnyGraph, ..) -> ..;
pub fn engine_with_options(limits, authorizer, options) -> Engine;           // unchanged: no graph in it
impl ScriptGraphSource { pub fn graph(g: impl IntoAnyGraph) -> ScriptGraphTraversalSource; }

// session (graphersal_session)
impl Session {
    pub fn new() -> Self;                                         // TraversalGraph, as today
    pub fn with_storage(factory: Arc<dyn GraphFactory>) -> Self;  // the host's storage for loads and stores
    pub fn load_shared(&mut self, graph: impl IntoSessionGraph, name: &str, kind: &str) -> Value;
    pub fn graph(&self) -> &Arc<dyn SessionGraph>;                // was &Arc<Graph>
}
pub struct StorageKind<S>;   // implements GraphFactory for S: GraphStorage + Default (+ PersistentStorage for stores)

// persist
impl Store<TraversalGraph> {                                       // non-generic: inference needs no annotation
    pub fn open(dir: impl IntoStoreDir, options: StoreOptions) -> Result<Self, PersistError>;
    pub fn create(..) / create_with(dir, TraversalGraph, options) / open_backup(..) ..
}
impl<S: PersistentStorage + Default> Store<S> {
    pub fn open_in(dir, options, empty: impl Fn() -> S + Send + Sync + 'static) -> Result<Self, PersistError>;
    pub fn create_in(dir, graph: S, options) -> Result<Self, PersistError>;
    pub fn graph(&self) -> &Arc<Graph<S>>;
    // every other method: unchanged signature, forwarded to the non-generic core
}
```

Why `open`/`create` stay on `impl Store<TraversalGraph>`: a default type parameter is not used for inference
in expressions. `Store::open(..)` on a generic impl would need a turbofish at every call site.

### 5.5 Store internals

`Store<S> { core: Arc<StoreCore>, graph: Arc<Graph<S>> }`:
- `StoreCore` is crate-internal and non-generic. It holds today's `Shared` with the live graph as
  `Arc<dyn StoreGraph>`, a crate-internal object-safe trait: `last_commit_seq`, `mark`,
  `write_snapshot_live`, `reload(&Recovery)`, `attach_journal`, `register_guard_hook`, `refuse_commits`.
- The 3.5k lines of `store/` are compiled once. Only the forwarders of `Store<S>` and the `StoreGraph` impl
  are monomorphised.
- The session gets a `Box<dyn SessionStore>`, a session-crate trait blanket-implemented for `Store<S>`, so the
  Store menu needs no generics.

## 6. What does not change

- The engine, the optimizer, `GraphTraversalSource<'g, S>`, and the hot paths. Rules 4 and 21 are untouched.
- The Rhai surface. The surface snapshot stays byte-identical (checked when E04b landed).
- The file formats. The generic snapshot writer must produce byte-identical files for `TraversalGraph`; the
  golden fingerprints in `encoder.rs` pin this.
- `graphersal-cli` (REPL, dev server, MCP), `graphersal-wasm`, and the Python binding keep serving
  `TraversalGraph`. They only follow the renamed types (decision 7: they stay `TraversalGraph`-only for now).

## 7. Risks

- **The `S: Default` bound on `AnyGraph`** (5.2). An in-memory storage can always be empty, so the bound is
  cheap. Decision 1 accepts it; no `Option<S>` placeholder.
- **`AnyGraph` growth.** Every front-end need becomes a method. Mitigation: the trait is sealed, and its
  methods are grouped and feature-gated. The session defines its own extension trait instead of adding view
  methods to the core.
- **Foreign change capture is the hardest part for a host.** The design gives a host the ordering contract
  (`CommitHooks`) and the constructors. It does not give an undo log (decision 5). A storage without
  `change_capture` can be served (DSL, session) but cannot be stored or dry-run.
## 8. Implementation steps (one layer or concern each)

The implementation was layered as follows. The step ids (E02-E09) are referenced from code comments and
tests; the "Checks" column names the test suites that guard each layer.

| # | Step | Needs | Checks |
|---|---|---|---|
| E02 | **`Graph<S>` struct and read-only blanket.** `Graph<S = TraversalGraph>` + guards + constructors; remove the alias and the `RwLock` re-export; blanket `impl GraphStorage for &G` replacing the concrete one; update every call site (src, tests, CLI, session, wasm, Python, harness). No dyn yet | - | core, [features], [wasm], [python]; surface unchanged |
| E03 | **Generic units.** `Transactional` extension trait (`transaction`, `transaction_with`, `dry_run`) for every `GraphStorage`; new trait methods `set_unit_metadata`, `discard_unit`, `mark` (TraversalGraph's inherent ones removed); conformance items (4.1 contract additions); book `transactions.md`, `GraphStorage` rustdoc guide | E02 | core, storage-tests |
| E04a | **`AnyGraph`, non-script part.** The sealed trait with the storage-independent groups (identity, capabilities, statistics, memory, schema, catalog, mark, io, an interim `write_snapshot` by downcast), the blanket impl for `Graph<S>`, `IntoAnyGraph`; unit tests on `Graph<TraversalGraph>`; nothing uses it yet | E03 | core, [features] |
| E04b | **Script layer over `AnyGraph`.** `ScriptPlan`/`Terminal`/`TerminalOutput`, `run_plan`/`run_unit`; `ScriptGraphTraversalSource.g: Option<Arc<dyn AnyGraph>>`; terminals, schema, catalog, saved queries, compression, statistics, mark, io, `export_snapshot` through it; scratch graphs as `Graph<TraversalGraph>`; `withSideEffect` to a foreign graph -> `Unsupported`; all `eval_*`/`graph_scope*` signatures take `impl IntoAnyGraph` | E04a | core, [surface] must not change, [tinkerpop] |
| E05a | **Proof I: the `Forwarding` storage and the DSL differential.** `Forwarding` (own newtype ids) in `graphersal-storage-tests`, conformance on it, `foreign_storage_tests.rs` (every `register/docs.rs` example + curated mutating/atomic/dry-run/saved-query scripts on both storages) | E04b | core, storage-tests |
| E05b | **Proof I: TinkerPop on `Forwarding`, CI, measurement.** `GRAPHERSAL_TEST_STORAGE=forwarding` in the harness (same baseline, wrong behaviour 0), the CI job, the 3.4 re-measurement and `cargo build --timings` | E05a | core, [tinkerpop] twice |
| E06a | **Persist foundations I: hooks, constructors, trait.** Public `CommitHooks` helper + `JournalSlot`; public `ChangeSet`/`Mutation` constructors; `PersistentStorage` trait implemented by `TraversalGraph` (inherent persistence methods move into it) | E03 | core, persist tests |
| E06b | **Persist foundations II: generic snapshot and replay.** Snapshot writer over `&S` (byte identity pinned by the golden fingerprints; the petgraph fast path kept only if measured faster), reader into the load sink, generic verified `replay` used by `Recovery::start_graph`; `Graph::new_persistent` replacing E04a's interim `write_snapshot`; format spec untouched | E06a, E04a | core, persist tests, merge/backup fast budgets |
| E07a | **Store core over `dyn StoreGraph`.** Non-generic `StoreCore` with the crate-internal `StoreGraph`; `reload_in_place` via `clear` + load sink; `backup_from_memory`, `checkpoint_from_graph`, fork from live via the generic writer. Public API unchanged | E06b | core, persist-zip, store tests |
| E07b | **`Store<S>`.** The generic shell, `open_in`/`create_in` with the capability precondition, `PersistentStorage` for `Forwarding`, a store test module on `Forwarding` over every backend; book "Your own storage" section | E07a, E05a | core, persist-zip, store tests |
| E08a | **Session helpers generic over `S`.** views/selection/display/edit/create/queries/compression as `fn x<S: GraphStorage>(..)` on `Transactional`; behaviour unchanged, the session still holds `Graph<TraversalGraph>` | E04b | session + wasm tests, [wasm] |
| E08b | **Session over `S`.** `SessionGraph`, `GraphFactory`, `StorageKind<S>`, `SessionStore`, `Session::with_storage`, compression manager on foreign storages (decision 4); session tests on both storages; wasm keeps `TraversalGraph` | E08a, E07b | session + wasm tests, [wasm], [playground] |
| E09 | **Docs and closing proof.** Book page `for_developers/custom_storage.md`, `CLAUDE.md` architecture paragraphs, `CHANGELOG.md`, rustdoc of the traits; the CI proof job also runs the E07b store module and the E08b session tests | E08b, E05b | core, rustdoc `-D warnings` |

## 9. Design questions and their resolution


1. **`Default` bound.** Is it acceptable that a storage served by the DSL/session must be `Default`, i.e. able
   to be "an empty graph" (5.2)? The alternative is an internal `Option<S>` placeholder, which is more code in
   the lock type.
   Decision: yes, `S: Default` may be required for what the DSL and the session serve.
2. **Scratch graphs.** Should `GraphSource::empty()`, `__`, `_g`, `subgraph()` and `toGraph()` in a session on
   a foreign storage keep creating `TraversalGraph`s (proposed)? The alternative is a host-chosen scratch
   storage through the engine options.
   Decision: yes, script scratch graphs (`GraphSource::*`, `__`, `_g`, `subgraph`, `toGraph`) stay
   `TraversalGraph`.
3. **Subgraph into a foreign host graph.** Should `withSideEffect(name, g)` with a foreign `g` fail with
   `Unsupported` in 0.1.0 (proposed), or write through a dyn write API?
   Decision: yes, it fails with `GraphError::Unsupported`.
4. **Compression on foreign storages.** Should the rule definitions be kept in the catalog while
   `recompress`/stats stay unsupported (proposed), or should the catalog refuse compression definitions for
   storages that cannot apply them?
   Decision: keep the definitions; `recompress` (and the compression pass) is `Unsupported` for a
   foreign storage. Other storage types will be thought about separately later. Folded into 4.3.
5. **Change capture for foreign storages.** Public constructors only (proposed for 0.1.0), or also a reusable
   undo-log + capture component that a host storage embeds (much larger, later)?
   Decision: only public constructors of `ChangeSet`/`Mutation` for now; no reusable undo-log
   component. Folded into 4.3 and 7.
6. **Store shape.** `Store<S = TraversalGraph>` with typed `graph()` (proposed), or a non-generic `Store` whose
   `graph()` returns `Arc<dyn AnyGraph>` with a typed downcast?
   Decision: try `Store<S>` as designed (5.4, 5.5).
7. **Dev server / MCP / CLI / Python.** Should they stay `TraversalGraph`-only (proposed; the external server
   builds on `graphersal-session`), or should the dev server be generic too?
   Decision: CLI, dev server, MCP and Python stay `TraversalGraph`-only for now.
8. **Names.** `Graph<S>` as the struct (keeps `Arc<Graph>` source-compatible) or a new name (`SharedGraph<S>`)?
   `AnyGraph` for the dyn trait?
   Decision: the names are fine: `Graph<S>` and `AnyGraph`.
9. **Proof depth.** Do the TinkerPop suite, every DSL doc example, the conformance suite and the curated
   differential scripts on a forwarding storage with its own id types suffice? Or should the whole `tests/all`
   DSL suite also run on it? That would mean making graph construction in ~100 test files storage-parametric,
   one more task.
   Decision: the planned proof (E05a/E05b) is enough.
10. **A genuinely independent second storage.** Should a small storage with its own undo log and capture, not
    forwarding to `TraversalGraph`, be part of the proof? It would prove more, at the cost of about one more
    task.
    Decision: not now; the proof uses the forwarding wrapper storage.
11. **Stability label.** Should `PersistentStorage` be EXPERIMENTAL in 0.1.x, like `GraphStorage` and
    `StoreDir` (proposed)?
    Decision: yes, EXPERIMENTAL in 0.1.x.

## 10. Appendix: the measurement code (section 3.4)

This is a scratch crate, not committed. `Cargo.toml`:
`graphersal = { path = ".../crates/graphersal" }`, `compact_str = "0.10"`, edition 2024, `[workspace]`.

```rust
trait ObjGraph: Send + Sync {          // approach (b): fixed ids, boxed iterators
    fn vertices(&self) -> Box<dyn Iterator<Item = VertexHandle> + '_>;
    fn out_edges(&self, v: VertexHandle) -> Box<dyn Iterator<Item = EdgeHandle> + '_>;
    fn vertex_properties(&self, v: VertexHandle) -> Option<&PropertyMap>;
    // ... the whole read subset of GraphStorage, incl. statistics()
}
impl ObjGraph for TraversalGraph { /* Box::new(GraphStorage::x(self, ..)) */ }

struct DynRead<'a>(&'a dyn ObjGraph);
impl GraphStorage for DynRead<'_> {   // reads through the vtable, writes unreachable!()
    type VertexId = VertexHandle; type EdgeId = EdgeHandle;
    fn out_edges(&self, v: VertexHandle) -> impl Iterator<Item = EdgeHandle> + '_ { self.0.out_edges(v) }
    // ...
}

// per query: best of 15 after 2 warm-ups, static vs dyn
let g = GraphTraversalSource::new(graph);            // &TraversalGraph
let g = GraphTraversalSource::new(DynRead(facade));  // &dyn ObjGraph
g.v(None::<()>).out(None::<()>).out(None::<()>).count().next()
```
