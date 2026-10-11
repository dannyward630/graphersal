# Changelog

All notable changes to this project are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor version may break the API.

## [Unreleased]

## [0.1.0] - unreleased

First public release.

### Added

- Dev server start output: after the graph is in memory, `graphersal --server` prints on stderr the load time (Store:
  with the WAL commits replayed), the vertex/edge totals, the ten largest labels, the graph's memory and the process
  heap (the data of the playground's Statistics tab); stdout still carries only the address line.
- Playground: **Run** (and `Ctrl`/`⌘`+`Enter`) runs the editor's selection when there is one, else the whole tab; the
  button reads "Run selection", the status bar and Results header say "selection (N lines)", and error positions point
  into the full text.
- Book: a **Current Limitations** page (User's Guide) listing what 0.1.0 does not do, with links to the details.
- Book: the 0.1.0 **release notes**, the developer pages **Releases and Versioning** (published artifacts, stability
  in 0.1.x, release gates) and **Version Control and Contributions** (pull requests, Conventional Commits, DCO
  sign-off), and a **Step Reference** entry page; the table of contents no longer lists pages that do not exist.
- **Custom storages for the DSL, saved queries, the session and the Store** (EXPERIMENTAL; book "Custom
  Storages"): a host's own `GraphStorage` (+ `Default`) runs under every generic front end, statically dispatched in
  the engine and with one virtual call per operation above it.
  - Rhai DSL and saved queries: the script layer holds its graph as `Arc<dyn AnyGraph>` (`graphersal::storage`,
    sealed, blanket for every `Graph<S>`: capabilities, statistics, memory, the seven schema methods, the catalog,
    `mark`, GraphML/GraphSON import and export, `write_snapshot`); `graph_scope`, `graph_scope_with_options`, every
    `eval_*` entry and `ScriptGraphSource::graph` take `impl IntoAnyGraph` (`Arc<Graph>` callers are unchanged).
    Graphs a script builds itself stay `TraversalGraph`s; `withSideEffect(name, g)` with a `g` of another storage
    fails with `GraphError::Unsupported`. The DSL surface is unchanged.
  - Change capture: `ChangeSet::builder` (`ChangeSetBuilder`, checked by `build()`, `GraphError::InvalidChangeSet`),
    one public constructor per `Mutation` kind (`Mutation::add_vertex`, ..., `VertexState::new`, `EdgeState::new`),
    the public `CommitHooks` helper a storage embeds (user hooks in registration order, then the journal slot
    `JournalSlot`; `RollbackReason::from_error`).
  - Persistence: `PersistentStorage` (feature `persist`): lineage id, commit position, load sink, `clear`,
    `attach_journal`, `replay(&changes, verify)`. `Store<S = TraversalGraph>` keeps any `PersistentStorage`:
    `Store::open_in(dir, options, empty)`, `create_in(dir, graph, options)`, `create_from_packed_in`,
    `open_backup_in`, `open_read_only_in` (the storage must claim `atomic` and `change_capture`, else
    `StoreFailure::MissingCapability` before any file is touched); every other method works for every `S`,
    `store.graph()` is an `Arc<Graph<S>>`, and the files do not depend on the storage. `Store::open`/`create`/...
    keep their signatures.
  - Session (`graphersal-session`, internal): `Session::with_storage(factory)` serves any `GraphStorage` through a
    `GraphFactory` (`StorageKind::<S>::persistent()`: graphs, `.gsnap` loads and stores; `StorageKind::<S>::new()`:
    graphs only); it holds `Arc<dyn SessionGraph>` and `Arc<dyn SessionStore>` (blanket for `Graph<S>` and
    `Store<S>`), `load_shared` takes any `IntoSessionGraph`, `attach_store` any `Store<S>`. The JSON is the same for
    every storage; `Session::new()` still serves `TraversalGraph` (playground, dev server).
  - The proof: `graphersal-storage-tests`' `Forwarding` storage (EXPERIMENTAL test helper: every method forwarded to
    a `TraversalGraph`, own handle types `ForwardVertexId`/`ForwardEdgeId`) passes the conformance suite, the
    TinkerPop suite with the same baseline, every documented DSL example and curated scripts, the snapshot and Store
    tests on every backend and the session's tests with the same results as `TraversalGraph`; a complete host example (`crates/graphersal-session/tests/custom_storage_example.rs`)
    is compiled, run and shown in the book.
- **Store warnings on stderr**: `graphersal --server` and `graphersal store` print the library's `tracing` events
  (damage found, a read-only open, a failed automatic checkpoint, now logged as a warning) on stderr as
  `WARN <target>: ...` lines, never on stdout; `GRAPHERSAL_LOG=off|error|warn|info|debug|trace` sets the level
  (default `warn`).
- **`PersistError::CommitTooLarge`**: a commit whose journal record body exceeds 1 GiB (uncompressed) is refused with
  its own error and a help that says how to split the work (drop in its own run in the playground; in the CLI, Python
  and Rust every traversal is already a unit); the limit is documented in Resource Limits and the Store pages.
- **`graphersal-storage-tests` on crates.io (EXPERIMENTAL)**: the `GraphStorage` conformance suite
  (`conformance!`/`conformance_core!`) for storages in other crates; it uses only the public API, and its schema items
  pass without checking anything when `set_schema` is `Unsupported` (the trait default). May break in minor versions
  before 1.0, like the trait.
- **REPL completion and help from the engine's docs**: Tab completes steps (after `g.`, `__.`, `.`), token classes and
  their values (`P.`, `Order.`, `T.`, `GType.`, ...), free functions and the `/` commands with their arguments, in the
  session spelling; `/help <name>` prints a step's, function's, token's or command's doc in either spelling with its
  overloads and example (`/help steps`, `/help tokens` list them). One source with the playground's completion
  (`graphersal_session::views::completion_entries`, `token_values`). `graphersal --repl` runs the REPL on a piped stdin.
- **Topic facade modules** of the public API: `graphersal::storage` (the `GraphStorage` trait and every type in its
  signatures), `changes`, `auth`, `catalog`, `schema`, `profile` and `exec`, beside `prelude` (now also `Order`) and
  `error`. Public constructors for the errors a storage implementation returns (`GraphError::unsupported`,
  `element_removed`, `duplicate_vertex_id`, `duplicate_edge_id`, `vertex_not_found`, `edge_not_found`,
  `invalid_label`, `invalid_element_state`), since the variants are `#[non_exhaustive]`.
- Storage contract helpers in `graphersal::storage`: the error constructors `GraphError::invalid_property_value` and
  `invalid_property_path`, the write checks `check_property_value`, `check_properties`, `check_label`,
  `normalize_vertex_labels` and the JSONPath write helpers `path_property_name`, `write_path`, `write_path_in`,
  `remove_path`, `remove_path_in` (the same functions `TraversalGraph` runs), and the public `JsonPathSegment` with
  `JsonPath::segments()`.
- The topic modules also hold the items downstream code reached through the old paths: `storage::{Graph,
  HookError}`, `schema::{UNLABELED, MAX_GROUPS, PRINTED_GROUPS, SAMPLE_IDS, SAMPLE_ID_CHARS}`, the catalog's size
  bounds (`catalog::MAX_*`, `DEFAULT_MIN_BYTES`, `MAX_MIN_BYTES`) and `exec::optimizer_rule_names`; the workspace
  crates, tests, examples and the book now use the prelude and these modules instead of `graphersal::graph::..`,
  `graphersal::traversal::..`, `graphersal::value::..` and the crate-root globs.
- **`iterate()` terminal** (TinkerPop): runs a traversal for its side effects and discards its results without
  materializing them, as one unit like every terminal. Rust `GraphTraversalSource::iterate`, Rhai `iterate()` (one word,
  both spellings), and so the CLI, Python `execute`, the playground and MCP.
- **Host-defined catalog kinds**: definition kinds 128-255 are reserved for hosts (`DefinitionKind::host(n)`,
  `HOST_FIRST`/`HOST_LAST`, `is_host()`); the library stores them opaque, byte for byte, and the payload codec of its
  own kinds is public (`catalog::encode_payload`/`decode_payload`, feature `persist`).
- **Dev server `--ai FILE`** (EXPERIMENTAL, CLI feature `ai`, on by default): the configuration of the AI Chat's
  LLM provider (`openai` Chat Completions incl. compatible servers via `base_url`, or `anthropic` Messages), read
  strictly and checked at server start; the API key comes from the environment variable `api_key_env` names.
- **Dev server AI Chat agent** (EXPERIMENTAL, `--ai FILE`): the agent loop runs in the server and calls the MCP
  tools in process (change feed client `ai` with the conversation id as `detail`); endpoints
  `/api/ai/conversations[/<id>[/messages|/events|/stop]]`, conversations isolated per page (`403` for another
  client, random ids), one turn at a time per conversation, Stop cancels only that conversation's call and query;
  `GET /api/info` reports `ai`. Session: `Session::execute_with_cancel` runs a script under the caller's own token.
- **Playground AI Chat tab** (EXPERIMENTAL, dev server with `--ai`): "+" in the Query panel opens a Query or an AI
  Chat tab with its own conversation, transcript (persisted with the tab), collapsible tool calls ("Open in a query
  tab", Run), Stop and the `/clear` and `/help` commands; answers render as safe Markdown (GFM tables that copy as
  Markdown, query blocks with Copy and Run); read-only in the WebAssembly playground.
- **AI Chat docs and example configs**: the book section "AI Chat" of the dev server page (every `--ai` key,
  Anthropic/OpenAI/Ollama examples, what data leaves the machine, read-only, limits), a cheat sheet entry, and
  `crates/graphersal-cli/examples/ai-{anthropic,openai,ollama}.json` (no keys inside).
- **`has_label()` without arguments** (`hasLabel()`, Rust `has_label(None)`) keeps the elements that have a label;
  `not(__.has_label())` selects unlabeled vertices and edges (a Graphersal extension; TinkerPop has no zero-argument
  form).
- **Dev server on the IPv6 loopback**: `--server` also listens on `[::1]:<port>` when the system has IPv6 (silently
  IPv4 only otherwise); `[::1]` is accepted by the Host and Origin checks.
- **Dev server: Save to file writes the schema back**: with `--schema FILE`, `POST /api/save` writes the stored
  schema to that file too (atomically, like the graph); without it, saving a GraphML/GraphSON graph that has a schema
  answers a `warning` that the playground shows as a toast.
- **Python: load a graph with its schema; import methods**: `Graph.from_graphml(path, schema=.., mode=..)` and
  `Graph.from_graphson(path, schema=.., mode=..)` set the schema (a mapping or JSON text) on the new empty graph
  first, then import the file as one unit (declared `uuid`/`array`/`object` come back typed; a violation in
  `open`/`closed` fails the load); `graph.import_graphml(path)` / `graph.import_graphson(path)` import into a live
  graph as one unit (all or nothing; GraphSON skips are one `UserWarning`).
- **CLI `--schema FILE [--schema-mode MODE]`**: the schema file is set on the new empty graph first, then the
  `--graph` GraphSON/GraphML file is imported against it as one unit (declared logical types come back, the mode is
  enforced); with a sample graph it is applied to it; a violation stops the start with exit code 1; refused with a
  Store or a `.gsnap`. Works with `--server` too (`GET /api/info` reports `schemaSource`).
- **Label steps** `add_label("L")`/`addLabel`, `drop_label("L")`/`dropLabel` (vertices) and `set_label("L")`/`setLabel`
  (edges): Graphersal extensions that change the labels of existing elements and pass them on, checked by the schema,
  rolled back with a failing traversal; the help of a `closed` freeze blocked by unlabeled elements suggests them.
- **Edge label change** (`GraphStorage::set_edge_label`): an edge's single label can be changed (e.g. to label
  unlabeled edges before freezing a schema as `closed`), checked under the new label, rolled back, captured as
  `Mutation::SetEdgeLabel` and journaled as WAL op 11 (format spec 9.3).
- **Persistence documentation**: the book's "Persistence" part (`book/src/persistence/`: overview, quick
  start, the Store page by page, the `graphersal store` reference with every command, options and exit codes, the
  dev server and MCP, the playground's store, Python, an FAQ), the storage format as a public normative
  specification (`book/src/persistence/format.md`), and a Command Cheat Sheet with every command to build, run and
  operate Graphersal (`book/src/users_guide/command_cheat_sheet.md`).

- **Rollback, attic, maintenance mode, repair** (`persist`, native; format spec 11, 12.5, 13): `Store::rollback_to(target,
  Move | Delete)` rolls a store back in place (the later history moves to `attic/`, a new lineage continues);
  `attic_list` / `attic_changes` / `attic_restore` / `attic_fork` / `attic_remove`; prune keeps attic bases. A damaged
  store opens READ-ONLY in maintenance mode with a `DamageReport` (damaged files, chunks, records, id ranges, commits,
  donors); `Store::repair_to(new_dir)` builds a repaired, verified copy (donor chunks, skipped covered records, lost
  and DIVERGED elements reported). A record whose fsync failed is cut off again before the journal poisons itself
  (or its position recorded in `GRAPH`), so a refused commit is never replayed. CLI `graphersal store rollback |
  attic | repair`, clean close of an open store on Ctrl+C / SIGTERM (`ctrlc`); the dev server shows maintenance mode
  (banner, damage report) and the attic; Python `Store.rollback/attic/attic_restore/repair_to/maintenance`.
- **The Store** (`persist`, native): one directory = one graph (`persist::Store`): WAL segments with rotation and
  fsync per commit, recovery on open (interrupted operations completed, temporary files removed, torn tail cut,
  clean-close marker, `OpenReport`), an OS lock (one writer), manual and automatic checkpoints (verified before they
  count), marks across the whole history, point-in-time read-only loads, `fork`, `prune` with donor retention,
  `verify` (scrub), file-level backups without a graph lock (and ZIP behind `persist-zip`), the format-version
  guard, `changes_since`, packed import/export. CLI `graphersal store ...` and `--graph <dir>`; the dev server on a
  Store directory (the playground's Store menu: position, snapshots with `.gsnap` downloads, marks, Checkpoint,
  Mark); Python `graphersal.Store`. WAL record frames gained a header CRC (format v1, before any release).

- **Traversal engine** (`graphersal`): an in-memory property graph with labels, indexes and an optional schema,
  traversed through a fluent Rust API modelled on Apache TinkerPop Gremlin. Vertices carry a label set; edges carry one
  label. A rule-based optimizer, path-requirement analysis, bulk (multiplicity) traversers, sacks, `repeat()` with
  `emit`/`until`/`times`, `.profile()` returning structured `TraversalMetrics` (plan step ids, optional exact memory
  figures), the `execute()` terminal (results, error and profile of one run, never throws), `glob_path()`, upserts (`mergeV`/`mergeE`),
  `jpath` property keys for nested values, `math()` expressions, UUID values and type-conversion steps.
- **Gremlin DSL**: a Rhai script layer (`script` feature) in Gremlin spelling (snake_case and camelCase), with data
  terminals returning native values and a session visualizer for unterminated traversals (table, markdown, JSON,
  JSON Schema, Mermaid, PlantUML, tree).
- **Schema**: one format everywhere, an envelope (`$schema`, `meta`, `mode`, `vertices`, `edges` with `connections`)
  around a strict subset of JSON Schema 2020-12 per label (a Pydantic `model_json_schema()` pastes in); unknown keywords
  are located errors. Seven methods in Rust, the DSL, Python and the playground: `get_schema`, `set_schema`,
  `patch_schema` (JSON Merge Patch), `validate_schema` / `validate_schema_patch` (dry run with violation report and
  diff), `infer_schema`, `diff_schema` (changes with backward compatibility). Enforcement in `none`/`open`/`closed`
  modes (`required` and nullability independent, `additionalProperties` defaulted by the mode), a schema change is a
  database change (change capture, rollback, `Execution::changes()`), `default` is an annotation only (never applied),
  a meta-graph and renderers (table, markdown, tree, Mermaid, PlantUML, JSON Schema, JSON). "Freeze the structure you
  have": `set_schema(schema, mode)` (DSL token `SchemaMode`, Python `mode=`, Rust `GraphSchema::with_mode`), so
  `g.set_schema(g.infer_schema(), SchemaMode.closed)` stores the inferred schema enforced; it always applies to the data
  it came from (unlabeled elements in `closed` mode are reported by name). The playground's schema editor has
  "Freeze as open/closed".
- **I/O** (`io` feature): GraphSON 3.0 import and export, the primary exchange format for Gremlin users (TinkerGraph
  3.8.2 reads the export, verified; ids stay strings, the book gives the `LONG` id-manager recipe for `g.V(1)`;
  import reads the line and wrapped forms, 3.0/2.0 typed and untyped 1.0, leniently: dates and types Graphersal cannot hold are skipped and listed in an `ImportReport`, never converted; multi-valued properties
  become arrays, meta-properties the `_meta` property), and GraphML import and export. CLI `--graph x.json`, Rhai
  `import_graphson`/`export_graphson`, Python `Graph.from_graphson`/`export_graphson`, playground Load/Save.
- **Resource limits**: `ScriptLimits` for the script engine, `traversal.max_traversers`, `evaluationTimeout`,
  `repeat.max_loops` and `bulk.max_expansion`, lockable by a host-set `ExecutionPolicy`. A value-depth limit
  (`ScriptLimits::max_value_depth`, `traversal.max_value_depth`, default 128) bounds how deeply arrays and maps nest
  wherever a value enters the engine, so no recursive reader can overflow the native stack.
- **Memory budget** (off by default): `memory.limit` (bytes, or `"auto"` = the cgroup memory limit minus
  `memory.headroom`) fails a traversal that would take the memory in use over it with `ResourceLimitExceeded` and rolls
  it back, instead of the process being OOM-killed; exact with the tracking allocator, estimated without. CLI
  `--memory-limit <bytes|auto>`, Python `Graph(memory_limit=...)`.
- **Command line** (`graphersal-cli`, binary `graphersal`): an interactive REPL and a one-shot query runner with bounded
  display and scriptable exit codes.
- **Python binding** (`bindings/py-graphersal`): abi3 wheels for Python 3.10+.
- **Browser support**: the library builds for `wasm32-unknown-unknown`; the web playground (`playground/`) runs the real
  engine compiled to WebAssembly (`graphersal-wasm`, internal, not published) in a Web Worker, with Stop, Save graph and
  exact (tracked) memory profiles, and a Statistics tab in the Profile panel (`g.statistics()`, the memory footprint,
  and the limits a query runs under: host presets beside the engine defaults).
- **GraphML with typed values**: GraphML carries no schema (it travels as its own JSON); an import restores logical types
  from the target graph's stored schema (set it first): a declared `uuid` from its canonical text, a declared
  `array`/`object` from its JSON text. The playground saves the graph and its schema as two files and has "Load schema".
- **Conformance**: runs the vendored Apache TinkerPop 3.8.2 Gherkin suite (see the compliance page of the book for the
  current numbers and the documented deviations) and ships `graphersal-storage-tests` for `GraphStorage` implementations.
- **Atomic traversals**: every traversal is one unit of work (implicit auto-commit). A traversal that fails (a step
  error, a schema violation, a resource limit, `evaluationTimeout`, an error while materializing its results) is
  rolled back as a whole: data, labels, properties, indexes, statistics and the stored schema. A GraphML import into a
  live graph and `set_schema()` are one unit each. Automatic ids are never handed out again, so a rollback can leave
  gaps. A panic unwinding through any unit (an engine bug, or a panic in a transaction closure) rolls it back first.
- **Explicit transactions** (Rust API): `TraversalGraph::transaction(|g| ..)` (`Ok` commits once, `Err` rolls back,
  every traversal inside is a savepoint, nesting allowed), `transaction_with(CommitMetadata, ..)` and
  `dry_run(|g| ..)`, which always rolls back and returns the `ChangeSet` of what would have changed.
- **Whole-script units** (Rhai host option): `script::eval_value_atomic` runs a whole script as one unit; the web
  playground runs every script this way.
- **Change capture and commit hooks**: `ChangeSet` / `Mutation` describe a committed unit in execution order with
  before and after values (`compact()` gives the net effect; with `serde` it serializes as JSON with per-value type
  tags such as `{"@type": "g:UUID", ...}`, so a journal replays into an identical graph without a schema).
  `CommitHook` (`before_commit` may veto and roll back, `after_commit`, `after_rollback`), registered with
  `GraphStorage::register_commit_hook`, makes a write-ahead log, replication or audit possible on top of the library.
  `apply_change_set()` applies a change set to a live graph as one unit; `TraversalGraph::replay()` rebuilds an
  unpublished graph without firing hooks; `last_commit_seq()` plus an export under one read lock is a snapshot with
  position (`with_last_commit_seq()` continues the numbering); `GraphSource::from_graphml_reader()` loads from any
  reader.
- **Storage capabilities**: `GraphStorage::capabilities()` returns `StorageCapabilities` (`atomic`, `change_capture`,
  `isolation`, `concurrent_writers`, `durability`, `writable`, `immutable`); change capture requires atomicity.
  `graphersal-storage-tests` runs its rollback and hook items only for a storage that claims them.
- **Authorization**: a pluggable `Authorizer` decides every `AccessRequest` (an `Action` — `Read`, `Create`,
  `Update`, `Delete`, `Execute`, `Custom` — on a `Resource` — `Data { element, label }`, `Schema`, `File`, `Option`,
  `Custom`), asked once per execution on the plan as written, before anything runs
  (`TraverserError::PermissionDenied` names the step and the request). Everything is allowed by default (`AllowAll`),
  in the Rust API and the Rhai script entry; `AccessPolicy` is a ready-made policy (allow/deny by action and resource
  kind) with a file root that confines file access and Rhai's `import`. A Rust-built source opts in with
  `GraphTraversalSource::with_authorizer`; an embedding server checks its own resources (`Resource::Custom`) with the
  same authorizer.
- **Persistence over streams** (`persist` feature, format v1 of the storage format specification): the packed single-file snapshot
  (`graph.write_snapshot` / `TraversalGraph::read_snapshot`, lossless, schema, lineage id, commit position and
  auto-id sequences, CRC-32C everywhere, LZ4 chunks, zstd behind `persist-zstd`), the `Journal` commit hook
  (write-ahead log over any `Write`, runs after every other hook, vetoes the commit on a failed write or sync,
  lock-free `DurableEnd`), marks (`graph.mark(name)`, Rhai `g.mark`, Python `graph.mark`) and recovery to the end, a
  commit, a time or a mark (`persist::recover`). Rhai `g.export_snapshot(path)` and `.gsnap` in
  `GraphSource::file` / `--graph`; Python `Graph.save` / `Graph.load` / `start_journal` / `Graph.recover`; the
  playground saves and loads `.gsnap`. `TraversalGraph::graph_id`, `with_graph_id`, `with_last_committed_at`,
  `with_sequences`.
- `Execution::mutated()` (Rhai `r.mutated`): whether a run changed the graph and kept the change.
- Generational element handles `VertexHandle` / `EdgeHandle` and `GraphStorage::contains_vertex` /
  `contains_edge`; a new `GraphError::ElementRemoved` for the id, label or value of an element removed earlier in
  the same query.
- Rhai: `Uuid.asString()` and `GraphSchema.toGraph()` (the missing second spellings).
- Rhai: `g.exportGraphml(path)` / `g.importGraphml(path)`, the camelCase twins of `export_graphml` / `import_graphml`.
- `PersistError::Store` carries a `cause: StoreFailure` (name taken, backup running, nothing after the target,
  unknown attic entry, store closed, ...) with its own `help()` per cause; Python store errors get a `cause`
  attribute (`"name_taken"`, ...; `None` for other errors).
- Store identity `store_id` (a creation parameter, `GRAPH` offset 100): `StoreInfo::store_id`, `Store::store_id`,
  `graphersal store info` ("fixed at creation"), the dev server's `storeId`, Python `store.store_id`. A fork,
  attic fork, repair or `store convert` gets a new one; rollback, attic restore and a restored backup keep it.

### Changed

- Playground **Store menu: a running store action is clearly visible**: its button reads "Verifying the store… 0:42"
  (bold, accent colour, spinner, ticking time), the top-bar Store button reads "Store · verifying… 0:42" while the
  menu is closed, every other store action is disabled with "Wait for … to finish", a toast gives the result (with
  **Details** when problems were found) and a polite live region announces start and end; the same for Checkpoint,
  Mark, Compact, Back up, Fork, Roll back, Restore, Remove, Switch to, Close and the read-only view.
- Playground highlighting: the token classes come from the engine (`tokenValues()`) once loaded, with a fallback list
  that now includes `SchemaMode` and `WithOptions`.
- **Playground completion of token values from the engine**: after `T.`, `Order.`, `GType.`, ... the playground
  offers the members the engine registers (one source with the CLI REPL, `graphersal_session::views::token_values`)
  instead of a hard-coded list, so every token class is completed now (`GType`, `Operator`, `Merge`, `Pick`,
  `Barrier`, `SchemaMode`, `WithOptions` were missing) and `By.` also lists `By.Property(..)`. New engine call
  `tokenValues()` (wasm `tokenValues`, dev server `GET /api/token-values`, `Session::token_values`).
- **A mark is never lost without a question**: the playground's Compact always asks when marks would become
  unreachable (recommended or not), naming them; `POST /api/store/compact` answers 409 with `marksUnreachable`
  unless the body has `{"dropMarks": true}`; `graphersal store prune`/`store compact` name the marks, change
  nothing and exit 1 unless `--drop-marks` (`store compact-advice` lists them). New
  `Store::prune_estimate_dir(dir, up_to)` (the estimate, nothing removed); `Store::prune`/`compact` are unchanged.
- Dev server: the playground UI is embedded in the `graphersal` binary, so `--server` works from any directory;
  `--ui-dir` (default `./playground` when it exists) only overrides files of the embedded copy, the banner says
  `UI: embedded` or `UI: <dir> (embedded fallback)`, and a path leaving the UI root is a 404 (was 403).
- **A Store's rollback and attic restore reload its graph in place**: the graph of `Store::graph()` is cleared and
  the store loaded into the same instance (the commit hooks registered on it stay, as before). Attaching a Store's
  journal requires a storage that claims `atomic` and `change_capture` (new cause `StoreFailure::MissingCapability`;
  `TraversalGraph` claims both).
- **A failed in-place reload leaves an EMPTY graph and a clear error**: when loading the new state after a rollback
  or attic restore fails, the graph is emptied (never left half-loaded) and the error is the new cause
  `StoreFailure::ReloadFailed` (graph emptied, store stopped, files intact, close and reopen). A store stopped by a
  failed rollback or restore refuses every change with the new cause `StoreFailure::Stopped` (before:
  `PersistError::ReadOnly` with the help for a newer version) and says why in `Store::stopped_reason()`,
  `StoreInfo::stopped`, the session's `stopped` and the playground Store menu ("Store stopped").
- **Snapshots and recovery work for any `PersistentStorage`**: `persist::write_snapshot` takes `&S` (pass
  `&*graph.read()` for a shared graph), the new `persist::read_snapshot_into(reader, options, empty)` loads into an
  empty `S` through the load sink (`read_snapshot` stays the `TraversalGraph` form), `Recovery::new_in(empty)`
  recovers into any persistent storage (`Recovery<S = TraversalGraph>`, `Recovered<S>`; every journal commit through
  `PersistentStorage::replay`). `Graph::new_persistent(s)` makes a persistent storage writable through
  `AnyGraph::write_snapshot` and `g.export_snapshot` (a `Graph<TraversalGraph>` is written either way). The bytes
  written are unchanged.
- `GraphError::Unsupported`'s help fits every refused operation: it names the operation, says the storage does not
  implement it (or not in its current state) and points to the storage's documentation and `capabilities()`; it no
  longer talks about commit hooks only.
- Playground: vertex and edge counts in the header, the Store menu and the history calendar have thousands
  separators (`122,012 V · 1,223,362,500 E`).
- `GraphStorage::recompress` defaults to `GraphError::Unsupported` (was `Ok(false)`): a storage without compressed
  properties keeps compression rules as definitions but cannot apply them.
- **`side_effect(SideEffectFn)` (Rust) is usable from outside the crate**: `SideEffectFn` is public (prelude) and its
  callback now takes the traverser's value and bulk (`Fn(&TraverserValue<S>, u64)`) instead of the internal
  `Traverser`; it runs once per traverser (TinkerPop's `sideEffect(Consumer)`), no longer `bulk` times.
- **`Graph` is a struct**, `Graph<S: GraphStorage = TraversalGraph>`: a lock newtype instead of the alias
  `RwLock<Box<TraversalGraph>>`. `Graph::new(storage)` replaces `RwLock::new(Box::new(storage))`, `read()`/`write()`/
  `try_write()` return the guards `GraphRead`/`GraphWrite` that deref to `S` (`&mut *g.write()` instead of
  `&mut **g.write()`), plus `get_mut`/`into_inner`; `Arc<Graph>` still means a shared `TraversalGraph`.
  `storage::RwLock` is no longer re-exported (no `parking_lot` type in the public API).
- The read-only view `&G` is a blanket `GraphStorage` impl for every storage (it was `&TraversalGraph` only): reads
  forward, writes fail, no capabilities, hooks refused.
- **Explicit units are the extension trait `Transactional`** (prelude, `graphersal::storage`), implemented for every
  `GraphStorage`: `transaction`, `transaction_with` and `dry_run` replace the inherent `TraversalGraph` methods (same
  behaviour; `dry_run` now needs `E: From<GraphError>` and a storage with `change_capture`, else `Unsupported`).
  New `GraphStorage` methods with defaults: `set_unit_metadata`, `discard_unit` (roll back without hooks, return the
  `ChangeSet`) and `mark` (replaces the inherent `TraversalGraph::mark`), plus `GraphError::mark_unavailable`. The
  conformance suite gained `mutation_outside_unit_records_nothing`, `discard_unit_restores_state_keeps_sequences`,
  `transaction_with_metadata_reaches_hooks` and `transactional_panic_rolls_back`.
- REPL commands start with `/` instead of `:` (`/help`, `/set max-rows 20`, `/exit`); an input starting with `:` gets a
  pointer to its `/` form. The display-limit notices name `/set no-limit` and `/set max-items <n>`.
Relative to the pre-release development snapshots:

- **Playground: compact property rows in the element edit/create dialogs**: each property is one line
  (name | type | value | buttons, columns aligned across the rows; two lines in a narrow window), a boolean
  is a checkbox, a changed or invalid row is marked at its edge; multi-line text, raw JSON and the children
  of arrays/objects open below their line; the add row takes name, type and value on one line (Enter adds);
  id and label sit on one line under the title; **Edit as JSON** moved next to Save.
- **`PropertyMap` replaces the `Element` alias**: an element's property map (the payload of
  `ElementProperty::Object`/`Vertex`/`Edge`, what `GraphStorage` stores and returns) is now the newtype
  `graphersal::value::PropertyMap` (`#[repr(transparent)]`, no run-time cost) instead of a public alias of
  `indexmap::IndexMap`, so a major version of `indexmap`/`fnv` no longer breaks users. Its API: `new`,
  `with_capacity`, `len`, `is_empty`, `get`/`get_mut`/`contains_key`/`shift_remove` (keys as `&str`), `insert`
  (any `Into<CompactString>` key), `iter`/`iter_mut`/`keys`/`values`/`values_mut`, `clear`, `Index<&str>`,
  `FromIterator`/`Extend`/`IntoIterator`, `From<[(K, V); N]>`, serde behind `serde`.
- **`StreamMap` replaces the `ObjectType` alias**: the map a traversal produces (the payload of
  `TraverserValue::Object`: `group()`, `group_count()`, `tree()`, ...) is now the newtype
  `graphersal::value::StreamMap<S>` (`#[repr(transparent)]`, no run-time cost) with the same API shape as
  `PropertyMap` (keys are `TraverserValue`s). The crate-root re-exports `graphersal::IndexMap` and
  `graphersal::FnvBuildHasher` are gone.

- **Playground AI Chat shows the answer, the steps collapsed**: per question, the agent's interim texts, tool calls
  and results go into one collapsed group with a summary of how far the turn got ("Worked: 2 tool calls (…)",
  "Working: …" live, "Stopped after 1 tool call: …" on an error); the final answer is shown expanded below it.

- **Store format version 2**: the store format version in `GRAPH` is 2 (the store id; `persist::STORE_FORMAT_VERSION`),
  separate from the file format version 1 of snapshots, WAL and `.gsnap` (`persist::FORMAT_VERSION`, unchanged). A
  store of version 1 is refused by every operation with the new `PersistError::OlderFormat` (no longer "not a
  store"), whose help gives the migration: `g.export_snapshot("graph.gsnap")` with the old binary, then
  `graphersal store create <new_dir> --from graph.gsnap` (recipe in the Command Cheat Sheet). `UpgradeRequired` and
  the read-only opening of older stores are removed.
- **Playground: the Schema panel infers on demand**: without a stored schema it infers on its first show and on its
  refresh button only, never after each change of the graph; after a change it keeps the last inferred schema with
  the badge "may be outdated - refresh". A stored schema still reloads on every change; a replaced graph (load,
  read-only view, rollback, database switch) is inferred once when the panel is shown.
- **Python: tracking allocator**: the extension installs `TrackingAllocator` as its global allocator, so
  `profile_with(ProfileType.Memory)` reports exact (`tracked`) bytes and `Graph(memory_limit=..)` compares the
  live Rust heap of the extension (Python objects are not counted: with `"auto"`, `memory_headroom` must cover
  the Python side).
- **Memory budget, estimated mode**: the graph footprint a run starts from is cached by the graph's commit
  position (`GraphStorage::memory_footprint_bytes`), so traversals on an unchanged graph no longer walk it
  each time; the first traversal after a committed change walks again.
- **Memory budget, units that are not traversals**: GraphML/GraphSON imports into a live graph and
  `apply_change_set` check the graph's `memory.limit` (its `ExecutionPolicy` default; a script import also the
  session preset) every 4096 elements and at their end, and every outermost unit (a `transaction(|g| ..)` closure)
  is checked at its commit; a unit that grew the memory in use over the budget rolls back with the new
  `GraphError::ResourceLimitExceeded { operation, .. }`. Without a budget nothing is checked.
- **Playground Store menu, Compact**: the Disk space block's Compact button is no longer disabled when the
  compaction advice says it is not worth it; it asks first then ("Compacting frees about N; not recommended:
  <reason>. Compact anyway?"). It stays disabled while locked, in a read-only view, in maintenance mode, on a
  backup and on a store frozen by damage.

- **Playground Load from file**: a loaded GraphSON/GraphML file always starts WITHOUT the previous graph's
  schema (it was carried over before). The load dialog has an optional second field "Schema file (JSON)": the
  schema is set on the new empty graph first and the data imported against it as one unit (typed GraphML
  values come back; an `open`/`closed` violation fails the load and nothing changes). Dropping one file loads
  it without a schema; a graph file + its `.json` schema dropped together pair up when unambiguous. "Load
  schema" applies to the current graph only. Session `load_graphml`/`load_graphson` and the wasm
  `loadGraphml(Bytes)`/`loadGraphson(Bytes)` take the optional schema JSON as a third argument.

- Schema violation reports group array items under `[*]` (`$.addresses[*].zip`) and name the first
  concrete path (`ViolationGroup::example_field`, `example_field` in the data form, "(first at ...)"
  in the text), so one array rule no longer fills the group cap.
- **Store reports** (`persist`): when both `GRAPH` copies are damaged, the `DamageReport` says "lineage before commit N
  is approximate (both GRAPH copies damaged)" (`lineage_approximate_before`; `graphersal store verify` prints the
  damage report then instead of "not a store", `repair`, the dev server's damage view, Python `repair_to`).
  `Store::fork`/`fork_dir`/`attic_fork`/`attic_fork_dir` return a `ForkReport` (the snapshot plus
  `marks_not_carried`), and `RepairReport::marks_not_carried` lists the marks a repair does not copy (CLI lines
  `name @ commit N`, Python `"marks_not_carried"`, dev server `marksNotCarried`). A bare date as a time target
  (`--at-time 2026-10-08`, the dev server's `{"time": ..}`) is still refused; its error now shows the explicit forms
  (`2026-10-08T23:59:59Z` = the end of that day in UTC, `2026-10-08T00:00:00+02:00` with an offset).
- DSL tokens show as the DSL text that builds them (`Scope.local`, `Order.desc`, `T.id`, `Operator.sum`,
  `P.gt(1).and(P.lt(3))`, `TextP.containing("a")`, `Column.keys`, a class itself as `Scope`) as a final value,
  through `to_string()` and in `${..}`, instead of a Rust type path or a bare name (`sum`, `Gt(1)`, `local`); an
  anonymous traversal returned by a script shows its text (`__.out().has("name", "x")`) instead of running it.
- `diff_schema`: removing a property declaration (optional or required) from an open object (one that admits
  undeclared keys) is backward compatible; it is still listed as `property_removed`. From a closed object it stays
  incompatible.
- `has_id(P)` / `has(T.id, P)` with an order predicate on a number (`P.gt(3)`, `P.between(1, 5)`, ...) is a
  plan-time error (`ValueError::NumericIdComparison`) instead of silently matching nothing: ids are strings; the
  help shows the string form (`has_id(P.gt("3"))`) and the string order. Equality forms keep numbers.
- `compact_str` 0.8 -> 0.10 (no behaviour change for Graphersal). `CompactString` (labels, keys, string values in
  the public API) is re-exported as `graphersal::CompactString` (and in the prelude), so users need not match the
  `compact_str` version; from 0.1.0 on a `compact_str` major bump is a Graphersal minor bump.
- `TraversalGraph::add_vertex` / `add_edge` / `add_edge_by_ids` return `VertexHandle` / `EdgeHandle`; petgraph's
  `NodeIndex` / `EdgeIndex`, the `graph::index` module and `GraphType` are no longer public.
- A handle to an element removed earlier never resolves to another element any more (a dropped vertex's slot could be
  reused by a new one). Writes through such a handle fail with `ElementRemoved` (was `VertexNotFound` /
  `EdgeNotFound`); `outV()` / `inV()` of a removed edge are empty (was an `EdgeNotFound` error).
- `GraphStorage` grew the unit methods `begin_unit` / `commit_unit` / `rollback_unit` (with `UnitMark`) and
  `unit_changes` (`UnitChanges { data, schema }`), plus `capabilities`, `register_commit_hook`, `last_commit_seq`, `apply_change_set` and
  `contains_vertex` / `contains_edge`, all with defaults. `commit_unit` returns `Result<(), GraphError>` (a hook veto
  is `GraphError::CommitRejected`), and `rollback_unit` takes a `&RollbackReason`. New error variants:
  `GraphError::ElementRemoved`, `CommitRejected`, `Unsupported`; `TraverserError::PermissionDenied`,
  `SourceStepInChild`; `ScriptError::ReturnedClosure`.
- The schema format `graphersal-schema-v2` (`version`, `nullable`, `variants`, `min`/`max`, ...), `load_schema()`,
  `load_schema_json()`, `GraphSchema::from_file`, `SchemaDelta` and the `schema()` step and source method are removed:
  use the JSON Schema based format and the seven schema methods (the stream step is `infer_schema()`). A schema
  change is recorded, rolled back and captured like data.
- Script entry: `engine()`, `graph_scope()` and the `eval_*` functions without limits allow everything;
  `engine_with_limits(&limits, authorizer)`, `engine_with_options`, `graph_scope_with_options(graph, &options,
  authorizer)`, `eval_value_with_limits`, `eval_value_atomic` and `eval_value_dry_run` take an
  `Arc<dyn Authorizer>`. The CLI and the Python binding allow everything (Python: `policy=AccessPolicy...` per call),
  the playground everything except files. Rhai's `import` asks for `Execute` on the module's file and resolves it
  through the authorizer (file root). Setting an administrative `g.with()` option (resource ceilings, optimizer and
  engine switches) is an `Update` on `Resource::Option`.
- `ScriptGraphTraversalSource::statistics()` / `memory_usage()` return a `Result` (they ask for `Read` on `Data`).
- `withSack()` / `withSideEffect()` inside a child traversal (`__`) fail with `SourceStepInChild` instead of being
  ignored silently.
- `type_of(P.eq(1))` is `"Predicate"`.
- Creating a script engine is cheap (about 1.5 µs instead of about 1.2 ms): the Rhai registrations are built once per
  thread and shared.
- TinkerPop conformance: the 16 scenarios that need the `crew` dataset are deliberate incompatibilities (group
  `multi-meta-properties`: one value per key by design) instead of `skipped`; the compatibility scope is 1674.
- CI: the Python wheels (five platforms) are built only on pushes to `main` and on `v*` tags; a pull request runs
  the binding's tests only (Python 3.10 and 3.13).
- Permissions: only steps that reach graph data (`V()`, `E()`, adjacency steps, `subgraph()`) ask the authorizer
  for a read; value-only steps (`has`, `values`, `inject`, `math`, ...) ask nothing, so `g.inject(1, 2).sum()`
  runs under `AccessPolicy::deny_all()`. Each distinct request is asked once per execution.
- Store backups: an incremental backup requires the backup's store id to equal the store's, so a fork, repair or
  copy of the backed-up store (which shares its lineage history) is refused with "a different store" instead of
  continuing the original's backup; take a full backup into a new directory. The refusal after a rollback behind
  the backup now says to keep the old backup as the archive of the abandoned history.

### Removed

- **The old public paths**: `graphersal::graph`, `graphersal::traversal` and `graphersal::value` are internal, the
  crate-root globs and `graphersal::RwLock` are gone; the root keeps only the modules and `TraversalGraph`,
  `GraphTraversalSource`, `GraphSource`, `TraverserValue`, `ElementProperty`, `TraverserError` and `CompactString`.
  The prelude holds what queries need (graph, sources, `GraphStorage`, values, tokens, `GraphSchema`/`SchemaMode`,
  `VisualizeFormat`, `Execution`, `TraversalMetrics`, errors, now also `ValueMap`); host items live only in their
  topic module (`auth`, `changes`, `catalog`, `schema`, `profile`, `exec`, `storage`). Internals are no longer
  reachable (`SchemaValidator`, `Traverser`, `TraverserPath`, `ArgStrings`, `TextRegex`, `OrderedF64`,
  `TraversalStrategy`, `StepFrame::logical`, the step structs, `CompressedText`, ...).
- **MCP persistence tools**: the dev server's MCP endpoint works with the in-memory graph only; `store_info`,
  `checkpoint`, `mark`, `open_view`, `close_view`, `rollback`, `attic_list` and `attic_restore` are gone (the Store
  menu and `/api/store*` keep them). While the browser has a read-only view open, MCP's `graph_info` and every tool
  error carry a note naming the view's commit.
- The inherent `TraversalGraph::set_schema(&self)`, which bypassed rollback and change capture; use
  `GraphStorage::set_schema(&mut self)`.
- The DSL-only builders `GraphTraversalSource::merge_v_entries`, `merge_e_entries`, `option_merge_entries`,
  `option_merge_by_with` and `property_entries` (the DSL lowers through the regular builders).
- Rhai: the bare PascalCase predicate globals (`Eq`, `Neq`, `Lt`, `Lte`, `Gt`, `Gte`, `Inside`, `Outside`, `Between`,
  `WithIn`, `WithOut`, `StartingWith`, `EndingWith`, `Containing`, their `Not*` forms, `TypeOf`) and the bare `by()`
  globals `Property`, `Traversal`, `JsonPath`; write `P.gt(30)`, `TextP.containing("a")`, `By.Property("age")`,
  `jpath("a.b")`.
- The hidden test hook `GraphTraversalSource::internal_to_list` (raw traversers with their bulk) is crate-internal;
  the bulk tests that need it are unit tests now.

### Fixed

- `graphersal` no longer panics when the reader of its stdout or stderr goes away (`graphersal -e '..' | head -1`,
  `graphersal --server 2>&1 | head`, a REPL whose terminal closed): a closed stdout ends a one-shot run, a store
  command or the REPL quietly with exit code `141`, stderr output is best effort, and the dev server keeps serving.
  The CLI crate denies `clippy::print_stdout`/`print_stderr`; output goes through `src/out.rs`.
- A graph as a script's final value (`g.e(0).to_graph()`, `cap("sg").next()`, `g`) is shown as its
  `{"vertices": [..], "edges": [..]}` listing like `cap("sg")` shows it, in `graphersal` and the playground, cut to
  the display limits (only `max_items + 1` elements of each kind are read); it printed nothing before.
- Error locations (`at #N:`, `plan:`, `step_location()`, a help's rewritten query) no longer show the
  `limit(n) [display]` step a bounded visualizer appends: they show the query as written.
- CLI: `graphersal --graph <store>` on a store that cannot be opened (in use by another writer, damaged, an older
  format) exits 1 like the `store` commands, not 2 (2 stays for invalid invocations).
- CLI: `store create` (and `--create-store`) into a non-empty directory says "cannot create a store in data/: the
  directory is not empty" with a help to choose an empty or new directory or inspect it with `store info`, instead of
  "is not a Graphersal store"; into an existing store it says "it is a store already".
- CLI: a read-write open that rewrote a missing or damaged `GRAPH` copy prints one `note:` line on stderr
  (`--graph <store>` in every mode, the `store` commands that open read-write, the dev server before its banner).
- CLI: the stderr warning log (`GRAPHERSAL_LOG`) is installed for every mode on a `--graph <store>` (`-e`, `--in`, a
  script file, the REPL), so a failed automatic checkpoint is visible there too; it was only on for `store` and
  `--server`.
- A commit hook that panics in `before_commit` rolls the unit back (every hook gets `after_rollback`, the commit
  sequence number does not move, a journal writes nothing) before the panic continues; the unit used to stay
  applied in memory. A panic in `after_commit` leaves the unit committed and continues.
- A panic inside the journal while it writes a record (a bug, or a store backend that panics) is handled like a failed
  write: the partial or complete record is cut off again (its position recorded in `GRAPH` when the cut fails), the
  journal is poisoned (`a panic while writing a commit record: ...`), the unit rolls back and the panic continues; a
  reopen gives the state before the unit. Documented in the book (`Failures while committing`).
- `Store::create` (and the target of `Store::convert`) into a non-empty directory fails with its own error
  `PersistError::DirectoryNotEmpty` ("Cannot create a store in <dir>: the directory is not empty", help: choose an empty
  or new directory, or open the existing store) instead of `NotAStore`, whose help advised creating a store.
- A failed reload of a store whose reads fail names the real read error instead of a follow-on "starting a WAL
  segment: ... exists"; a failed manifest read in the open's check for critical segment kinds fails the open instead
  of being ignored (the best-effort manifest copy check reports it as an open warning).
- `g.export_snapshot(path)` that fails (a storage without snapshots, a write error) removes the file it created
  instead of leaving an empty or partial one behind.
- **The compaction advice counts the WAL** (`store compact-advice`, `Store::compaction_advice`, the Store menu's
  Disk space block, `/api/store/compaction`): the prune estimate sized every doomed path as a directory, so WAL
  segments (files) counted 0 bytes and a store of mostly old WAL was reported as "no compaction needed". It now sizes
  files by their length and leaves out the files kept snapshots still reference, so `prune_bytes` equals what the
  prune removes.
- **The automatic checkpoint no longer stops after a large commit or a failure** (a store's WAL could grow past
  1 GiB with no new snapshot): only the commit that crossed `checkpoint_after_bytes` woke the background thread, so a
  commit landing while a checkpoint ran (one commit can exceed the threshold) or a failed checkpoint left the WAL above
  it for good. Now every commit wakes the thread while the WAL since the last snapshot is at or above the threshold,
  the thread checks again right after a checkpoint it wrote, and a failed one is retried at the next commit (at most
  one attempt per commit; `last_auto_checkpoint_error` is cleared by the next success). A Checkpoint record also no
  longer moves the journal's durable end back to the snapshot's commit when commits landed during the checkpoint.
- **Playground dialogs scroll with the pointer anywhere inside them**: the Help, Settings, Load, Catalog (saved
  queries, compression), saved-query editor, element editor and Store dialogs (incl. Show changes and the history
  calendar) no longer stop a wheel or trackpad gesture over their body, a table that only scrolls sideways, or a
  textarea, editor or list that reached its end; the gesture goes on to the dialog, never to the page.
- **The AI Chat transcript scrolls with the pointer over a code block, a table or a tool result**: a block that
  only scrolls sideways no longer swallows vertical wheel and trackpad scrolling, and a tool result scrolls
  itself to its end and then hands the gesture on to the transcript.
- **AI Chat works with Gemini thinking models** through Google's OpenAI-compatible endpoint: the `openai` adapter
  sends the provider extras of a tool call (`extra_content`, Gemini's thought signature) and the message's
  `extra_content` back unchanged (never `reasoning_content`); before, the second request of a tool round failed
  with HTTP 400 "missing a thought_signature". A provider error in the middle of a turn now says how many tool
  calls the turn made and that the next message continues the chat.
- **The Store menu shows the store's size once**: "store N" is every byte the store occupies (all its files, the
  container file of a `.gstore`), the figure the compaction advice uses; the Disk space block no longer repeats it
  (it shows live and free space inside) and the advice's reason no longer names it ("the store is smaller than the
  minimum of 64.0 MiB"). It was the sum of snapshots, WAL and attic before, without the small files.
- **A write in a read-only view says so**: "the served state is a read-only view at commit N (mark ...) of the
  store ...: writes are refused", with a help that points to Back to the live graph and to a fork, instead of the
  generic commit-hook help ("for example the write-ahead log ..."). A Store's other refusals (a backup, maintenance
  mode, damage found, a store read-only for this version) now give their own help too. New
  `graph::HookRefusal::new(message, help)`: a commit hook's veto with its own help.
- **AI Chat pairs each tool call with its own result** when a provider repeats a tool call id in a later turn
  (nothing in the API forbids it): an earlier call no longer shows the later call's result.
- **Rust `has_label(ElementProperty::Null)` keeps nothing**, like the DSL's `has_label(())` and TinkerPop's
  `hasLabel(null)`: a value converted to a step argument (`ArgStrings`) is never "no argument"; null, a map or an
  object without a string `id` is an empty list that matches nothing (also `v`, `e`, `out`, `values`, ...), only
  `None::<()>` means "no argument".
- **`playground/serve.py` no longer resets connections**: the page's burst of parallel module requests overflowed
  the default listen backlog of 5 (`ERR_CONNECTION_RESET`, the page never started); the server now listens with a
  backlog of 128 (threaded, `Cache-Control: no-store` unchanged; system Python 3.9 and 3.10+).
- **Playground stays responsive on large graphs after Store actions**: a view, rollback, restore or a run that
  changed the graph no longer blocks the page for 1-3 s on the `large` sample (longest main-thread task now about
  0.1 s, WebAssembly and dev server): the Schema panel keeps the stored and inferred schema out of Vue's reactivity,
  formats the JSON text only when it is read, holds the JSON tab of a schema with more than 2000 labels until asked,
  and the worker passes schemas as JSON text.

- A map injected with `inject()` is an object like a `project()`-ed map: `values(k)`, `select(k)`, `select(a, b)`
  and `has(k, v)` read its keys (nested maps and the maps of an injected list too) instead of failing with a cast
  error or an "undeclared label" refusal; an injected list is an `array` value like a `fold()` result.
- `has_id(P)` / `has(T.id, P)` with a number in an equality predicate (`P.neq(3)`, `P.without(1, 2)`, nested in
  `and`/`or`/`not`; Rust `has_id_p(P::Eq(3))`, `P::WithIn(vec![1, 2])`) compares the number's text with the string
  ids instead of matching nothing (or, negated, everything).
- `apply_change_set` raises the target's auto-id sequences to the change set's (`max`, like `TraversalGraph::replay`;
  new `GraphStorage::raise_id_sequences`, default no-op), so an auto id the source handed out (also compacted away
  or rolled back) is never handed out again by the target.
- A unit that changed nothing but raised an auto-id sequence (applying a change set whose mutations were all
  compacted away) is now a sequence-only commit: it takes a commit sequence number, hooks get a `ChangeSet` without
  mutations carrying the raised sequences, and a journal/Store writes it, so the raise survives a reopen.
- `select("a")`, `cap("x")` or a path could return a *new* element that reused the storage slot of an element dropped
  earlier in the same query; it now reads as removed.
- A failing traversal no longer leaves partial changes behind (`property(map)`, `property_json`, `mergeV` /
  `mergeE` `onMatch` maps and `addE` over several start vertices could be half applied under a schema).
- A failed `add_vertex` / `add_edge` with a duplicate id no longer advances the automatic id sequence.
- A Store's mark whose history a prune removed is no longer listed as if it could be opened (`Store::marks`,
  `StoreInfo::marks`, `graphersal store marks|info`, the Store menu, Python): listings leave out marks before the
  oldest snapshot, opening one says that a prune removed its history, and its name stays taken with a message that
  says why. `PruneReport::marks_unreachable` (CLI `store prune`, Python, the dev server's Compact) names the marks a
  prune made unreachable, and the compaction advice (the Store menu's Disk space block) names them BEFORE the prune.

### Notes

- The `GraphStorage` trait is experimental in 0.1.x.
- Minimum supported Rust version: 1.88.
