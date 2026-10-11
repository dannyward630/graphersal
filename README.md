# Graphersal

> In-Memory Graph Database & Traversal Engine in Rust, inspired by Apache TinkerPop Gremlin.

Graphersal is an open-source, high-performance graph traversal engine and interactive query tool written in Rust. It provides a fluent, type-safe Rust traversal API, a Gremlin-compatible Rhai scripting environment, GraphSON 3.0 and GraphML I/O, and an interactive command-line REPL.

---

## Workspace Architecture

Graphersal is structured as a Rust workspace with dedicated crates:

- **`graphersal`** (`crates/graphersal`): Core graph library. Implements graph data structures (`TraversalGraph`), traverser pipeline primitives, traversal steps (`V`, `E`, `has_label`, `out`, `in`, `values`, `profile`, etc.), GraphSON and GraphML import/export (`io` feature), and Rhai script engine bindings (`script` feature).
- **`graphersal-cli`** (`crates/graphersal-cli`, binary `graphersal`): Command-line front end built with `rustyline` and `rhai` — an interactive REPL (`graphersal>`) and a one-shot query runner (`graphersal -e '<query>'`, or a query piped on stdin) that prints only the result.
- **`graphersal-wasm`** (`crates/graphersal-wasm`, not published): the engine compiled to WebAssembly for the browser playground.
- **Playground** (`playground/`): a static single-page app that runs the real engine (WebAssembly, in a Web Worker) in the browser, with no backend.

---

## Key Features

- **Fluent Rust Traversal API:** Construct Gremlin-style traversals directly in Rust code (e.g., `g.v(None).has_label("person").out("knows").to_list()`).
- **Gremlin DSL Scripting:** Execute string queries using Rhai script evaluation (`g.V().hasLabel("person").out("knows")`). Data terminals (`toList()`, `next()`) return native script values, so results can be bound to variables and processed further; a traversal left without a terminal step is rendered with the session visualizer.
- **Profiling & Execution Stats:** Deep introspection into step timing and traverser counts using `.profile()`.
- **GraphSON Support:** Import and export graphs as GraphSON 3.0, TinkerPop's JSON format (`io` feature): `GraphSource::from_graphson("graph.json")`, `graph.export_graphson("out.json")`.
- **GraphML Support:** Import and export graphs in GraphML XML (`io` feature). Exports default to compact representation, with optional formatted ("beautified") output via `g.with("beautify", true)`.
- **`with()` Modulator Configuration:** Configure global traversal source options using Apache TinkerPop `with(key, value)` semantics.
- **Graph Schema & Pipeline Visualization:** Infer schemas (`g.infer_schema()`, the `infer_schema()` step), store, patch, validate and diff them (JSON Schema based format), and visualize schemas or query results directly using `.visualize(format)` or shortcuts (`.to_table()`, `.to_mermaid()`, `.to_plantuml()`, `.to_json_schema()`, `.to_json()`).
- **Persistence** (`persist` feature): lossless packed snapshots (`.gsnap`), a write-ahead journal with recovery to a commit, a time or a mark, and the Store: one directory (or one `.gstore` file) = one graph, durable commits, checkpoints, point-in-time views, fork, rollback with an attic, full/incremental/ZIP backups, verify and repair (`graphersal store ...`). See the book's "Persistence" part (`book/src/persistence/`); the storage format is a public specification (`book/src/persistence/format.md`).
- **Interactive REPL:** Full interactive shell with command history, auto-completion, and inline help.
- **Scriptable Query Runner:** The same binary evaluates a query passed on stdin or via `--eval` and prints only the result, with a dedicated exit code for a failed query — usable from shell scripts, CI checks and automated tooling.

---

## Building & Running

Every command to build, run and operate Graphersal is on one page of the book: the Command Cheat Sheet (`book/src/users_guide/command_cheat_sheet.md`).

### Prerequisites

- Rust 1.88+ / `cargo` (measured minimum: 1.87 for the library, 1.88 for the CLI)
- `mdbook` (optional, for building local documentation)

### Commands

- **Build workspace:**
  ```bash
  cargo build --workspace
  ```

- **Run CLI REPL:**
  ```bash
  cargo run -p graphersal-cli
  ```

- **Run a single query (non-interactive):**
  ```bash
  echo 'g.v().values("age").max().next();' | cargo run -q -p graphersal-cli -- --in --graph modern
  ```

- **Run the Web Playground** (needs `rustup target add wasm32-unknown-unknown` and
  `cargo install --locked wasm-bindgen-cli --version 0.2.129`; see `playground/README.md`):
  ```bash
  playground/build.sh
  python3 -m http.server -d playground 8000   # open http://localhost:8000/
  ```

