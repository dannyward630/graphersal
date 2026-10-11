# Graphersal: the first public release (v0.1.0) and what follows

**Positioning:** a fast, in-memory graph traversal engine and graph query library in Rust with a
Gremlin-compatible DSL, transactional storage with change capture, an optional durable Store, an
interactive browser playground, Python bindings and tracked Apache TinkerPop compliance.

This page is the release roadmap: what 0.1.0 contains (section 1), how a release is cut (section 2),
the benchmark rule (section 3), the TinkerPop backlog (section 4), the plan for 0.2.0 and later
(section 5) and the unscheduled backlog (section 6). Feature designs live in the other files of
`roadmap/`; user documentation lives in the book (`book/`). When this page and the code disagree,
the code wins.

---

## 1. What 0.1.0 contains

**Engine** (`crates/graphersal`)
- `TraversalGraph`, the reference in-memory storage: generational element handles, multi-label
  vertices, nested property values with `jpath(..)` keys, `uuid` values, statistics, exact memory
  diagnostics (`memory_usage()`).
- The `GraphStorage` trait (EXPERIMENTAL in 0.1.x) with the conformance suite
  `graphersal-storage-tests`; the DSL, saved queries, the session and the Store also run over a
  host's own storage (`roadmap/generic-front-ends.md`, book `for_developers/custom_storage.md`).
- The rule-based optimizer (rules listed in `book/src/optimizer/index.md`), path requirement
  analysis, traverser bulk with merge points, sack and `Operator`, `select(Pop, ..)`, `repeat()` in
  BFS and DFS order.
- Structured `profile()` and the `execute()` terminal (`roadmap/profile-execute.md`).
- Transactions: every traversal is one unit, explicit transactions with savepoints, dry runs,
  `ChangeSet` change capture and commit hooks (`roadmap/transactions.md`).
- Resource limits, a cancellation token, an optional memory budget, and a pluggable `Authorizer`
  checked on the plan as written.
- The schema: a strict subset of JSON Schema 2020-12 in our own envelope, modes `none`/`open`/
  `closed`, inference, diff, patch and validation.
- The catalog of definitions: saved queries (`roadmap/saved-queries.md`), compressed properties
  (`roadmap/compressed-properties.md`), host kinds.
- I/O: GraphML and GraphSON 3.0 import and export (feature `io`); the result visualizers (table,
  Markdown, JSON and more; feature `display`).

**DSL** (feature `script`): the Rhai DSL in both spellings (snake_case and Gremlin camelCase), a
`help()` text for every error, a doc entry with a checked example for every function.

**Persistence** (feature `persist`, book part "Persistence"): the native snapshot format, the
write-ahead journal with marks and point-in-time recovery, and the Store: directory, single-file and
in-memory backends, merge checkpoints (`roadmap/streaming-checkpoint.md`), verified full,
incremental and ZIP backups, rollback with an attic, forks, damage detection, salvage and repair,
compaction advice, and the `graphersal store` commands.

**Front ends**
- `graphersal-cli`: the REPL with completion, one-shot and scripted runs, the dev server with the
  playground UI, an MCP endpoint and an EXPERIMENTAL AI chat.
- The browser playground: the engine as WebAssembly in a Web Worker, query tabs, Detailed and Fast
  graph drawing (`roadmap/graph-rendering.md`), the Catalog and Store menus.
- The Python binding `graphersal` (PyO3, abi3 from Python 3.10), with a GraphRAG example
  (`bindings/py-graphersal/examples/graphrag.py`).

**Quality gates**
- The vendored TinkerPop 3.8.2 Gherkin suite with a ratchet baseline: "wrong behaviour of supported
  steps" stays 0. The numbers are generated into `book/src/for_developers/tinkerpop_compliance.md`.
- Randomized robustness, merge-checkpoint and backup tests (a fast budget in every `cargo test`, a
  long budget as a release gate), the storage conformance suite, rustfmt and clippy clean, CI
  workflows for the workspace, the Python binding and DCO sign-offs.

## 2. Release engineering and API stability

