# graphersal-session

**Internal crate, no stability guarantees.** The session logic of the Graphersal playground: one
graph, one Rhai engine, and the operations of the playground's engine boundary (execute a script,
schema views and the schema methods, graph drawing, completions, memory usage, GraphSON and
GraphML load/save) as plain Rust that takes and returns JSON.

It is shared by two front ends, so both answer with the same structures by construction:

- `graphersal-wasm`: the engine compiled to WebAssembly for the browser playground;
- `graphersal-cli`: `graphersal --server`, the local DEV server that serves the same playground
  UI over HTTP (unstable, local only).

It is published only because `graphersal-cli` depends on it. Its API follows the playground and
changes with it in any release; use the `graphersal` crate instead.

License: Apache-2.0 (see `LICENSE-APACHE` and `NOTICE`).