- **Run Test Suite:**
  ```bash
  time cargo test --package graphersal --features script,io,display,serde --release
  ```

**Tinkerpop Test Suite:**
```bash
cargo test --package graphersal --features script,io,display,serde --test tinkerpop_features
```
Report recreating here `target/tinkerpop-report.md`

Current state (TinkerPop 3.8.2, in-scope scenarios): 1247 / 1792 pass (69.6 %), 61 skipped, 484 unsupported (not implemented), 0 wrong behaviour of supported steps; see `book/src/for_developers/tinkerpop_compliance.md` and `book/src/for_developers/tinkerpop_deviations.md`.

```bash
cargo llvm-cov test --package graphersal --features script,io,display,serde
```

- **Build Book Documentation:**
  ```bash
  mdbook build book/
  ```

**Run Benchmarks:**
```bash
cargo bench -p graphersal --features io,script,display,serde
cargo bench -p graphersal --bench graph --features io,script,display,serde
```

---

## Rust Library Usage Example

Add `graphersal` to your `Cargo.toml`:

```toml
[dependencies]
graphersal = { path = "crates/graphersal", features = ["io", "serde", "script"] }
```

In your `src/main.rs`:

```rust
use graphersal::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize an empty traversal graph
    let graph = GraphSource::empty();

    // 2. Add vertices and edges
    let v1 = GraphTraversalSource::new(&graph)
        .add_v("person")
        .property("name", "Alice")
        .next()?
        .unwrap();

    let v2 = GraphTraversalSource::new(&graph)
        .add_v("person")
        .property("name", "Bob")
        .next()?
        .unwrap();

    GraphTraversalSource::new(&graph)
        .add_e("knows")
        .from(&v1)
        .to(&v2)
        .property("weight", 0.8)
        .next()?;

    // 3. Export graph to GraphML format (compact default or beautified)
    graph.export_graphml("output.xml")?;
    GraphTraversalSource::new(&graph)
        .with("beautify", true)
        .export_graphml("output_pretty.xml")?;

    // 4. Import graph from file
    let imported = GraphSource::from_graphml("output.xml")?;
    let g = GraphTraversalSource::new(&imported);

    // 5. Execute traversals
    let results = g.v(None)
        .has_label("person")
        .out("knows")
        .values("name")
        .to_list()?;

    for val in results {
        println!("Found: {:?}", val);
    }

    Ok(())
}
```

---

## CLI Usage Guide

`graphersal` runs in two modes: the interactive `graphersal>` REPL (`cargo run -p graphersal-cli`), and a
one-shot query runner that evaluates a script and exits.

### Non-Interactive Query Runner

Pass a script on stdin or through `--eval`, and `graphersal` prints the result and nothing else. That
makes it usable straight from a shell script, a CI check, a Makefile, or any tool that needs to
verify a Gremlin-like query without embedding the library:

```bash
cargo build -p graphersal-cli

# A query on stdin, evaluated against the built-in TinkerPop Modern sample graph
echo 'g.v().values("age").max().next();' | target/debug/graphersal --in --graph modern
# 35

# A query as an argument, against a graph loaded from disk
target/debug/graphersal --graph ./mygraph.graphml -e 'g.v().has_label("person").count().next()'

# Several queries sharing one scope, each result printed on its own
target/debug/graphersal -e 'let names = g.v().values("name").to_list();' -e 'names.len()'

# A failing query: diagnostic on stderr, exit code 1
target/debug/graphersal -e 'g.v().values("name").max().next()'
```

**Options**

| Option | Description |
|---|---|
| `-g`, `--graph <SOURCE>` | The graph to run against; see *Graph sources* below. |
| `-e`, `--eval <SCRIPT>` | Evaluate `SCRIPT`. May be repeated; scripts share one scope and are evaluated in order. |
| `-i`, `--in` | Read the script from stdin. Implied when stdin is not a terminal, so `echo ... \| graphersal` works on its own. |
| `-f`, `--format <FORMAT>` | Default visualizer for results rendered without a data terminal: `auto`, `table`, `markdown`, `json`, `jsonschema`, `mermaid`, `plantuml`. |
| `-h`, `--help` / `-V`, `--version` | Print usage / version and exit. |

**Graph sources** (`--graph`, case-insensitive)

| Value | Graph |
|---|---|
| `modern` | TinkerPop Modern sample graph (6 vertices, 6 edges); the default. |
| `empty` | An empty graph, to be populated by the script itself. |
| `large` | Generated ~110k element benchmark graph. |
| `tree` | Filesystem-like sample tree (37 vertices, 38 edges) for `glob_path`; also `file_tree` / `file-tree`. |
| *path* | A GraphML file (`*.xml`, `*.graphml`); GraphML is the only graph file format. |

**Output contract**

