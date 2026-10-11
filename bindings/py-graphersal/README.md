# `graphersal` — Python bindings for Graphersal

`graphersal` embeds the [Graphersal](../../README.md) in-memory graph traversal engine into
Python. It does not expose every Gremlin step as a Python method; instead you write Gremlin
script text (the same DSL `graphersal` uses) and run it against a `Graph`. The result is converted
into native Python values (`list`/`dict`/`int`/`float`/`str`/`bool`/`None`/`uuid.UUID`).

Graph files are GraphSON 3.0 (the primary exchange format for Gremlin users) and GraphML.

## Installing

### From source (this repository), for development

You need a Rust toolchain and Python 3.10+ (the binding targets `abi3-py310`). The macOS system
`python3` is 3.9 and too old; use e.g. Homebrew's `python3.12`. Keep the virtualenv outside the
repository or at `bindings/py-graphersal/.venv` (ignored by version control). One-time setup, from
the repository root:

```bash
/opt/homebrew/bin/python3.12 -m venv bindings/py-graphersal/.venv
bindings/py-graphersal/.venv/bin/pip install maturin pytest
```

Build the extension into that virtualenv as `graphersal` (repeat after every Rust change in the
binding or the engine), then run the tests:

```bash
cd bindings/py-graphersal
VIRTUAL_ENV=$PWD/.venv .venv/bin/maturin develop      # add --release for an optimized build
.venv/bin/python -m pytest tests/                     # Python suite, including examples/
cd ../..
PYO3_PYTHON=$PWD/bindings/py-graphersal/.venv/bin/python cargo test -p py-graphersal   # Rust unit tests
```

`PYO3_PYTHON` must name a Python >= 3.10 for every Cargo command that builds this crate
(`cargo test -p py-graphersal`, workspace-wide `cargo check`/`cargo clippy`); otherwise exclude it
with `--exclude py-graphersal`. `import graphersal` is then available in the virtualenv
(`source bindings/py-graphersal/.venv/bin/activate`).

### Building a wheel to install elsewhere

```bash
cd bindings/py-graphersal
maturin build --release        # writes a wheel to target/wheels/
pip install ../../target/wheels/graphersal-*.whl
```

The wheel targets the stable ABI (`abi3-py310`), so one wheel per platform covers Python 3.10 and
newer — no per-Python-version rebuild is needed.

## Quick start

```python
import graphersal

graph = graphersal.Graph()
graph.execute('g.addV("person").property("name", "Alice").next()')
names = graph.query('g.V().values("name").to_list()')
print(names)  # ['Alice']
```

Script text follows this project's Gremlin/Rhai syntax: string literals use **double quotes**
(single quotes are Rhai character literals), and both snake_case (`has_label`) and Gremlin
camelCase (`hasLabel`) step spellings work. Inside a script a traversal only runs when it ends
in a terminal — `.next()`, `.to_list()` or `.iterate()` (runs for the side effects, returns
`None`) — including mutations: a statement `g.addV("person");` in the middle of a script builds a
traversal object and does nothing; add `.iterate()`. A traversal
the script *returns* without a terminal (also inside a returned list or map) is run for you, as
if it ended in `.to_list()`: `graph.execute('g.V().values("name")')` returns the list of names.

