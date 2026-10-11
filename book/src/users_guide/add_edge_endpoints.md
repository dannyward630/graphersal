# addE() Endpoints: property(), Traversals and Side Effects

`addE()` takes everything Apache TinkerPop's `AddEdgeStep` takes: `property()` calls between
`addE()` and `from()`/`to()`, an endpoint given as a traversal, as a step label or as a side-effect
key, and a label given as a traversal.

```groovy
// property() before from()/to() (as() labels in between are fine too)
g.addV("p").as("a").addV("p").as("b").
  addE("knows").property("weight", 1).from("a").to("b")

// an endpoint is a traversal: its first result is the vertex (or the id of a vertex)
g.addE("knows").from(__.V("1")).to(__.V().has("name", "vadas"))
g.V("1").addE("knows").to(__.V("2").id())

// a traversal runs once per incoming traverser, and may create the vertex itself
g.addV().as("first").
  repeat(__.addE("next").to(__.addV()).inV()).times(5).
  addE("next").to(__.select("first"))

// an endpoint is a side-effect key
g.withSideEffect("b", "2").V("1").addE("likes").to("b")

// the label is a traversal: its first result must be a string
g.V().has("name", "marko").as("a").outE("created").as("b").inV().
  addE(__.select("b").label()).to("a")
```

## Rules

- **Where `from()`/`to()` attach.** They modulate the `addE()` before them, with any number of
  `property()` calls, `as()` labels and other `from()`/`to()` calls in between. After any other step
  (`g.V().property("a", 1).from("x")`) they still name that step in the error.
- **Endpoints.** `from()`/`to()` take a step label, a side-effect key, a vertex id, or a traversal.
  A name is looked up as the object last labelled so on the path (`as()`), then as a side effect of
  that key (a single value; an `aggregate()`/`store()` list or a set bucket is an error), then as a
  vertex id. A traversal endpoint is run on the incoming traverser (for `g.addE(..)` on a seed
  holding `null`) and its first result is used; no result is an error.
- **What an endpoint value may be.** A vertex, or the id of a vertex present in the graph: a string
  or an integer (its decimal text). An edge, a path, a list, a map, a property element (`properties()`)
  and `null` are errors, and so is an id that matches no vertex. Nothing is created when an endpoint
  fails.
- **Labels.** `addE(traversal)` runs the traversal on the incoming traverser before the endpoints
  and uses its first result, which must be a string; no result is an error.
- **One endpoint per traversal.** The traversal of one `from()`/`to()` yields one vertex; to connect
  many vertices, run `addE()` per traverser.
- **Incoming traverser.** With `from()` only, the incoming vertex is the `to` end; with `to()` only, it
  is the `from` end; with neither, the edge is a self-loop. With both, the incoming traverser is
  only the parent of the traversals and the owner of the path labels.

Rust API: `from_by(traversal)`, `to_by(traversal)` and `add_e_by(traversal)` on the traversal source,
on `__` (`add_e_by`) and on anonymous traversals.

## Deviations

Documented from TinkerPop 3.8.2 behaviour as seen in its feature files; `AddEdgeStep` itself was not
read.

- **A plain vertex id is an endpoint.** A name that is neither a path label nor a side-effect key is
  treated as the id of a vertex (`to("2")`). TinkerPop resolves a string like `select(key)` only and
  fails when nothing is found.
- **A list of names creates one edge per name.** `to(["2", "3"])` creates two edges; TinkerPop takes
  exactly one endpoint. (Rhai's `from([])` / `to([])` is read as no argument and is ignored.)
- **Endpoint ids compare as strings.** `2` and `"2"` are the same id, consistent with
  [the id rule](predicates.md) for `hasId()` and `V()`.