- `stdout` carries only the rendered result; `stderr` only the diagnostic of a failing script.
- Exit codes: `0` every script evaluated successfully, `1` a script failed to evaluate, `2` invalid
  invocation (unknown option, or a graph that could not be loaded), `141` stdout was closed before
  every result was written (`graphersal -e '..' | head -1`; never a panic).
- A multi-statement script yields the value of its last statement, like any Rhai block; statements
  that evaluate to unit (`let x = ...;`) print nothing.

The same flags apply to the REPL: `graphersal --graph ./mygraph.graphml --format mermaid` starts an
interactive session against that graph with that default visualizer.

### REPL Commands

Commands start with `/` (an input starting with `:` gets a pointer to its `/` form). Tab completes
steps after `g.`, `__.` and `.`, the token classes and their values (`P.`, `Order.`, `T.`, `GType.`,
...), the commands and their arguments, in the session spelling (`/set spelling camel` switches to
Gremlin camelCase). Completion and help read the same catalogue as the playground's query editor.

- `/help` — List the commands, grouped.
- `/help <name>` — The doc of a step, function, token class or command in either spelling, with its
  overloads and an example with its result (`/help has_label`, `/help hasLabel`, `/help P.eq`,
  `/help Order`); `/help steps` and `/help tokens` list them.
- `/set format <format>` — Set the default visualization format for schema and queries (`auto`, `table`, `markdown`, `json`, `mermaid`, `jsonschema`, `plantuml`, `tree`).
- `/set max-rows <n>`, `/set max-items <n>`, `/set no-limit`, `/set spelling <snake|camel>`, the
  script limits (`/set max-operations <n>`, ..., `/set safe-limits`): `/help set` lists them.
- `/exit` or `Ctrl+D` — Exit the REPL session.

### Example Queries in REPL

```text
graphersal> g.V();
graphersal> g.V().has_label("person");
graphersal> g.V().has_label("person").out("knows").values("name");
graphersal> g.V().profile();
```

### Query Results vs. Rendering

A traversal has two kinds of terminal steps, and the distinction is what makes the DSL usable both
as a one-liner shell and as a real scripting language:

- **Data terminals** — `toList()` returns a native script array, `next()` a single value. Scalars
  come back as numbers, strings and booleans; vertices, edges and projections come back as object
  maps addressable by key. Nothing is pre-rendered, so results can be stored in a variable and
  processed further:

  ```text
  graphersal> let people = g.V().hasLabel("person").toList();
  graphersal> people.len();
  graphersal> people[0].name;
  graphersal> for person in people { print(person.label + ": " + person.name); }

  // Results feed straight back into the DSL:
  graphersal> let names = g.V().hasLabel("person").values("name").toList();
  graphersal> g.V().has("name", within(names)).count().next();
  ```

  A failing traversal fails the script with the full diagnostic attached, so a broken query is
  never mistaken for a result.

- **Display terminals** — `visualize(format)` and its shortcuts (`to_table()`, `to_markdown()`,
  `to_json()`, `to_mermaid()`, ...) return rendered text. When a query ends without any terminal
  step, `visualize()` is applied implicitly, which is what makes bare one-liners readable in the
  REPL:

  ```text
  graphersal> g.V().hasLabel("person");            // rendered with the default visualizer
  graphersal> g.V().hasLabel("person").visualize(); // the same thing, written out
  ```

  The strategy is `/set format <format>` for the session, `g.set_visualizer(V.<Format>)` for every
  pipeline derived from `g`, and an explicit `visualize(V.<Format>)` argument for a single query —
  each one overriding the previous.

### Graph Schema & Visualizers in REPL

A schema is plain data: our envelope (`mode`, `vertices`, `edges`, `meta`) around standard JSON
Schema 2020-12 label schemas, so a Pydantic `model_json_schema()` can be pasted as a label's
schema. The full reference is the book page "Schemas" (`book/src/users_guide/schema_enforcement.md`).

