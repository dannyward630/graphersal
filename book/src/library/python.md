# Python Quick Start

The Python package `graphersal` embeds the engine into the Python process. Queries are Gremlin
text, the same DSL as everywhere else, and results come back as plain Python values (`list`,
`dict`, `int`, `float`, `str`, `bool`, `None`, `uuid.UUID`).

## Install

Build the extension from the repository (Python 3.10 or newer, one wheel covers every later
version):

```bash
cd bindings/py-graphersal
python3.12 -m venv .venv && .venv/bin/pip install maturin
VIRTUAL_ENV=$PWD/.venv .venv/bin/maturin develop --release     # installs `graphersal` into .venv
```

`maturin build --release` writes a wheel to install elsewhere; details are in
`bindings/py-graphersal/README.md`.

## Query

```python
import graphersal

graph = graphersal.Graph.tinkerpop_modern()      # or graphersal.Graph() for an empty graph

graph.query('g.V().has("name", "marko").out("knows").values("name")')
# ['josh', 'vadas']

graph.query('g.V().hasLabel("person").group().by("name").by("age").next()')
# {'josh': [32], 'marko': [29], 'peter': [35], 'vadas': [27]}
```

A traversal the script returns is run for you; inside a longer script, end each traversal with a
terminal (`.next()`, `.to_list()`, `.iterate()`).

## Parameters

Pass values as parameters instead of pasting them into the text; they arrive as script variables:

```python
graph.query('g.V().has_label("person").has("age", P.gt(min_age)).values("name")',
            params={"min_age": 30})
# ['josh', 'peter']
```

## Write

Every query is one unit: when it fails, nothing it wrote stays, and the error is a
`graphersal.GraphersalError` with the diagnostic and its help text.

```python
graph.execute('g.addV("person").property("name", "ann").property("age", 41).iterate()')

try:
    graph.execute('g.V().has("name", "ann").property("age", 42).fail("stop").iterate()')
except graphersal.GraphersalError as err:
    print(err)                                   # the failing step and how to fix it

graph.query('g.V().has("name", "ann").values("age").next()')
# 41: the failed query changed nothing
```

## Files and persistence

```python
graph.export_graphson("people.json")             # GraphSON 3.0; TinkerGraph reads it
graph = graphersal.Graph.from_graphson("people.json")

with graphersal.Store.create("data") as store:   # a graph on disk, every commit durable
    store.graph.execute('g.addV("person").property("name", "bob").iterate()')
```

## Where to go from here

- `bindings/py-graphersal/README.md`: every method, schemas, access policies, limits;
- [Python](../persistence/python.md): snapshots, the journal and the Store;
- [Writing Queries](../dsl/writing_queries.md): the DSL.
