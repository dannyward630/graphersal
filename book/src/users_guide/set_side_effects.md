# Set Side Effects

`withSideEffect("a", Set.of(..))` declares a side effect as a set: `aggregate("a")` and `store("a")`
keep each value once, and `cap("a")`/`select("a")` return it as a list without duplicates. It
differs from Apache TinkerPop 3.8.2 in these details.

## A set is a property of the side effect, not a value type

- **TinkerPop:** a set is a first-class value (`{"alice"}` in Groovy, `toSet()`, `GType.SET`).
- **Graphersal:** there is no set value. The set belongs to the side-effect bucket. `cap("a")`
  returns a plain array (insertion order, no duplicates); `toSet()`, `GType.SET` and set
  literals anywhere else are not implemented.
- **DSL spelling:** `Set.of("a", "b")` (0 to 10 arguments), `Set.ofList(["a", "b"])` and the
  `Set::of(..)`/`Set::of_list([..])` forms. The Groovy literal `{..}` does not exist in Rhai. In Rust:
  `GraphTraversalSource::with_side_effect_set`.
- `Set.of(..)` accepts only plain values (strings, numbers, booleans, UUIDs, null); a vertex, a
  traversal or a map is a script error.

## Order

- **TinkerPop:** the iteration order of a set is the one of its implementation (`HashSet`:
  unspecified).
- **Graphersal:** first-seen order, seed values first.

## Equality of members

- Members are compared by value. `1` and `1.0` are different members.
- A property value collected from `values()` is compared by its value, not by the element it came
  from, so two vertices named "x" contribute one member. A property element collected from
  `properties()` is compared by its element and key (TinkerPop's property id), so those two
  vertices contribute two members (an edge property is also kept by identity, unlike TinkerPop; see
  [Property Elements](property_elements.md)). Elements (vertices, edges) keep their identity.
- Bulk is invisible: a traverser of bulk `n` inserts its value once, with merging on or off.

```rhai,graph=modern
g.withSideEffect("a", Set.of()).V().both().values("name").aggregate("a").cap("a").next()
// ["josh", "lop", "vadas", "marko", "peter", "ripple"]
g.withSideEffect("a", []).V().both().values("name").aggregate("a").cap("a").next()
// a list seed keeps duplicates
```

## Empty side effects

`aggregate("a")`, `store("a")` and `subgraph("a")` declare their key when they run, even if they
collect nothing (an empty upstream, or a `by()` that resolves no value). `cap("a")` then returns
the empty collection, as in TinkerPop, and `where(P.within("a"))` is false and
`where(P.without("a"))` true for it. Only a key that no step declares (a typo) is an error.

- `cap("sg")` of a `subgraph("sg")` that collected nothing is an empty graph (see
  [Subgraphs](#subgraphs)).

## Subgraphs

`subgraph("sg")` collects the edges that pass through it, and `cap("sg")` returns them as a **graph
value**, as in TinkerPop:

```rhai
let sub = g.e(0).subgraph("sg").cap("sg").next();
sub.v().to_json()     // the two endpoint vertices of edge 0, with all their properties
sub.e().count().next()   // 1
```

The graph value is a traversal source: `sub.v()`, `sub.e()`, `sub.addV(..)` work as on `g`.

What the graph holds:

- the selected edges and **both endpoint vertices** of each (a vertex shared by several edges is
  copied once; a vertex without a selected edge is not included),
- ids, the whole **label set** of every vertex (not only the first label) and **all properties at
  full depth** (nested objects and arrays),
- the schema the source graph stores (mode and declarations, copied; clear it with
  `set_schema` on the copy if you do not want it). A source with no stored schema gives a copy
  that keeps inferring from its own data.

`subgraph()` needs the label: `subgraph()` alone is an error that names `subgraph("sg")` + `cap("sg")` and the
short `to_graph()` terminal. The stream must be made of **edges**: a vertex (or any other value) reaching
`subgraph("sg")` is an error whose help points at `outE()`/`inE()`/`bothE()`
(`g.v(1).bothE().subgraph("sg").cap("sg")`). The kind is checked when each traverser arrives, so this
also holds inside a nested traversal (`local(__.out().subgraph("sg"))` fails, `local(__.outE().subgraph("sg"))`
works); an empty stream is fine and gives the empty graph.

It is a **snapshot**: a copy, never a view. Changing the subgraph does not touch the source, and
later changes of the source do not reach the subgraph. Ids are preserved (so the same vertex has
the same id in both graphs) and stay unique. Bulk is invisible: an edge that reaches `subgraph()`
once or a thousand times is collected once. While the stream runs only edge handles are
collected; the copy is built when `cap()` runs.

`to_graph()` is the short form (a Graphersal extension, TinkerPop has no such step): a terminal
that builds the same graph from a stream of edges, without a label.

```rhai,graph=modern
g.e(0).to_graph()                 // one edge and its two vertices
g.v(1).bothE().to_graph()         // all edges of vertex 1, with their vertices
g.v(1).to_graph()                 // error: to_graph() needs edges
```

A stream of anything else than edges (vertices, values) is an error that names the edge forms; a
stream with no results is an empty graph. In the Rust API `to_graph()` returns the shared graph
handle (`Arc<Graph>`), and `cap("sg").next()` lists the graph as
`{"vertices": [..], "edges": [..]}`. A script that ends on a graph value (`g.e(0).to_graph()`,
`cap("sg").next()`) shows that same listing in `graphersal` and the playground, cut to the display
limits (see [Graph values](displaying_results.md#graph-values)); in the script the value stays a
graph you traverse.

### Writing into your own graph

By default `cap("sg")` builds a new in-memory `TraversalGraph`. To write into a graph you own,
declare it as the side effect, as in TinkerPop:

```rhai
let target = g.e(0).to_graph();                        // or GraphSource::empty()
g.withSideEffect("sg", target).v(1).outE("created").subgraph("sg").toList();
target.e().count().next()                              // 2: the edge copied before, plus one
```

The step writes through the graph mutation API when it has run, and `cap("sg")` returns the target
itself. A vertex id the target already holds is reused and an edge id it already holds is skipped,
so writing the same edges twice changes nothing. The target's own stored schema is kept (the
source schema is copied only into a target that stores none). The target must not be the graph
being traversed (`g` itself): that fails with an error instead of deadlocking.

The target must also be a `TraversalGraph`. Every graph a script builds is one (`GraphSource::*`,
`to_graph()`, `cap("sg")`, `_g`), but a host may serve its own `g` from another `GraphStorage`.
`withSideEffect("sg", g)` with such a graph fails when the step is built, with
`GraphError::Unsupported` naming the target's storage type. To get a subgraph of a foreign graph,
let it land in a graph of the script (`g.E().hasLabel("knows").to_graph()`) and copy what you
need from there.

- **Deviations from TinkerPop:** the default graph is a Graphersal `TraversalGraph`, not a
  TinkerGraph, and an explicit target must be a graph value of this kind. `to_graph()` and the
  schema copy are extensions. In the Rust API a graph value is shown by `next()`/`to_list()` as
  its `{"vertices", "edges"}` listing; use `to_graph()` for the graph itself. See
  [TinkerPop deviations](../for_developers/tinkerpop_deviations.md).

## Reduced values

`withSideEffect("a", 0, Operator.sum)` declares a side effect that `aggregate("a")` and `store("a")` combine with
an `Operator` instead of collecting a list: `g.withSideEffect("a", 1, Operator.sum).V().aggregate("a").by("age").cap("a")`
is `124`. See [Sack and Operators](sack_and_operators.md#reducer-operators) for the rule and the deviations.
