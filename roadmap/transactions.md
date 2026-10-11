# Transactions, change capture, resource limits and access control: design

Status: the 0.1.0 part is IMPLEMENTED: sections 1, 1a, 2, 3, 4 and 5. Sections 6 and 7 (`Layered`,
0.2) remain design; section 12 is the backlog beyond 0.1.0. Items marked DECIDED are settled design
choices; section 10 summarises how each open question was resolved. User documentation:
`book/src/users_guide/transactions.md`, `permissions.md`, `resource_limits.md`, and the rustdoc of
`GraphStorage`. When this design and the code disagree, the code wins.

## 0. Summary

An earlier draft used ONE mechanism (a `Layered<B, O>` overlay) for everything: rollback of a failing
query, explicit transactions, isolation of concurrent readers and long-lived project overlays. The design
splits this in two:

| Concern | Mechanism | Release |
|---|---|---|
| Atomicity of one traversal / one explicit transaction / dry run, single writer | **undo log inside `TraversalGraph`** | 0.1.0 |
| Change capture and commit events for a custom server (persistence, CDC, replication) | **`ChangeSet` + commit hooks** on `GraphStorage` | 0.1.0 (stable contract) |
| Isolation of concurrent readers during a write, long-lived project overlays over a frozen graph | **`Layered<B, O>`**, a SECOND `GraphStorage` implementation | 0.2 |
| Untrusted scripts must not hang or exhaust the process (also needed for wasm) | **resource limits** for Rhai and traversals | 0.1.0 |
| Untrusted scripts must not do what the host does not allow | **`Authorizer`** (action x resource), allow-all by default, host-configured | 0.1.0 |

Result for library users: a choice between the fast, simple reference storage (`TraversalGraph`: atomic,
single writer, readers wait for the writer) and a more powerful, slightly slower layered storage (0.2:
readers run in parallel with writers, overlays).

## 1. Undo log in `TraversalGraph` (0.1.0, DECIDED, IMPLEMENTED)

An implementation detail of `TraversalGraph`: the log itself is private and not part of the trait;
`capabilities()` reports `atomic`. The trait grows by exactly the three unit-boundary methods the generic
executor needs (DECIDED; an earlier draft kept the trait unchanged): `begin_unit() -> UnitMark`,
`commit_unit(UnitMark)`, `rollback_unit(UnitMark)`, `UnitMark` opaque, no-op defaults (a non-atomic
storage), forwarded by the `&mut`/`Box`/`Arc` wrappers. They are also the surface the hooks of section 2
attach to.

- While a mutating unit runs, every mutating `GraphStorage` method of `TraversalGraph` pushes the inverse
  operation into an internal log: `add_vertex` -> remove, `set_*_property` -> restore old value or "absent",
  `drop_vertex` -> full image of the vertex AND its incident edges (ids, labels, properties), label changes,
  `set_schema` (old schema; schema changes are transactional and roll back like data, DECIDED),
  jpath writes (old value of the top-level key).
- Every undo entry restores the INDEXES too (id index, label index, counts), not only the petgraph
  structure. The storage conformance suite found that `drop_vertex` already
  leaves stale edge-id index entries today; the undo log must not repeat that class of bug.
- Success clears the log; failure replays it in reverse order. Cost on success: one `Vec` push per mutation;
  reads cost nothing.
- Savepoints come for free: a savepoint is a position in the log; rolling back to it replays the log down to
  that position. Nested units use this.
- Units of atomicity:
  - every mutating traversal (implicit auto-commit; a failing one leaves nothing behind). Closes the
    partial-application gaps from schema-audit (`property(map)`, `property_json`, `add_e` over several
    endpoints, characterised in `tests/all/schema_gap_tests.rs`) and makes aborts by resource limits
    (section 4) and `evaluationTimeout` safe;
  - an explicit transaction in the Rust API (DECIDED): a closure, not a guard object (a closure
    cannot forget to finish; no implicit rollback in `Drop`):
    ```rust
    graph.transaction(|g| {
        g.traversal_mut().add_v("person")./*...*/next()?;  // nested unit = savepoint
        g.traversal_mut().v("1").property(/*...*/)./*...*/; // if it fails, ONLY it is rolled back
        Ok(())
    })?; // Err from the closure, or a before_commit veto, rolls back EVERYTHING
    ```
    A traversal inside the transaction is a nested unit: its failure rolls back to its savepoint and the
    closure receives the error and decides (TinkerPop / SQL behaviour). No isolation needed: `&mut` already
    excludes everybody else;
  - a Rhai script: DEFAULT unit = one traversal (predictable, TinkerPop-like). Atomicity of the WHOLE script
    is opted into by the HOST (an option of the `eval` call), not by the script; the playground and dry run
    use it. No `g.tx(|| ..)` inside Rhai in 0.1.0 (additive later). DECIDED;
  - dry run: the same transaction, always rolled back at the end: `graph.dry_run(|g| ..)` returns the result
    and the `ChangeSet` of what WOULD have changed (preview of migrations, playground).
