# graphersal-cli

The command line front end of [Graphersal](https://crates.io/crates/graphersal), an in-memory
graph traversal engine modelled on Apache TinkerPop Gremlin (compatible with and inspired by it,
not endorsed by the Apache Software Foundation). It installs the binary `graphersal`: an
interactive REPL and a one-shot query runner.

```bash
cargo install graphersal-cli

graphersal                                   # interactive REPL (graphersal>)
graphersal -e 'g.v().has_label("person").values("name").to_list()'
echo 'g.v().values("age").max().next();' | graphersal --in --graph modern
graphersal --graph ./mygraph.graphml --format mermaid -e 'g.infer_schema()'
graphersal --graph ./tinkerpop-modern.json -e 'g.v().count().next()'   # GraphSON 3.0
```

stdout carries only the result, stderr the diagnostics. Exit codes: `0` ok, `1` the script failed,
`2` invalid invocation, `141` stdout closed before every result was written (`| head -1`). `--graph` takes `modern` (default), `empty`, `large`, or a GraphSON
(`.json`, `.jsonl`, `.graphson`) or GraphML (`.xml`, `.graphml`) file; data a GraphSON import
skips (dates, unsupported types) is listed in a note on stderr. `--schema schema.json
[--schema-mode open|closed|none]` sets a schema (the schema format) on the new graph before the
file is imported against it (declared `uuid`/`array`/`object` values come back typed, the mode is
enforced; a violation exits with `1`); with a sample graph it is applied to it, with a Store or a
`.gsnap` (they carry their own schema) it is refused.
Run `graphersal --help` for display limits (`--max-rows`, `--no-limit`), `--timeout` and the
script resource limits (`--safe-limits`, `--max-operations`, ...).

## Dev server (DEV mode, unstable, local only)

```bash
graphersal --graph ./mygraph.json --server --port 8080 --timeout 5000
# open http://127.0.0.1:8080/
```

`--server` serves the Graphersal playground UI (embedded in the binary; `--ui-dir <DIR>`, default
`./playground` when it exists, names a directory whose files win over the embedded copy) on the loopback (`127.0.0.1`, and `[::1]` when available) and answers its engine calls from this process:
queries, profiles, schema, graph drawing, Stop (cancels the running query, which rolls back), and
an explicit "Save to file" back to a `--graph` file (and the schema back to a `--schema` file). The limit options (`--timeout`,
`--memory-limit`, `--max-*`, ...) apply to every query. No authentication and an internal HTTP API
that changes without notice. Nothing persists unless `--graph` is a Store (below): then every
commit is durable before the query answers. Build without it: `--no-default-features` (feature
`dev-server`).

`--mcp` adds a Model Context Protocol endpoint on the same graph and session, so an AI agent works
next to you in the browser (`--mcp-read-only`: the agent may only read; `--mcp-token <secret>`:
it must send `Authorization: Bearer <secret>`; `--mcp-query-tools`: every saved query is also a
tool of its own, besides `list_saved_queries` and `run_saved_query`):

```bash
graphersal --graph modern --server --port 9000 --mcp
claude mcp add --transport http graphersal http://127.0.0.1:9000/mcp   # then start a new claude session
```

The browser reloads by itself when the agent (or another tab) changed the graph. Tools,
resources and protocol details: the book page "Dev Server and MCP"
(`book/src/persistence/dev_server.md`).

## The Store (persistence)

```bash
graphersal store create data/ --from modern          # one directory (or data.gstore: one file) = one graph
graphersal --graph data/ -e 'g.add_v("person").to_list()'   # every commit is durable before it returns
graphersal --graph data/ --server                    # the dev server with a Store menu
graphersal store help                                # info, list, marks, mark, checkpoint, verify, fork,
                                                     # rollback, attic, backup, restore, export, prune,
                                                     # compact, compact-advice, convert, repair
```

Reference: the book's "Persistence" part (`book/src/persistence/store_cli.md` for every command);
every command of the tool on one page: `book/src/users_guide/command_cheat_sheet.md`.

Licensed under the Apache License, Version 2.0 (see `LICENSE-APACHE` and `NOTICE`).
