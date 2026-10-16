# The Command Line

The `graphersal` binary runs Gremlin queries from a terminal. It has three modes: a one-shot
query runner for shell scripts, an interactive REPL, and a local server (next chapter).

Install it from a prebuilt binary or with `cargo install graphersal-cli`
([Installation](installation.md)), or build it in a clone of the repository:

```bash
cargo build -p graphersal-cli --release      # target/release/graphersal
```

## One-shot queries

Pass a query with `-e`; the result goes to stdout and nothing else does:

```text
$ graphersal -e 'g.v().has("name", "marko").out("knows").values("name")'
╭───────╮
│ value │
├───────┤
│ josh  │
│ vadas │
╰───────╯
```

Without `--graph` it runs on the TinkerPop "modern" sample. `--graph` also takes `empty`, `large`
(about 110,000 elements), `tree`, a GraphSON or GraphML file, a snapshot or a Store directory:

```bash
graphersal --graph people.json -e 'g.v().count()'
echo 'g.v().values("age").max().next()' | graphersal --graph modern     # the query from stdin
graphersal -e 'g.v().has_label("person").values("name")' --format json  # also markdown, mermaid, ...
```

A query that fails prints its diagnostic on stderr and exits with `1`, so a shell script or a CI
job can check a graph:

```bash
# fails (exit 1) when a server has no owner
graphersal --graph export.json -e 'g.v().has_label("server").has_not("owner").fail("a server without an owner")'
```

## The REPL

Without a query, `graphersal` starts an interactive shell with history, Tab completion of steps
and tokens, and the documentation of every function:

```text
$ graphersal
graphersal> g.v().has_label("person").values("name").limit(2);
╭───────╮
│ value │
├───────┤
│ marko │
│ vadas │
╰───────╯
graphersal> let n = g.v().count().next();
graphersal> n * 10;
60
graphersal> /help values
values  (step, terminal or source method of a traversal)
  values()
  values(arg1: any)
  ...
Yields the values of the given (or all) properties; a jpath() key reads a nested value.

Example: `g.v(1).values("name", "age").to_list()` returns `["marko", 29]`
```

Variables live across lines: a script is ordinary Rhai code around Gremlin traversals, so loops,
functions and maps work as well.

## See the plan

`profile()` prints the plan the optimizer made, with timings and traverser counts:

```text
$ graphersal -e 'g.v().has_label("person").out("created").count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"])                                           1       0       4    5.667µs     8.66
out("created")                                                  1       4       4    5.125µs     7.83
count()                                                         1       4       1      125ns     0.19
                                                      TOTAL:             execute:   65.417µs    16.69
=====================================================================================================
Optimizer rules applied: source_filter_pushdown
```

## Keep a graph on disk

`graphersal store` creates and manages a Store, a graph on disk with a write-ahead log, and every
mode opens one with `--graph <dir>`:

```bash
graphersal store create data --from people.json    # a new store from a graph file
graphersal --graph data -e 'g.add_v("person").property("name", "ann").iterate()'   # durable
graphersal store info data
```

More: [The graphersal store Command](../persistence/store_cli.md). Every option of the tool is in
`graphersal --help` and on the [Command Cheat Sheet](../users_guide/command_cheat_sheet.md).