#### 1. The seven schema methods
```text
graphersal> g.get_schema();                 // the stored schema, () without one
graphersal> g.infer_schema();               // the schema the data satisfies (mode "none")
graphersal> g.set_schema(g.infer_schema()); // store a schema (a map or JSON text); {} removes it
graphersal> g.patch_schema(#{mode: "closed"});          // JSON Merge Patch onto the stored schema
graphersal> g.validate_schema(#{mode: "closed"}).violations;   // dry run against the data
graphersal> g.validate_schema_patch(#{mode: "open"}).compatible;
graphersal> g.diff_schema(#{}).changes;     // changes from the stored schema, each with "compatible"
```
Every method also has its camelCase spelling (`getSchema`, `setSchema`, ...). A schema written
inline as JSON text:
```text
graphersal> g.set_schema(`{"mode": "open", "vertices": {"person": {"schema": {
              "type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}}}}`);
```

#### 2. Visualizer formats
A schema shown as the final value of a session renders as a table; the session format picks
another view (`table`, `markdown`, `tree`, `json`, `jsonschema`, `mermaid`, `plantuml`):
```text
graphersal> /set format mermaid
graphersal> g.infer_schema();

graphersal> /set format jsonschema
graphersal> g.get_schema();
```
The `--format <fmt>` flag of the CLI does the same for a one-shot run.

#### 3. Meta-graph traversal from a schema
```text
graphersal> let s = g.infer_schema();
graphersal> s.to_graph().V().values("label_name").toList();
graphersal> s.subgraph("person").V().values("label_name").toList();   // one label and its neighbours
```

#### 4. Pipeline schema inference & direct pipeline visualization
```text
// Infer the schema of a sub-pipeline and render it with a terminal format shortcut:
graphersal> g.V().hasLabel("person").infer_schema().to_table();
graphersal> g.V().hasLabel("person").infer_schema().to_markdown();
graphersal> g.V().hasLabel("person").infer_schema().to_mermaid();
graphersal> g.V().hasLabel("person").infer_schema().visualize(V.Mermaid);
graphersal> g.V().hasLabel("person").infer_schema().to_json_schema();
graphersal> g.V().hasLabel("person").infer_schema().to_json();

// Direct element formatting shortcuts on standard traversal pipelines:
graphersal> g.V().hasLabel("person").to_json();
graphersal> g.V().hasLabel("person").to_table();
graphersal> g.V().hasLabel("person").to_markdown();

// Element table visualization with column selection, exclusion, wrapping & row numbers:
graphersal> g.V().hasLabel("person").to_table(["name", "age"]);
graphersal> g.V().hasLabel("person").to_table(["!id", "!label"]);
graphersal> g.V().hasLabel("person").to_table(#{ columns: ["name", "age"], truncate: 20, row_numbers: true });
graphersal> g.V().hasLabel("person").to_table(#{ columns: ["!id", "!label"], max_width: 30, truncate: 50, row_numbers: true });
graphersal> g.V().union([__.identity(), __.infer_schema()]).toList();
```

### Graph Sources & Overriding `g` in REPL

By default, the REPL session starts with a pre-populated in-memory graph. You can create or reassign the traversal source variable `g` at any time using `let g = ...;` (or `g = ...;`):

- **TinkerPop Modern graph:**
  ```text
  graphersal> let g = GraphSource::tinkerpop_modern();
  // Or using the short helper:
  graphersal> let g = tinkerpop_modern();
  ```

- **Filesystem-like sample tree** (vertex ids are paths such as `"root/src/main.rs"`, labels
  `directory`/`file`/`symlink`, `contains` and `points_to` edges):
  ```text
  graphersal> let g = GraphSource::file_tree();
  // Or using the short helper:
  graphersal> let g = file_tree();
  ```

- **Empty graph (for constructing graphs from scratch):**
  ```text
  graphersal> let g = GraphSource::empty();
  // Or using the short helper:
  graphersal> let g = empty();
  ```

- **From a GraphML (`.xml` / `.graphml`) file:**
  ```text
  graphersal> let g = GraphSource::file("data.graphml");
  // Or using the short helper:
  graphersal> let g = file("data.graphml");
  ```
  GraphML is the only graph file format; any other extension is refused with a diagnostic.

### Importing & Exporting Graphs in REPL

In the REPL, you can import and export graph data using `GraphSource` and `g`:

**Importing into an Existing Graph:**
```text
graphersal> g.import_graphml("data.graphml");
```

**Exporting / Saving a Graph:**
By default, exports produce compact output. Use `.with("beautify", true)` before terminal operations to produce formatted XML:
```text
graphersal> g.export_graphml("output.xml");

// Formatted (beautified) export options via .with():
graphersal> g.with("beautify", true).export_graphml("output_pretty.xml");
graphersal> g.with("io.graphml.beautify", true).export_graphml("output_pretty.xml");
graphersal> g.with("io.beautify", false).with("io.graphml.beautify", true).export_graphml("out.xml");
```

> **Note on `with()` placement:** `.with(...)` is a source modulator that configures options on `g` and must be chained **before** terminal actions (e.g., `g.with("beautify", true).export_graphml("...")`). Call order: `g.with(...).export_graphml(...)`.

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE-APACHE](LICENSE-APACHE) for details.

Graphersal stays open source under Apache-2.0. Contributions are welcome under the same licence with a DCO
sign-off (`git commit -s`); see [CONTRIBUTING.md](CONTRIBUTING.md). Use of the project name is described in
[TRADEMARKS.md](TRADEMARKS.md).