- **Semver check.** `cargo semver-checks` compares `graphersal`'s public API with a baseline. The CI
  workflow `semver.yml` is manual until the first version is on crates.io; from then on it runs on
  every pull request and is required. Locally:
  ```bash
  cargo install --locked cargo-semver-checks          # one-time
  cargo semver-checks --package graphersal --baseline-rev main
  ```
  Before v0.1.0 a reported break is allowed (pre-release, no backward compatibility); from v0.1.0
  on it is a release gate. It backs the additive-change rule: public enums, named-field error
  variants and result/config structs are `#[non_exhaustive]`, and new `GraphStorage` methods have
  defaults, so an additive change passes the check.
- **Dependency audit.** `cargo deny check` against `deny.toml` (licences, RustSec advisories,
  duplicate crates, sources) becomes a CI gate.
- **Release gates before a tag:** a benchmark baseline on a quiet machine (section 3), the long
  fuzz budgets (robustness, merge checkpoint, backups; the commands are in `CLAUDE.md`), a secrets
  scan, the checks CI leaves out (`RUSTDOCFLAGS="-D warnings" cargo doc` with the feature sets of
  `CLAUDE.md`, `mdbook build book`, the TinkerPop suite with `GRAPHERSAL_TEST_STORAGE=forwarding`,
  CI with `full` for Windows) and `cargo publish --dry-run` of every published crate (`graphersal`
  first, then the crates that depend on it).
- **Stability labels.** `GraphStorage`, `PersistentStorage`, `StoreDir` and
  `graphersal-storage-tests` are EXPERIMENTAL in 0.1.x; the JS API of the wasm crate is internal to
  the playground in 0.1.0. Versioning conventions: `book/src/for_developers/release_plan.md`.

## 3. Benchmarks

`cargo bench -p graphersal --bench graph --features io,script,display,serde` is the regression
benchmark. Rule: every supported step or feature has at least one representative shape in
`crates/graphersal/benches/graph.rs`, run on a graph large enough to surface an O(n²)-class
regression (the 110k-element `large` graph), so a change that reintroduces an allocation- or
complexity-class regression is caught by `cargo bench` rather than by a user profiling a real
dataset. A new step gets its bench shape with the step.

Benchmarks are run once per release on a quiet machine, compared with the previous baseline, not
per change.

## 4. TinkerPop backlog

The live state is generated, not written here: pass/fail numbers in
`book/src/for_developers/tinkerpop_compliance.md`, every deliberate deviation and known gap in
`book/src/for_developers/tinkerpop_deviations.md`, and the ranked list of missing steps, tokens and
overloads in `target/tinkerpop-report.md` (written by every run of the suite). Principle: an
unimplemented step is fine; an implemented step follows the TinkerPop specification unless a
deviation is documented.

Open design questions behind the remaining gaps:

1. **Null property values.** A write of `null` stores the `null` (deliberate; `remove_property()`
   removes explicitly). TinkerPop's `@DisallowNullPropertyValues` scenarios expect removal, its
   `@AllowNullPropertyValues` ones expect storage; both can only hold with a graph-level null mode.
2. **Multi-properties and meta-properties** (`Cardinality.list`/`set`, `property(k, v, metaKey,
   metaValue)`): one value per key by design; the GraphSON import folds several values into an array
   and meta-properties into `_meta`. A property value that carries its own properties would need a
   new property model.
3. **Value types** the engine does not have: set, tree and vertex-property values (`GType.SET`,
   `GType.TREE`, `GType.VPROPERTY`); dates and times come in 0.2.0 (section 5).
4. **`call()`**: `call` is a Rhai built-in function, so the step needs a DSL name of its own, and
   there is no service model (registry, `list`, `search`) yet; it may come with a server.
5. **Strategies** (`withStrategies(..)`/`withoutStrategies(..)`) are outside the compatibility
   scope: Graphersal gets its own execution-metadata mechanism instead.
6. **Smaller semantic questions**, each documented on the deviations page: `negate()`/`P.not` as a
   plain complement (differs only for incomparable operands), `Pick.any` when the selector yields
   nothing, `local(tree("a"))` without a streaming executor, a `limit()` inside a `repeat.order =
   dfs` body counting per traverser, `trim()` and `length()` on Unicode text.

