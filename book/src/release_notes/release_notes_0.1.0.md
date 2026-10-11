# Release 0.1.0

The first public release of Graphersal: an in-memory graph traversal engine in Rust, modelled on Apache TinkerPop
Gremlin, with a durable store, a command line, a Python binding and a browser playground. This page gives the
highlights; the complete list of changes is `CHANGELOG.md` in the repository root. What 0.1.0 does not do yet is
on [Current Limitations](../users_guide/current_limitations.md).

## The engine and the query language

- **Gremlin traversals** in Rust and in a Rhai script DSL, in both spellings (`has_label` and `hasLabel`), with a
  rule-based [optimizer](../optimizer/index.md), bulk traversers, sacks, `repeat()` loops, upserts
  ([`mergeV`/`mergeE`](../users_guide/upserts.md)), [`math()`](../users_guide/math.md),
  [nested property paths](../users_guide/jpath_keys.md) (`jpath`), [UUID values](../users_guide/uuid_values.md) and
  [multi-label vertices](../users_guide/multi_label_vertices.md).
- **TinkerPop conformance**: the vendored Apache TinkerPop 3.8.2 Gherkin suite runs on every test run. More than 95 % of the
  in-scope scenarios pass and no supported step behaves differently from the specification; the numbers and every
  deliberate difference are on [TinkerPop Compliance](../for_developers/tinkerpop_compliance.md) and
  [TinkerPop Deviations](../for_developers/tinkerpop_deviations.md).
- **Every traversal is atomic**: a failing traversal leaves nothing behind; explicit transactions, dry runs, change
  capture and commit hooks are in the Rust API ([Transactions](../users_guide/transactions.md)).
- **[Profiling](../users_guide/profiling_and_execute.md)** of the optimized plan with timings, traverser counts and
  optional exact memory figures; `execute()` returns results, error and profile of one run without throwing.
- **[Schemas](../users_guide/schema_enforcement.md)**: one format everywhere (a strict subset of JSON Schema 2020-12
  per label), enforced in `open` or `closed` mode, inferred from data, diffed and patched; a schema change is a
  database change.
- **[Saved queries](../users_guide/saved_queries.md)** and **[compressed properties](../users_guide/compressed_properties.md)**
  live in a catalog stored with the graph.
- **Safe for untrusted queries**: [resource limits](../users_guide/resource_limits.md), an optional memory budget,
  cancellation, and a pluggable [authorizer](../users_guide/permissions.md); no query may crash the engine (a fuzzer
  checks it).

## Data in and out

- **[GraphSON 3.0](../users_guide/graphson.md)** import and export (TinkerGraph reads the export), and GraphML with
  logical types restored from a stored schema.
- **[Persistence](../persistence/overview.md)**: lossless packed snapshots, a write-ahead journal and **the Store**:
  one directory (or [one file](../persistence/single_file_store.md)) per graph with recovery on open, checkpoints,
  marks, point-in-time reads, fork, rollback with an attic, verified full and incremental backups, verify, damage
  detection with a read-only maintenance mode and repair. The format is a public
  [specification](../persistence/format.md).

## Front ends

- **Command line** `graphersal`: a REPL with completion and help, a scriptable one-shot runner with bounded display
  and exit codes, and [`graphersal store`](../persistence/store_cli.md) to operate stores
  ([Command Cheat Sheet](../users_guide/command_cheat_sheet.md)).
- **[Web playground](../users_guide/web_playground.md)**: the real engine compiled to WebAssembly, with a graph
  drawing, a schema editor, saved queries and a store in memory; the same UI runs over the
  [dev server](../persistence/dev_server.md) (`graphersal --server`), which can also serve MCP to AI clients and an
  experimental AI Chat.
- **Python**: the `graphersal` package (abi3 wheels for Python 3.10+) with graphs, schemas and the Store
  ([Python](../persistence/python.md)).

## For integrators

- The `GraphStorage` trait lets a host run the engine, the DSL, saved queries and the Store over its own storage
  ([Custom Storages](../for_developers/custom_storage.md)); `graphersal-storage-tests` checks an implementation
  ([Storage Conformance](../for_developers/storage_conformance.md)). Both are **experimental** in 0.1.x.

## Compatibility

- Minimum supported Rust version: 1.88.
- Versioning and what is stable in 0.1.x: [Releases and Versioning](../for_developers/release_plan.md).
- Date and time values and the TinkerPop date steps are planned for 0.2.0.
