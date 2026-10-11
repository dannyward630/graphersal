# Introduction

**Graphersal** is an in-memory graph database and traversal engine written in Rust. You ask it
questions in **Gremlin**, the query language of [Apache TinkerPop&trade;](https://tinkerpop.apache.org):
start somewhere in a graph, walk along its edges, filter, group and count.

```rhai,graph=modern
// who created software together with marko?
g.v().has("name", "marko").as("me")
  .out("created").in("created")
  .where(P.neq("me"))
  .path().by("name")
```

![The web playground: a query, its results and the graph](images/getting_started/playground-overview.png)

## What you get

- **Gremlin, two ways.** Build traversals in Rust (`g.v(None).has_label("person").out("knows")`)
  or write them as text in the Gremlin DSL, in snake_case or in Gremlin's own camelCase
  (`hasLabel`, `outE`). The text runs the same everywhere: in the browser, in the command line,
  from Python and through an AI agent.
- **A playground in the browser.** The real engine, compiled to WebAssembly: write a query, see a
  table, the graph drawing, the schema and the optimized plan. Nothing to install.
- **A command line tool.** One-shot queries for shell scripts, an interactive REPL with completion
  and help, and a local server that brings the playground UI to a graph on your disk.
- **AI agents on your graph.** The server speaks the Model Context Protocol (MCP), so Claude Code
  or another MCP client can query the graph you are looking at. The playground has its own AI Chat
  tab as well (experimental).
- **Safe by construction.** Every traversal is atomic: it commits completely or leaves nothing
  behind. Schemas (a subset of JSON Schema) are enforced on write. Limits, timeouts and
  permissions make it safe to run queries you did not write.
- **Durable when you need it.** A graph lives in memory, and the Store keeps it on disk: every
  commit is in a write-ahead log, you can go back to any point in time, fork, roll back, back up
  and repair. The file format is a public specification.
- **Fast and observable.** A rule-based optimizer rewrites each query; `profile()` shows the
  optimized plan with timings, traverser counts and memory per step.
- **Open for integration.** Rust and Python APIs, saved (named, parameterized) queries stored in
  the graph, change capture with commit hooks, GraphSON and GraphML files, and a storage trait to
  run the whole engine on a storage of your own.

## Where to go next

| You want to ... | Read |
|---|---|
| try it right now | [Try It in the Browser](getting_started/playground_tour.md) |
| run queries from a terminal or a script | [The Command Line](getting_started/cli.md) |
| let an AI agent work with your graph | [Dev Server, MCP and AI Chat](getting_started/server_mcp_ai.md) |
| understand graphs and Gremlin first | [Why a Graph?](concepts/why_graphs.md) |
| use it from code | [Rust Quick Start](library/rust.md), [Python Quick Start](library/python.md) |
| look up a step or a rule | [The Gremlin DSL](dsl/writing_queries.md), [Step Reference](traversal/step_reference.md) |
| find a command | [Command Cheat Sheet](users_guide/command_cheat_sheet.md) |

> **Graphersal is not a complete copy of TinkerPop.** It follows the TinkerPop specification for
> every step it implements and checks that with TinkerPop's own test suite, but some steps are not
> implemented yet and a few behave differently on purpose. Each difference is documented where the
> feature is described and collected in [TinkerPop Deviations](for_developers/tinkerpop_deviations.md);
> see [Graphersal and TinkerPop](concepts/graphersal_and_tinkerpop.md).