- Restoring a dropped element: the undo log stores string ids and full element images, never element
  handles. An undone `drop_vertex` may come back in a different slot or with a new generation (section 1a);
  every handle taken before the drop stays stale, which is correct. The id and label indexes
  (`graph/index.rs`) are restored together with the element.
- DEVIATION from TinkerPop (non-transactional TinkerGraph keeps partial changes after an error), documented
  in the `///` of `to_list`, `book/src/users_guide/transactions.md` and `tinkerpop_deviations.md`.
- Implementation: `graph/undo.rs`; every terminal and `run()` open a unit (also for read-only plans: an
  empty unit costs a counter); the auto-id sequences are NOT restored (DECIDED: an id once handed out is
  never reused, as with database sequences; gaps are fine); iteration order of restored elements is not
  (unspecified, as in TinkerGraph). Loading an unpublished graph stays outside units (2.4.1); importing
  into a live graph is one unit (2.4.2).
- Implementation of the explicit API: `TraversalGraph::transaction` / `transaction_with`
  (`CommitMetadata` for the commit; a nested one merges its metadata into the enclosing unit's) and
  `dry_run` in `graph/transaction.rs` (for every atomic storage through the `Transactional` extension
  trait, `roadmap/generic-front-ends.md` 2.4). A veto reaches the closure's error type through
  `E: From<GraphError>` (`GraphError::CommitRejected`, the hook's error as `source`). A dry-run
  `ChangeSet` has `commit_seq` 0 and no hooks are called. `GraphStorage::unit_changes(mark)`
  (default: nothing) makes `Execution::mutated` mean "changed something kept in the enclosing unit"
  for a nested run. Rhai host option: `script::eval_value_atomic` (holds the shared write lock for
  the whole script); the playground runs every script as one unit (commits iff the run is `ok`),
  the CLI and the Python binding keep the per-traversal default.

## 1a. Generational element handles (0.1.0, DECIDED, IMPLEMENTED before the undo log)

Problem: `VertexId`/`EdgeId` of `TraversalGraph` were plain petgraph indices, and `StableGraph` reuses freed
slots (LIFO). A handle held across a drop whose slot is reused by a new element pointed to that new element.
This is NOT limited to handles leaving an execution (they never do: terminals materialize, Rhai/Python/wasm
see no handles); it happens INSIDE one query, through step labels, the path arena, side effects and sacks:
```
g.v("1").as_("a").side_effect(__.drop()).add_v("ghost").property("name","GHOST")
  .select("a").values("name")                       // returned "GHOST"
g.v("1").aggregate("x").side_effect(__.drop()).add_v("ghost")./*..*/.cap("x").unfold().values("name")
g.v("1").out_e().as_("e").side_effect(__.drop()).v("2").add_e("x").to(__.v("3")).select("e").label()
                                                    // returned "x", "x", "x"
```
Decision (options compared: generation counter / documenting a lifetime rule / pinning freed
slots until the end of the unit; the lifetime rule cannot fix the in-query case, slot pinning relies on
petgraph internals and fails for the concurrent readers of `Layered`):
- Handles become opaque newtypes `VertexHandle` / `EdgeHandle` (slot + `NonZeroU32` generation, no public
  constructor or fields): petgraph leaves the public API, and the representation can change without a
  breaking change.
- `TraversalGraph` keeps one generation per slot (nodes and edges), bumped when the slot is freed. A handle
  is valid iff its slot is occupied and the generations match. An edge records the generations of its
  endpoints at creation (constant for its whole life, because an endpoint cannot be dropped without the
  edge), so adjacency builds neighbour handles without an extra random memory load.
- Storage contract (`GraphStorage` docs + `graphersal-storage-tests`): a handle to a removed element never
  resolves to another element; reads through it behave as "removed".
- Semantics of a stale handle in a traversal (follows TinkerGraph, where a removed element yields no
  properties and no edges): property reads and adjacency are empty; `id()`, `label()` and materialization in
  a terminal fail with `GraphError::ElementRemoved` (TinkerGraph still knows id and label of a removed
  element; Graphersal no longer has the data: DEVIATION, rule 18).
- Budget: `size_of::<TraverserValue>()` stays 64 (an `Option<(u32, NonZeroU32)>` is 8 bytes) and
  `size_of::<Traverser>()` stays 80; the cost of the extra generation compare per element access is checked
  in the release benchmark run.
- Implementation: the generation lives in the vertex/edge weight (plus one generation array per kind
  for freed slots), so validation compares against the weight that is loaded anyway. A lazy value handle
  (`values(k)`, `id()`, `label()`) taken before a drop and returned after it fails with `ElementRemoved`
  too: the value was never read. DECIDED: an explicit error, no materialization at mutating-step
  boundaries (no performance cost, no ambiguous TinkerPop-style stale value); see
  `book/src/users_guide/dropping_elements.md`.

## 2. `ChangeSet` and commit hooks (0.1.0, DECIDED that it is part of the stable contract, IMPLEMENTED)

Why in 0.1.0: there is no Graphersal server yet, but a library user must be able to write
their OWN server on top of the library: persistence into their own store, CDC, replication, audit. So the
change capture contract belongs to the stabilised API, not to 0.2.

### 2.1 Persistence ordering problem and the answer

If an observer is only called AFTER the in-memory commit, and writing to the server's disk then fails, memory
and journal diverge and a client got "ok" for data that is lost on restart. The undo log solves this for
free: the commit hook is FALLIBLE and runs after validation but BEFORE the unit is finalised; an `Err` from
the hook rolls the unit back through the undo log and the caller gets the error. This is a classic
write-ahead log without the library knowing anything about disks.

### 2.2 Events (DECIDED)

| Event | When | Can veto? | Server use |
|---|---|---|---|
| `before_commit(&ChangeSet) -> Result<(), E>` | validated, before finalising, under the write lock | yes: `Err` -> rollback | WAL / persistence, synchronous replication, own business rules |
| `after_commit(&ChangeSet)` | after finalising | no | CDC fan-out, cache invalidation, subscriptions, metrics |
| `after_rollback(reason)` | after a rollback | no | metrics, audit of failed attempts |

Naming: `before_<operation>` / `after_<operation>`. The pattern leaves room for more
operations later.
- Several hooks may be registered; they run in registration order. The first `Err` of a `before_commit`
  stops the commit: the remaining `before_commit` hooks are not called, the unit is rolled back, and ALL
  hooks receive `after_rollback`.
- A hook gets `&ChangeSet` only, NO access to the graph: re-entering the graph from under the write lock is
  impossible by signature, and the before/after values in the `ChangeSet` are enough.
- Units that change nothing (a read-only traversal, `drop()` over an empty set) emit NO events and do not
  advance `commit_seq`.
- Synchronous only (an async trait is a separate future design).
- Hooks run strictly in `commit_seq` order under the write lock; a server may rely on that order.
- Deliberately absent in 0.1.0: per-mutation hooks and read events. Database-style TRIGGERS (a hook per
  mutating operation, e.g. `before_add_vertex`, possibly allowed to modify the change) can be added later
  ADDITIVELY under the same naming (new trait methods with defaults); a trigger that mutates the graph needs
  its own re-entrancy design and is out of scope now.

### 2.3 Content of a `ChangeSet` (DECIDED; replaces an earlier "net after-image" draft)

- `commit_seq: u64`, monotonic.
- Commit metadata: optional principal + free attributes, stored and passed through, never interpreted.
- The mutations IN EXECUTION ORDER, each with before AND after values:
  `AddVertex { id, labels, properties }`, `DropVertex { id, before }`,
  `AddEdge { id, label, out_id, in_id, properties }`, `DropEdge { id, before }`,
  `SetProperty { element, key, before, after }`, `RemoveProperty { element, key, before }`,
  `AddLabel` / `RemoveLabel`, `SetSchema { before, after }` (schema changes MUST be in the same stream,
  otherwise a WAL replay applies schema and data in the wrong order).
- Why ordered operations instead of a net after-image: deterministic replay (drop then
  re-add of the same id), CDC consumers get old and new value (Debezium style), and the undo log already holds
  exactly this data, so it costs nothing extra. The net image is derivable (`ChangeSet::compact()` helper);
  the reverse is not. Element images stay an internal detail of the future `Layered`.
- `DropVertex` lists the cascaded edge removals explicitly as `DropEdge`s, so a `ChangeSet` is self-contained
  and a consumer needs no knowledge of cascade rules.
- A jpath write (`a.b[1]`) appears as `SetProperty` of the top-level key with the whole new value: simpler,
  and the format does not depend on the path syntax.
- `#[non_exhaustive]` everywhere, serde-serializable behind the `serde` feature, our own value types only
  (rule 15).
- Implementation: `graph/change.rs` (public types, `compact()`, serde),
  `graph/capture.rs` (derivation). The cascaded `DropEdge`s come BEFORE their `DropVertex` (removal order).
  The after-values are not recorded during the mutations: at the outermost commit, while capture is on, the
  log is walked backwards over a shadow of the touched elements (starting from the graph's end state), which
  yields every entry's after-value; so capture costs nothing during the unit and nothing at all while off.
  `last_commit_seq` advances on every outermost commit of a non-empty log, with or without capture. Ids,
  labels and keys are `String`, property maps `PropertyMap`. Capture is on while a commit hook is registered;
  `CommitMetadata` is set by `transaction_with`, empty otherwise. `ChangeSet` and `Mutation` have public
  constructors, so a third-party atomic storage can build change sets and support hooks.
- Serialization (DECIDED): a serialized `ChangeSet` replays into an IDENTICAL
  graph without a schema. Values of a logical type plain JSON cannot hold carry a per-value type tag at any
  depth (rule 7 allows explicit tags): today only `uuid`, as `{"@type": "g:UUID", "@value": "<canonical>"}`
  (GraphSON style). A user object with its own `"@type"` key is escaped as
  `{"@type": "graphersal:Object", "@value": {..}}`; any other object with `"@type"` is rejected on reading.
  Scope: `ChangeSet` serde only; GraphML and the physical codec are unchanged.

### 2.4 The reverse direction: recovery and replicas

- `apply(ChangeSet)` (implemented as `GraphStorage::apply_change_set`): atomically apply a journal with preserved
  ids; explicit ids bump the auto-id sequence (existing rule).
- Snapshot with position: a consistent full dump plus `last_commit_seq`, so the server can checkpoint and
  truncate its WAL. With a single writer this is trivial under the read lock.
- Loading vs importing (DECIDED; an earlier note captured every import):
  1. **Loading into a graph that is not published yet** (start-up from disk, GraphML into a new graph,
     replaying the server's own WAL through `apply` at recovery) runs OUTSIDE transactions: no undo log, no
     `ChangeSet`, no hooks. Natural Rust shape: constructors / builders such as
     `TraversalGraph::from_graphml(reader) -> Result<TraversalGraph, _>` and an `apply` on a not-yet-shared
     graph. Nobody else can see the graph yet, so isolation comes from "build, then publish"; on error the
     broken graph is simply dropped (cheaper than undoing a journal). This is also REQUIRED for correctness:
     replaying a WAL must not fire hooks, or the server would re-write its WAL while reading it.
  2. **Importing into a live graph** (data, hooks, concurrent callers) is an ordinary mutating unit:
     transactional and captured, otherwise the server's WAL silently misses it. Only this case pays the
     memory cost of the `ChangeSet` (roughly the size of the imported data); chunked hook delivery can be
     added later if it becomes a problem.
  3. **Lazy capture:** a `ChangeSet` (the after-values) is only built while at least one hook is registered.
     The undo log always keeps the before-values for rollback. A library user without hooks pays nothing for
     change capture; a fresh graph captures nothing until the server registers its hooks.

### 2.5 Where hooks are registered

On the `GraphStorage` trait (a server is generic over the storage), with a default implementation that
reports "not supported" (rule 15), plus `capabilities().change_capture`. DECIDED:
`change_capture` REQUIRES `atomic`. A storage that cannot roll back reports `change_capture: false` and
hook registration returns "not supported"; there is no half-atomic notification semantics to specify. The
docs must explain this clearly (trait docs + book page). `TraversalGraph` supports it; the
future `Layered` emits the same `ChangeSet` from its commit, so the contract survives 0.2 unchanged. That
is the reason to freeze it now.

- Implementation: `graph/hooks.rs` (`CommitHook` with `name()`,
  `before_commit(&ChangeSet) -> Result<(), HookError>`, `after_commit`, `after_rollback(&RollbackReason)`,
  `HookError = Box<dyn Error + Send + Sync>`), `GraphStorage::register_commit_hook` (default
  `GraphError::Unsupported`; hooks cannot be removed in 0.1.0, conservative), `commit_unit` returns
  `Result<(), GraphError>` (a veto is `GraphError::CommitRejected { hook, source }`), `rollback_unit(mark,
  &RollbackReason)` (`Vetoed { hook, message }` / `Failed { message }`; reported for an outermost unit that
  changed something, also when a savepoint already undid it). `GraphStorage::apply_change_set` (default
  implementation over the trait methods inside one unit), `GraphStorage::last_commit_seq() -> u64` (default
  0), `TraversalGraph::replay(self, &ChangeSet)` and `with_last_commit_seq(self, n)` for unpublished graphs.
  Snapshot with position = an export plus `last_commit_seq()` under the same read lock (no separate snapshot
  type). `set_schema` outside a unit is a unit of its own (the inherent `&self` variant that bypassed the
  log is gone). GraphML and GraphSON imports into a live graph are one unit each; `GraphSource::from_graphml(_reader)`
  and `from_graphson(_reader)` load outside units. `Execution::mutated()` (Rhai `r.mutated`); the playground's flag is exact.

## 3. `capabilities()` (0.1.0, IMPLEMENTED)

`StorageCapabilities` (`#[non_exhaustive]`, default = nothing): `atomic`, `change_capture`, `isolation`
(level), `concurrent_writers`, `durability`, `writable`, `immutable` (storage FACTS, not permissions;
`Layered` will require an `immutable` base). Conformance tests are conditional on it.

Implementation: `graph/capabilities.rs`, `StorageCapabilities` (`#[non_exhaustive]`, pub fields,
`none()` + `with_*` const builders, `is_consistent()`), `IsolationLevel { None, Snapshot }`.
`TraversalGraph` reports `atomic + change_capture + writable` (isolation `None`: readers wait for the
writer); `&mut`/`Box` forward, a solely owned `Arc` forwards, a shared `Arc` and `&TraversalGraph` claim
nothing. A debug assertion in `GraphTraversalSource::new` and the storage-tests item
`capabilities_are_consistent` check `change_capture => atomic`.

## 4. Resource limits (0.1.0, DECIDED, IMPLEMENTED)

Untrusted scripts must not hang or exhaust the process; also needed for the wasm playground. Without
limits, `loop {}` or `s += s` in a script runs forever / exhausts memory, and `evaluationTimeout` covers
traversals only.
- Rhai: max operations / progress callback (wall-clock deadline where a clock exists; wasm32 has no
  `std::time::Instant`), max string / array / map size, max call levels, max modules.
- Traversals: audit the existing bounds (`evaluationTimeout`, `repeat.max_loops`, `bulk.max_expansion`) and
  add what is missing (e.g. a cap on materialised results / traversers), configurable through
  `ExecutionPolicy` so a host can set and lock them.
- A limit abort is an error; with the undo log (section 1) it leaves no partial changes.
- Implementation: `ScriptLimits`, `traversal.max_traversers` and the other `traversal.*` limits, the
  optional memory budget (`memory.limit`), cancellation (`CancelToken`), `ResourceLimitExceeded`; user page
  `book/src/users_guide/resource_limits.md`.

## 5. Access control (0.1.0; IMPLEMENTED, then REPLACED)

REPLACED: the operation classes and the default-deny script entry below gave way to a pluggable `graphersal::graph::Authorizer`
(an `AccessRequest` = `Action` x `Resource`, `Resource::Custom` for servers), allow-all by default in the
Rust API AND the script entry, with the ready-made `AccessPolicy` (allow/deny by action and resource kind,
file root) for the front ends. The plan-time check, the private-graph rule and the file root are kept. The
text below is the history of the first design.

The first implementation (the user page `book/src/users_guide/permissions.md` describes the current one):
`ExecutionPermissions` / `OperationClass`
(`Read`, `Write`, `Schema`, `FileIo`, `Admin`), the check in `GraphTraversalSource::run` before the
optimizer, default deny on the Rhai entry (every `eval_*` and `engine_with_limits` /
`graph_scope_with_options` take the permissions), the file root (also for Rhai `import`), private
script-built graphs. The review points below are settled as written there.

- DECIDED: **the Rust API is trusted by default.** Whoever holds `&mut graph` can call
  `graph.add_vertex()` directly, so a default-deny check on a Rust-built `GraphTraversalSource` protects
  nothing and only breaks tests, doctests, benches and every Rust user. The library owner is responsible
  for what their Rust code does. Rights follow the handle: `&G` reads, `&mut G` reads and writes.
- **Default deny applied to untrusted TEXT input** in the first design: the Rhai script entry (and a
  future GQL/Cypher parser). A script engine without explicitly granted `ExecutionPermissions` could do
  nothing; the CLI and the playground granted `unrestricted()` themselves. The `Authorizer` design makes the
  script entry allow-all as well; a host attaches its policy.
- Kept from the first draft: ONE check mechanism in the Rust core (not per-role Rhai registration), once
  per execution, before anything runs, O(steps), recursing into nested traversals; denial =
  `PermissionDenied` naming the step; a `#[non_exhaustive]` set of operation classes; an explicit set is an
  allow-list; no hiding of functions from `help()`; no user context threaded through `GraphStorage`; a host
  may still attach restrictions to a Rust-built source (same mechanism, different default).
- Review points, settled in the implementation:
  - check the plan AS WRITTEN (before optimisation), not the optimised plan: equally cheap, and an
    optimizer rule cannot hide a step by fusing it;
  - classify `g.with(...)` options (some are administrative; `ExecutionPolicy` locks overlap);
  - mutating a PRIVATE graph value the script built itself (a `subgraph()` result) is not a write to the
    host graph;
  - `export_graphml` / `import_graphml` take arbitrary paths (`script/source/mod.rs`): file I/O is its own class,
    probably with a host-configured root directory.
- Later, additive: element/label-level security as a `Filtered<S, Policy>` decorator; `ReadOnly<S>`.

## 6. Deferred to 0.2: `Layered<B, O>` (DESIGN)

A read-only lower storage `B` + a writable overlay `O` (a `TraversalGraph` without schema enforcement) +
a tombstone log. Reads overlay first, then base, filtered by tombstones; writes to the overlay only.
Transaction = short-lived layer; project overlay = long-lived layer over an immutable product graph; both
the same code. Static nesting, no `dyn`. `VertexId`/`EdgeId` = `Base(B::Id) | Overlay(NodeIndex)`.
- Immutable releases: a published product graph is never modified; a hotfix is a NEW graph; overlays are
  never rebased at runtime; copy-up of whole elements is sufficient (forced anyway by the trait returning
  `&PropertyMap`/`&str` borrowed from `&self`); the overlay records the identity of its base and refuses to open
  over a different one; moving to a new release = offline replay of the `ChangeSet`.
- Ids: stable string ids; `id_seq` (atomic, gaps fine) and `commit_seq`; a new auto id in a layer must be
  unique across lower layers.
- Deletion = tombstones (deleted ids, removed properties/labels); deleting a base vertex hides its base edges
  at read time.
- Schema: the layer's schema is a COMPLETE, backward-compatible extension of the base schema, flagged
  `backward_compatible_only` (inherited, cannot be cleared); assembling a `Layered` checks compatibility via
  the `SchemaDelta` classification (today `diff_schema`'s compatibility rule). Type COERCION (string -> uuid, int64 -> float64) must
  still happen on write inside the overlay, otherwise a transaction reads its own writes differently before
  and after commit (`has(id, UUID)` mismatch); only required-field / topology VALIDATION is deferred to
  commit.
- Isolation: READ COMMITTED + first committer wins at element granularity; owner type
  (`Arc<RwLock<S>>`-like) with `begin()` as INHERENT API of each storage (no `begin()` on the trait).
- Conflict rules: see section 7.
- Read path: `Layered` implements the whole read side; counts / `vertices_by_label` / `get_vertex_by_id`
  overlay-aware (counts = base - hidden + overlay; O(overlay), open question 10.7); plain execution untouched;
  layered execution gets its own bench.
- Known limit of the trait (keeps `GraphStorage` EXPERIMENTAL): read methods return borrows from `&self`
  (`&PropertyMap`, `&str`), so any storage must keep data in memory at stable addresses; a disk-backed or
  remote snapshot-isolation storage needs a cache or a trait change.

## 7. Conflict rules for `Layered` (0.2, DESIGN, RECORDED FOR LATER, DECIDED not to implement now)

Does not affect the 0.1.0 public API. The simple rule "first committer wins if the write sets intersect"
misses two cases:
1. Dangling edge: T1 adds edge `E: V->W`, T2 concurrently drops `V`. Write sets `{E}` and `{V}` do not
   intersect, both commit, an edge to a dropped vertex remains. Fix: adding or dropping an edge counts as a
   write to BOTH endpoints (`{E, V, W}` vs `{V}` intersect -> `TransactionConflict` for the second).
2. Schema violated across two transactions: in `Closed` mode `works_at` is allowed only from `person`. T1
   relabels `V` from `person` to `robot`, T2 adds `works_at` from `V`; each is valid on its own snapshot.
   Fix: commit-time validation runs under the write lock against the CURRENT committed state plus the
   transaction's changes, not against the transaction's start snapshot.
Write skew from unchecked read sets stays allowed (DECIDED); rule 1 closes its most dangerous
case. Element locks (`SELECT FOR UPDATE`-like) later.

## 8. Server context (informs the library, lives outside it)

Three tiers: versioned schema registry (draft/release/deprecated/removed, only backward-compatible changes:
add nullable field, add label/edge type, widen int64 -> float64, relax constraints; forbidden: remove/rename,
make required, narrow, tighten); cold graphs on disk (frozen ro or rw); hot graphs in memory with composites
(frozen base + rw overlay). Registry, lifecycle, TTL eviction, loading and all on-disk formats are SERVER
concerns; the library only needs the `ChangeSet` to be serializable and `capabilities()` to carry
`immutable`. A frozen graph is one-way (rw -> ro). `memory_usage()` per layer feeds eviction. Library part
(additive, not blocking 0.1.0): a compatibility classification on `SchemaDelta`.

## 9. Conformance suite

Conformance tests (`graphersal-storage-tests`) are conditional on `capabilities()`: atomicity
and change-capture tests for storages that claim them, snapshot isolation only for storages that claim it.
Migration between storages = full scan + `add_vertex(Some(id))`, or one big `ChangeSet` via `apply`.

## 10. Resolved questions

1. Hooks: `before_commit` (fallible, veto -> rollback) / `after_commit` / `after_rollback`, naming
   `before_|after_<operation>`, several hooks in registration order, `&ChangeSet` only, no events for empty
   units, sync only; per-mutation triggers later and additive. See 2.2.
2. Bulk load: loading into an unpublished graph (incl. WAL replay) bypasses transactions and hooks; import
   into a live graph is a captured transaction; `ChangeSet` built only while hooks are registered. See 2.4.
3. Rhai: default unit = one traversal; whole-script atomicity is a host option of `eval`. See section 1.
4. `ChangeSet`: ordered operations with before/after, explicit cascaded `DropEdge`s, top-level `SetProperty`
   for jpath writes. See 2.3.
5. `set_schema` is transactional and rolls back like data (`SetSchema` in the `ChangeSet`). See section 1.
6. Explicit Rust transactions: closure API with savepoints for nested units; `dry_run` = always rolled-back
   transaction returning the `ChangeSet`. See section 1.
7. Still open, 0.2 only: count/index fast paths with a non-empty overlay in `Layered` (O(overlay) expected).
8. `change_capture` requires `atomic`; no notification semantics for non-atomic storages. See 2.5.
9. Generational element handles: opaque `VertexHandle`/`EdgeHandle` with a per-slot generation;
   implemented first, before the undo log. See 1a.

## 11. Scope of 0.1.0 (DONE)

0. Generational element handles (section 1a).
1. Undo log in `TraversalGraph` + implicit auto-commit per traversal + deviation docs (rule 18) + tests
   (the schema gap tests: a failing write applies nothing).
2. `Mutation` / `ChangeSet` (+ serde with per-value type tags, `compact()`), produced from the undo log.
3. Commit hooks on `GraphStorage` (`before_commit` / `after_commit` / `after_rollback`; default:
   unsupported), `apply_change_set`, `replay`, snapshot with `last_commit_seq`.
4. `capabilities()`.
5. Explicit Rust transaction API (closure + savepoints, `transaction_with`), host option for whole-script
   Rhai atomicity (`eval_value_atomic`), dry run.
6. Resource limits.
7. Access control on the plan as written (first as operation classes, then as the `Authorizer`, section 5).
8. Docs: EXPERIMENTAL status and implementer's guide on `GraphStorage`, transactions and permissions book
   pages, deviations, `CHANGELOG.md`.
9. (0.2) `Layered`, tombstones, owner type, commit log, conflict rules of section 7, `g.tx()` DSL: see
   section 12.

## 12. Beyond 0.1.0: backlog

Everything decided or proposed for AFTER 0.1.0, in one place. Details live in the referenced sections.
All items are additive to the 0.1.0 contract unless marked otherwise; none is scheduled yet.

**0.2: concurrency and overlays (the second storage implementation)**
- `Layered<B, O>`: overlay over an immutable base, tombstones, `Base | Overlay` ids, copy-up, base identity
  check, layer schema as a backward-compatible extension, coercion on write in the overlay (section 6).
- Owner type (`Arc<RwLock<S>>`-like) with inherent `begin()`; READ COMMITTED + first committer wins; commit
  log while older transactions live (section 6).
- Conflict rules: edge add/drop writes both endpoints; commit-time validation against the CURRENT committed
  state (section 7). Element locks (`SELECT FOR UPDATE`-like) after that.
- Overlay-aware count/index fast paths, O(overlay) (section 10.7); own bench for layered execution.
- Overlay persistence and offline migration of a project overlay to a new product release (replay the
  `ChangeSet`, report conflicts) (section 6); overlay on-disk format is a server matter (section 8).
- `g.tx(|| ..)` inside Rhai (section 1).
- Composite `GraphStorage` over two graphs (union/overlay of independent graphs; id collisions, write
  target, schema of the union).

**Follow-ups of the 0.1.0 work**
- Python has `execute(.., atomic=True)`, `Graph.dry_run` (a `DryRunResult` with `.result`/`.changes`) and a
  per-call `policy=` keyword; the host side has `eval_value_dry_run` (whole script, always rolled back,
  returns the `ChangeSet`). Open: a dry-run preview in the playground; commit hooks registered from Python.
- Public API audit: structured error kinds across the Rhai boundary instead of text matching. Rhai turns
  every engine error into an `EvalAltResult` carrying the rendered diagnostic text, so a host (the Python
  binding's `PermissionDeniedError`/`CommitRejectedError`/`ReturnedClosureError`) recognizes the kind by
  matching the `Error: `/`Caused by: ` lines; carry the `TraverserError` (or a stable kind plus its
  structured fields: step, operation class, hook) through the boundary so hosts can match on data.
- `g.tx()` in the DSL (see the 0.2 list above; additive).
- Removing a commit hook (needs a registration handle; today hooks are permanent and capture stays on).
- A reusable undo log for third-party storages (today such a storage builds its `ChangeSet`s itself with
  the public constructors; see `roadmap/generic-front-ends.md`).
- Already in 0.1.0: a panic unwinding through any unit (a traversal run, a terminal, `execute()`,
  `transaction`/`dry_run` closures, the whole-script units, `apply_change_set`, an import, a schema change,
  a panicking `before_commit` hook) rolls it back first (`graph::storage::close_on_unwind`, `catch_unwind` +
  `resume_unwind`, no allocation on the success path); book page "Transactions", section "Panics".
- Capture clones each touched element once per commit; a per-key shadow if very large units hurt.
- A script terminal lowers its plan twice (once for `is_mutating()`); check in the release benchmark run.
- Public API audit candidates: the near-identical builder APIs of `GraphTraversalSource` and
  `AnonymousTraversal`, raw Rust type paths in the Rhai function metadata and the size of the surface
  snapshot.
- More public API audit candidates: a host must pass the same `Authorizer` to `engine_with_limits` AND
  `graph_scope_with_options`; `regex` vs `regex-lite` (wasm size); `project()` column order is lost when results
  become Rhai maps (sorted keys); the optimizer rule name `barrier_pushdown` is misleading (it pushes edge-label
  filters); the public helper `script::materialize_traversals` (added for the Python binding).
- Release benchmark check: the generation compare on every element access (generational handles).
- `script/register/registry.rs` `new_engine()` mirrors rhai 1.26's `Engine::new()`: re-check it on every Rhai
  upgrade (the standard-library equality test covers only the package).

**Change capture, after real usage shows the need**
- Triggers: per-mutation hooks `before_<op>` / `after_<op>` (e.g. `before_add_vertex`), possibly allowed to
  modify the change; a mutating trigger needs its own re-entrancy design (section 2.2).
- Chunked hook delivery for very large imports into a live graph; capture-off with a `requires_snapshot`
  notice if memory becomes a problem (section 2.4).
- Operation/audit history beyond the per-unit `ChangeSet`.
- An async storage / hook trait as a separate trait, not a change of `GraphStorage` (section 2.2).

**Distribution**
- Publish the wasm build on npm as a JS/TypeScript library (public JS API + `.d.ts` under semver, CDN via
  jsDelivr/unpkg, trusted publishing) (0.2.0; in 0.1.0 the wasm crate only powers the playground).

**Security**
- Element/label-level security as a `Filtered<S, Policy>` decorator, filter-aware counts and `*_by_label`;
  `ReadOnly<S>` decorator (section 5).
- Permission check for a future GQL/Cypher text front end through the same mechanism (section 5).

**Storage trait evolution (keeps `GraphStorage` EXPERIMENTAL until settled)**
- The script layer is generic over `GraphStorage` and the TinkerPop suite runs on a forwarding storage in CI
  (`roadmap/generic-front-ends.md`); open: move the harness into its own crate so another storage crate can
  run it.
- Read methods return borrows from `&self` (`&PropertyMap`, `&str`); a disk-backed or remote snapshot-isolation
  storage needs a cache or a trait change (section 6). Decide before 1.0.
- `graphersal-storage-tests` is published as EXPERIMENTAL with 0.1.0 (a server in another repository runs it
  from crates.io);
 drop the EXPERIMENTAL notice together with the trait's. Capability-conditional
  tests (section 9).
- `SchemaDelta` compatibility classification for the server's `backward_compatible_only` schemas (section 8).