## 5. After 0.1.0

The disk-backed persistence (WAL, checkpoints) and the memory limit, planned for a later milestone in
an earlier version of this page, are part of 0.1.0 (the Store and `memory.limit`).

**0.2.0**
- **Dates and times:** a logical date/time value type (schema type, codec, GraphML/GraphSON,
  `ChangeSet` tags) and the TinkerPop date steps `asDate`, `dateAdd`, `dateDiff`, `DT`,
  `GType.DATETIME`/`DURATION`, with datetime literals in the conformance harness (about 30
  scenarios). Until then the GraphSON import skips date values and reports them.
- **`Layered<B, O>`:** the second storage implementation, an overlay over an immutable base: readers
  in parallel with a writer, long-lived project overlays, conflict rules (`roadmap/transactions.md`
  sections 6, 7 and 12).
- **The wasm build on npm** as a JS/TypeScript library (public JS API with `.d.ts` under semver).
- **Vector search:** a built-in HNSW index for hybrid semantic-graph queries (`P.similar()`).
- **Gremlin Server protocol:** the WebSocket transport, so standard Gremlin clients can connect.

**Later (0.3.0 and beyond)**
- A server built on the library, in its own repository: multi-tenancy, authentication and
  authorization over the `Authorizer`, commit hooks for replication and CDC.
- A Cypher / GQL front end on the same engine (also the natural home of TinkerPop 4's string-based
  `match()`, `roadmap/steps.md`).

## 6. Backlog (unscheduled)

Nothing here is scheduled; each item is additive to the 0.1.0 contract unless it says otherwise.

**Store and persistence**
- Kept marks: a prune that preserves named points (`roadmap/mark-retention.md`).
- Commits whose WAL body exceeds 1 GiB are refused (`CommitTooLarge`, the help says to split the
  work); splitting one commit over several WAL records would be a format change.

**Engine and storage API**
- Property index: declared indexes behind the folded `has()` conditions of `v()`/`e()`
  (`roadmap/property-index.md`).
- Public schema validation helpers, so a foreign storage can enforce a schema instead of reporting
  `set_schema` as unsupported.
- The remaining items of `roadmap/transactions.md` section 12 (commit hook removal, triggers,
  structured error kinds across the Rhai boundary, a reusable undo log for other storages).

**Bulk and sack follow-ups** (revisit only when tests or benchmarks show a problem)
- Compaction of the sack arena at barriers (a query with very many distinct sack values grows the
  table), a user limit `sack.max_values` if needed, a Rust closure API for split/merge operators
  (the DSL stays lambda-free).
- Content-based path equality for `Labels` mode (merging traversers whose paths are equal but
  separately recorded); it also limits merge operators: `withSack(init, Operator)` cannot combine
  sacks of traversers with separately recorded paths.
- `barrier(n)` chunking (today `n` is validated and the whole stream is merged; only the unspecified
  order of duplicates differs from TinkerPop).
- `coin()` with a very large bulk (today `round(p * bulk)` above 100, per-unit draws below).
- The adaptive merge probe judges only the first 4096 traversers of a stream, so a stream that starts
  all distinct but becomes dense is not merged; re-probing or sampling later positions is open.

**I/O and exchange**
- A GraphSON export option for numeric ids (`#{ids: "int64"}`, typed `g:Int64`), so a graph with
  numeric ids loads into a TinkerGraph configured with `LONG` id managers without a recipe (it cannot
  help graphs whose ids are not numbers).
- More export targets and formats beside GraphML and GraphSON: JSON Lines and CSV/TSV for query
  results, Cytoscape JSON for visualization tools, and a uniform export API to a file, a string, bytes
  or any `std::io::Write`.

**Front ends**
- MCP: the stateless MCP revision (2026-07-28), once clients require it; the tool catalog stays in
  the CLI until then.
- Rich diagnostics with source spans (`roadmap/improvements.md`).
- Playground: a dry-run preview of a mutating query, shareable query links, one token list shared
  by the playground's and the CLI's completion.
- Python: commit hooks registered from Python.
