# Graphersal Codebase Audit & Architectural Improvements

Graphersal is fundamentally a **Rust Library Crate** (`graphersal`) designed both to be embedded into Rust applications and to evaluate graph traversal scripts (via Rhai). Consequently, the codebase must adhere strictly to idiomatic Rust library design standards: clean public API boundaries, logical module organization, minimal dependency conflicts, standard error traits, structured logging/tracing, and strong internal encapsulation.

---

## Scripting and CLI diagnostics (`miette`, Rhai spans)

- Integrate `miette` in `graphersal` for rich diagnostic rendering, providing visual source code pointers, line/column spans, and syntax highlights for Gremlin DSL scripts.
