# graphersal

An in-memory graph traversal engine in Rust, modelled on Apache TinkerPop Gremlin.

Graphersal is compatible with, and inspired by, Apache TinkerPop Gremlin. It is an independent
project: it is not part of Apache TinkerPop and is not endorsed by the Apache Software Foundation.

- A fluent Rust traversal API (`g.v().has_label("person").out("knows")...`) over an in-memory
  property graph with labels, indexes and an optional schema.
- A rule-based query optimizer, bulk (multiplicity) traversers and `.profile()`.
- Optional features, all off by default and strictly additive:
  - `script`: a Rhai Gremlin DSL, with resource limits for untrusted scripts.
  - `io`: GraphSON 3.0 and GraphML import and export.
  - `display`: table rendering.
  - `serde`: schema I/O and the JSON visualizer.
  - `alloc-tracking`: exact per-step memory figures for `.profile()`.
- Checked against the Apache TinkerPop 3.8.2 Gherkin conformance suite.

```rust
use std::sync::Arc;
use graphersal::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Needs the `script` feature.
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let names = graphersal::script::eval(
        graph,
        r#"g.v().has_label("person").values("name").to_list()"#,
    )?;
    println!("{names}");
    Ok(())
}
```

Scripts run under safe default limits (`graphersal::script::ScriptLimits`) and may do everything
by default; a host that runs scripts from untrusted sources restricts them with a
`graphersal::auth::Authorizer` (for example the ready-made `AccessPolicy::read_only()`).
The command line tool is the `graphersal-cli` crate (binary `graphersal`).

Licensed under the Apache License, Version 2.0 (see `LICENSE-APACHE` and `NOTICE`).