`execute()` and `query()` are exactly the same method under two names; `query` is offered for
read-oriented call sites (it does not restrict the script by itself, see
[Access policy](#access-policy)). Both take the keywords `atomic=` ([Units of
atomicity](#units-of-atomicity)) and `policy=` ([Access policy](#access-policy)); type stubs
(`graphersal.pyi`) ship with the package.

The DSL terminals `.profile()` and `.execute()` come back as dicts with snake_case keys
(durations as integer nanoseconds, `duration_ns`); see the book page "Profiling and execute()":

```python
metrics = graph.execute('g.V().out().profile()')
metrics["metrics"][0]["counts"]["traverser_count"]

run = graph.execute('g.V().values("name").sum().execute(#{profile: true})')
run["error"]["step_id"]   # "2": the failing step, numbered like metrics[..]["id"]
run["profile"]            # the profile up to the failing step
```

A DSL `.execute()` never raises: its error is part of the dict. A failing `.profile()` raises
`GraphersalError` like any other terminal.

## Creating a graph

```python
import graphersal

# An empty graph.
graph = graphersal.Graph()

# The standard TinkerPop "modern" toy graph (6 vertices, 6 edges), useful for trying things out.
graph = graphersal.Graph.tinkerpop_modern()
```

## Loading and exporting GraphSON and GraphML

GraphSON 3.0 is what TinkerPop's `g.io("graph.json").write()` produces (one vertex per line):

```python
import graphersal

graph = graphersal.Graph.from_graphson("path/to/graph.json")
graph.export_graphson("path/to/out.json")   # TinkerGraph loads it with g.io(path).read()
```

GraphSON values carry their types (`uuid`, lists and maps come back as such). Data Graphersal
cannot hold yet (dates such as `g:Date`, other unsupported types) is skipped, never converted,
and named in one `UserWarning`. Multi-valued properties become lists, meta-properties go into the
`_meta` property; see the book's GraphSON page for every mapping.

GraphML:

```python
graph = graphersal.Graph.from_graphml("path/to/graph.xml")

# ... query or mutate ...

graph.export_graphml("path/to/out.graphml")
```

GraphML carries no schema. Without one, values come back with their GraphML types: a UUID, a list
or a map is a string. To restore declared types (`uuid`, `array`, `object`), pass the schema to the
load: it is set on the new, empty graph first, then the file is imported into it as one unit (in
`open`/`closed` mode a violation fails the load with the usual report):

```python
typed = graphersal.Graph.from_graphml(
    "path/to/out.graphml",
    schema=open("path/to/schema.json").read(),   # a dict or JSON text
    mode="closed",                               # optional: replaces the schema's own mode
)
```

`Graph.from_graphson` takes the same `schema=`/`mode=` keywords. To add a file to a graph you
already have, import it into the live graph; each import is one unit (a file that fails to parse
or violates the schema leaves the graph unchanged):

```python
graph.import_graphml("path/to/more.graphml")    # declared types restored from graph's schema
graph.import_graphson("path/to/more.json")      # skipped data: one UserWarning
```

## Saving, journal and recovery (packed snapshots)

Graphersal's own binary format keeps everything (every value type, the schema, the graph's
commit position) in one file, and a journal makes every committed change durable before it is
final (book chapter "Keeping a Graph on Disk", page "Python"):

```python
graph.save("g.gsnap")                                     # {"commit_seq", "vertex_count", "edge_count"}
graph = graphersal.Graph.load("g.gsnap")                  # no schema re-validation, checksummed

graph.start_journal("g.wal")                              # durability="every_commit" | "os" | seconds
graph.execute('g.addV("person").property("name", "ann").next()')
graph.mark("after-ann")                                   # a named point in the journal

graph, report = graphersal.Graph.recover("g.gsnap", ["g.wal"])            # to the end
graph, report = graphersal.Graph.recover("g.gsnap", ["g.wal"], mark="after-ann")
```

`Graph.recover` also takes `commit_seq=` or `time=` (microseconds since the Unix epoch). An
interrupted last write is ignored (`report["torn_tail"]`); damage raises `GraphersalError` with
its location. The recovered graph has no journal: `save` and `start_journal` again to continue.

## Schemas

A schema is a `dict` (or JSON text) in the library's schema format: an envelope with `mode`
(`"none"`, `"open"`, `"closed"`), `vertices`, `edges` and a free `meta`, around standard JSON
Schema 2020-12 label schemas, so a Pydantic `Model.model_json_schema()` can be used as a label's
schema (drop `date-time` formats and `dict[str, T]` fields first: no date or map type before
0.2.0). The full reference is the book page "Schemas".

```python
graph = graphersal.Graph.tinkerpop_modern()
graph.set_schema(graph.infer_schema(), mode="closed")  # freeze the structure the graph has
schema = graph.infer_schema()          # what the data satisfies; stores nothing
schema["mode"] = "closed"
report = graph.validate_schema(schema) # dry run: {"violations", "diff", "compatible"}
graph.set_schema(schema)               # validates the data first; on violations nothing changes
graph.patch_schema({"vertices": {"person": {"schema": {"properties": {"email": {"type": "string"}}}}}})
graph.diff_schema(schema)              # {"changes": [...], "compatible": bool}
graph.get_schema()                     # the stored schema, or None
graph.set_schema({})                   # removes it
```

| Method | Returns |
|---|---|
| `get_schema()` | the stored schema, or `None` (never inferred) |
| `set_schema(schema, mode=None)` | `None`; replaces the stored schema (`mode`: `"none"`/`"open"`/`"closed"` replaces the schema's own mode) |
| `patch_schema(patch)` | the new schema (JSON Merge Patch, RFC 7396) |
| `validate_schema(schema, mode=None)`, `validate_schema_patch(patch)` | `{"violations": {...}, "diff": [...], "compatible": bool}`; applies nothing |
| `infer_schema()` | the schema the data satisfies (mode `"none"`) |
| `diff_schema(schema)` | `{"changes": [...], "compatible": bool}` from the stored schema |

Every method takes `policy=` like `execute()` (read methods need `("read", "schema")`, `set`/`patch`
need `("update", "schema")`). A violation raises `GraphersalError` with the aggregated report and a
`help`. A schema `default` is an annotation only: it is never written into the data nor
substituted on read. To make a key required that existing elements lack, backfill first in one
atomic script (patch to optional, write the values, patch to required):

```python
graph.execute('''
    g.patch_schema(#{vertices: #{person: #{schema: #{properties: #{email: #{type: "string"}}}}}});
    g.V().hasLabel("person").property("email", "unknown").toList();
    g.patch_schema(#{vertices: #{person: #{schema: #{required: ["name", "age", "email"]}}}});
''', atomic=True)
```

The schema methods are also in the script DSL (`g.get_schema()`, `g.set_schema(..)`, ...), and a
run reports what it changed: `execute()` of a script ending in `.execute()` returns
`changes: {"data": bool, "schema": bool, "definitions": bool}`.

## Saved queries

Named, parameterized, read-only queries stored in the graph (book page "Saved Queries"):

```python
graph.define_query({
    "name": "older_than",
    "folder": "people",
    "params": {"age": {"schema": {"type": "integer", "minimum": 0}, "default": 30}},
    "body": 'fn older_than(age) { g.v().has_label("person").has("age", P.gt(age)).values("name") }',
})
graph.call_query("older_than", {"age": 29})   # ["josh", "peter"]; graph.query stays execute's alias
graph.queries("people")                        # [{"name", "folder", "description", "params", "status", ...}]
graph.get_query("older_than")["body"]
graph.drop_query("older_than")                 # True
```

Each wraps the DSL call (`g.define_query`, `g.query`, `g.queries`, `g.get_query`,
`g.drop_query`) and takes `policy=` like `execute()`.

## Creating vertices and edges

```python
import graphersal

graph = graphersal.Graph()

# addE()'s from()/to() take step labels (set with as(...)), not nested traversals.
graph.execute(
    'g.addV("person").property("name", "Alice").property("age", 30).as("a")'
    '.addV("person").property("name", "Bob").as("b")'
    '.addE("knows").from("a").to("b").next()'
)
```

## Query examples

```python
import graphersal

graph = graphersal.Graph.tinkerpop_modern()

# A plain list of scalar values.
names = graph.query('g.V().hasLabel("person").values("name").to_list()')
# ['marko', 'vadas', 'josh', 'peter']

# elementMap() turns each vertex/edge into a dict.
people = graph.query('g.V().hasLabel("person").elementMap().to_list()')
# [{'id': '1', 'label': 'person', 'name': 'marko', 'age': 29}, ...]

# next() returns a single value (or None if the traversal yields nothing).
count = graph.query('g.V().hasLabel("person").count().next()')
# 4

# A one-hop traversal.
who_marko_knows = graph.query(
    'g.V().has("name", "marko").out("knows").values("name").to_list()'
)
# ['josh', 'vadas']
```

## Parameterized queries

Pass a `params` dict of `int`, `float`, `str`, `bool`, `None` or `uuid.UUID` values. Each one is
bound as both a script variable of the same name and an entry of the `params` map, so user input
never needs to be spliced into the script string:

```python
import graphersal

graph = graphersal.Graph.tinkerpop_modern()

names = graph.query(
    'g.V().hasLabel("person").has("age", params.min_age).values("name").to_list()',
    params={"min_age": 32},
)
# ['josh']
```

`list`/`dict` values in `params` are not supported and raise `GraphersalError`, as does an `int`
outside the int64 range (it is never widened to a float). A key that is not a `str` raises
`TypeError`.

## Error handling

Every script/engine failure — parse errors, a failed cast, a broken GraphML file — raises
`graphersal.GraphersalError` or one of its subclasses. When the underlying error carries
actionable guidance, it is available as `.help` (`None` otherwise):

```python
import graphersal

graph = graphersal.Graph.tinkerpop_modern()

try:
    graph.execute('UUID.fromString("not-a-uuid")')
except graphersal.GraphersalError as err:
    print(err)        # Error: Cast exception: expected type [uuid], got type 'string' ...
    print(err.help)    # A UUID is written in canonical form: 8-4-4-4-12 lowercase hex digits, ...
```

The failures a program handles on purpose have their own subclasses (all of them are
`GraphersalError`s and keep `.help`):

| Exception | Raised when |
|---|---|
| `PermissionDeniedError` | the script makes a request (an action on a resource) its `policy` does not allow, or uses a file outside the file root; the denied traversal did not run |
| `ReturnedClosureError` | a script run as one unit (`atomic=True`, `dry_run`) returns a closure; everything it changed was rolled back |
| `CommitRejectedError` | a commit hook vetoed the commit (hooks can only be registered from Rust in 0.1.0); everything was rolled back |

```python
try:
    graph.execute('g.addV("x").next()', policy=graphersal.AccessPolicy.read_only())
except graphersal.PermissionDeniedError as err:
    print(err)        # Error: Permission denied for 'add_v' (Create Data(vertex "x")): ...
```

## Units of atomicity

A failing script never leaves a half-applied traversal behind. By default every traversal (every
terminal such as `.next()` or `.to_list()`) is its own unit: a failing traversal rolls back only
itself, and the statements before it stay — the TinkerPop behaviour:

```python
graph = graphersal.Graph.tinkerpop_modern()
script = 'g.addV("x").next(); g.V().values("name").sum().next()'   # sum() over strings fails
try:
    graph.execute(script)
except graphersal.GraphersalError:
    pass
graph.query('g.V().count().next()')   # 7: the first statement stayed, the second one left nothing
```

`atomic=True` makes the whole script one unit: when it fails, everything it changed is rolled
back; when it succeeds, all of it stays. While it runs, no other reader or writer of the graph
does. Inside it, each traversal is a savepoint, so a script that catches a failure itself
(`try { ... } catch (err) { }`) goes on with only that traversal undone.

```python
try:
    graph.execute(script, atomic=True)
except graphersal.GraphersalError:
    pass
graph.query('g.V().count().next()')   # still 7: nothing of the failing script stayed
```

A script run with `atomic=True` may not return a closure (`ReturnedClosureError`): return data.
A traversal it returns without a terminal runs after the commit, as its own unit.
Auto-generated ids are never reused, also not after a rollback.

## Dry run: preview the changes of a script

`graph.dry_run(script, params=None, *, policy=None)` runs the script as one unit and then
always rolls it back. It returns a `graphersal.DryRunResult`, an immutable object with two
attributes: `.result`, what `execute` would have returned, and `.changes`, the would-be changes as
plain Python data. The graph is left exactly as it was.

```python
graph = graphersal.Graph.tinkerpop_modern()
run = graph.dry_run(
    'g.V().has("name", "marko").property("age", 30).next();'
    'g.V().has("name", "lop").drop().to_list();'
    'g.V().count().next()'
)
run                         # DryRunResult(result=5, changes=<5 mutations>)
run.result                  # 5
changes = run.changes
changes["commit_seq"]       # 0: never committed
changes["committed_at"]     # 0: never committed (a commit's time: microseconds since the epoch)
changes["node_sequence"]    # 6: the vertex auto-id sequence after the run (ids are never reused)
[m["op"] for m in changes["mutations"]]
# ['set_property', 'drop_edge', 'drop_edge', 'drop_edge', 'drop_vertex']
changes["mutations"][0]
# {'op': 'set_property', 'element': {'kind': 'vertex', 'id': '1'}, 'key': 'age', 'before': 29, 'after': 30}
graph.query('g.V().count().next()')   # 6: untouched
```

The change set is the serialized form of the engine's `ChangeSet` (book page "Transactions"),
JSON-compatible as it is (`json.dumps(changes)` works):

- `commit_seq` and `committed_at` are `0` (never committed); `node_sequence` and `edge_sequence`
  are the auto-id sequences after the run: auto ids a dry run handed out are not handed out again.
- `mutations` lists the changes in execution order, each a dict with an `"op"` key:
  `add_vertex`, `drop_vertex`, `add_edge`, `drop_edge`, `set_property`, `remove_property`,
  `add_label`, `remove_label`, `set_schema`, with the values before and after. Dropping a vertex
  lists the drops of its incident edges first.
- Values JSON cannot hold carry a type tag instead of being guessed from their content: a
  `uuid.UUID` is `{"@type": "g:UUID", "@value": "550e8400-e29b-41d4-a716-446655440000"}`. A
  user map that itself has an `"@type"` key is wrapped as
  `{"@type": "graphersal:Object", "@value": {...}}`.

A failing script raises like `execute` and returns no change set; a returned closure raises
`ReturnedClosureError`. No commit hook runs. A traversal the script returns without a terminal
runs inside the dry run, so its result reflects the previewed changes. The change set is data only:
Python cannot register commit hooks (callbacks on every commit) in 0.1.0.

## Access policy

`policy=` limits what a script may do. Every step and file operation of a script makes a request,
an **action** on a **resource**; a `graphersal.AccessPolicy` allows or denies them by kind, plus
an optional file root:

| Resource | Requested by |
|---|---|
| `"data"` | `read`: every step that reads the graph (`V()`, `out()`, `values()`, `count()`, ...); `create`: `addV`, `addE`, `mergeV`, `mergeE`, importing GraphML; `update`: `property`, `removeProperty`, `mergeV`/`mergeE`; `delete`: `drop` |
| `"schema"` | `read`: `get_schema`, `infer_schema`, `diff_schema`, `validate_schema(_patch)` (methods and DSL); `update`: `set_schema`, `patch_schema` |
| `"file"` | `read`: `import_graphml`, `GraphSource::file`; `create`: `export_graphml`; `execute`: Rhai `import` |
| `"option"` | `update`: administrative `g.with()` options (`evaluationTimeout`, `repeat.max_loops`, optimizer switches, ...) |
| `"custom"` | never requested by the engine (for servers embedding the Rust library) |

```python
from graphersal import Graph, AccessPolicy

graph = Graph.tinkerpop_modern()

graph.query('g.V().count().next()', policy=AccessPolicy.read_only())          # 6
graph.execute('g.addV("x").next()', policy=AccessPolicy.read_only())          # PermissionDeniedError

AccessPolicy.deny_all()                                   # nothing: the script can only compute
AccessPolicy.allow_all()                                  # everything
AccessPolicy(("read", "data"), ("create", "data"))        # exactly these (action, resource) pairs
AccessPolicy.read_only().allow("*", "data")               # a new value; policies are immutable
AccessPolicy.allow_all().deny("*", "file").deny("update", "option")

# File access limited to one directory: relative paths resolve under it, paths that leave it
# ("..", absolute paths elsewhere, symbolic links pointing out) are denied.
uploads = AccessPolicy.read_only().allow("*", "file").with_file_root("/srv/graphs")
graph.execute('g.export_graphml("snapshot.graphml")', policy=uploads)   # /srv/graphs/snapshot.graphml
```

A denied script fails with `PermissionDeniedError` before the denied traversal runs; the message
names the step and the request (`Permission denied for 'add_v' (Create Data(vertex "x"))`). A file
root does not allow file access by itself.

**The default allows everything**: without `policy=`, a script may do everything, like the Rust
library's own default. A Python program is a trusted local host — it already has the graph
object, file access and `Graph.export_graphml`. Whenever a script comes from somewhere else (a
user, a request, an LLM), pass an explicit policy, e.g. `AccessPolicy.read_only()`, together with
resource limits (`graph.set_limits(...)`).

## Resource limits

A new `Graph` runs scripts under the library's SAFE limits: 10 million script operations, 16 MiB
strings, 100 000 list items and map entries, and 128 levels of nested lists and maps in one value.
`graph.set_limits(...)` changes them for every later script of that graph; each keyword overrides
one limit, `0` meaning unlimited, and `unlimited=True` starts from no limits:

```python
graph.set_limits(max_operations=1_000_000, max_string_size=1 << 20)
graph.set_limits(unlimited=True, max_value_depth=256)
```

`max_value_depth` is never unlimited (`0` means its default, 128): deeper values are read
recursively and would overflow the native stack. Scripts run on a worker thread with a 64 MiB
stack (one per calling Python thread), so the scripting engine's own recursion over nested values
has room. A limit hit raises `GraphersalError` whose `help` names the limit to raise. Traversal
limits (`evaluationTimeout`, `traversal.max_materialized_bytes`, `traversal.max_string_bytes`, ...)
are set per query with `g.with(key, value)`; see the book's "Resource Limits" page.

A graph can carry a memory budget (off by default): every traversal on it that would take the
memory in use over the budget raises `GraphersalError` and is rolled back, instead of the process
being killed for lack of memory. Pass a number of bytes, or `"auto"` for the cgroup memory limit
of the process (a Linux container) minus a headroom (`memory_headroom`, default a tenth of the
limit, at least 64 MiB):

```python
graph = graphersal.Graph(memory_limit=2 << 30)
graph = graphersal.Graph.from_graphml("data.graphml", memory_limit="auto")
```

The extension installs Graphersal's tracking allocator as its own global allocator, so the budget
compares the exact live heap of the extension's Rust code (every graph and query of the process,
read with one atomic load, no walk of the graph). That count does not include Python objects:
with `memory_limit="auto"` the Rust heap is compared with a limit for the whole process, so
`memory_headroom` must also cover the Python side (the interpreter, your own data, the converted
results). The same allocator makes Memory profiles exact:
`g.v().out().profile_with(ProfileType.Memory)` reports `"source": "tracked"` bytes per step
(a bare `profile()` collects no memory data). The cost is a few counter increments per
allocation of the extension.

## GraphRAG example

[`examples/graphrag.py`](examples/graphrag.py) ingests a small knowledge graph of text entities and
relations, extracts the 2-hop neighbourhood of a question entity with
`repeat(__.out()).times(2)` and prints one JSON line per path (`entity`, `type`, `text`, `path`,
`hops`), ready to paste into an LLM prompt:

```bash
python examples/graphrag.py "Marie Curie"
```

## Concurrency

`execute()`/`query()`/`dry_run()` release the GIL for the duration of the script, so a
long-running query on one `Graph` does not stall other Python threads (e.g. a thread-pool server,
or `asyncio` driving this through `run_in_executor`). The underlying graph still serializes
concurrent mutations from multiple threads; an `atomic=True` script and a dry run hold the graph's
write lock for the whole script.

## What's out of scope (for now)

- Commit hooks from Python (a Python callback on every commit); change sets are available only
  as data through `dry_run`.
- Positional parameters (`$1`, `$2`, ...).
- `list`/`dict` values in `params`.
- Any graph file format other than GraphML.
