# Dropping Elements

`drop()` removes the incoming vertices or edges from the graph (a vertex together with its edges) and
emits nothing. `sideEffect(__.drop())` removes them and lets the traverser continue. To remove only
properties, see [Removing Properties](removing_properties.md).

```groovy
g.V().hasLabel("software").drop()                 // remove all software vertices and their edges
g.V("1").outE("knows").drop()                     // remove two edges
g.V("1").sideEffect(__.drop()).count()            // remove and keep counting: 1
```

Dropping an element that is already gone is not an error; the step is idempotent.

## An element dropped earlier in the same query

A traversal can still *hold* an element after it was dropped: through a step label (`as("a")` ...
`select("a")`), a side effect (`aggregate("x")` ... `cap("x")`), a path, or a lazy value taken before
the drop (`values("name")`, `id()`, `label()` are read only when they are returned). Graphersal never
lets such a handle reach another element, even when a new element is stored in the freed place
(generational handles): the removed element reads as *removed*.

| Use of a removed element | Result |
|---|---|
| property reads (`values()`, `properties()`, `has(k, v)`, `by("k")`) | absent: nothing is emitted, `has()` filters it out |
| adjacency (`out()`, `inE()`, `outV()`, ...) | empty |
| filters that need its data (`hasLabel()`, `hasId()`, `has(T.id, ..)`) | filtered out |
| `id()`, `label()`, returning it (materialization), `elementMap()`, a lazy `values()`/`id()` read after the drop | error `ElementRemoved` |
| writes (`property()`, `addE()` to it) | error `ElementRemoved` |
| `drop()` again | no-op |

An error here fails the whole traversal, and a failing traversal is rolled back: the drop itself is
undone too (see [Transactions](transactions.md)).

```groovy
g.V("1").as("a").sideEffect(__.drop()).addV("ghost").property("name", "GHOST").select("a").values("name")
// []  (the new vertex may take the freed storage place, but never the dropped one's handle)

g.V("1").outE().as("e").sideEffect(__.drop()).select("e").label()
// error: The edge was removed earlier in this query; its id, label and value are no longer available
```

To keep data of an element you drop, materialize it before the drop, for example with
`elementMap()`, `valueMap()` or `project()`:

```groovy
g.V("1").as("a").elementMap().as("m").select("a").sideEffect(__.drop()).select("m")
```

### Deviation from TinkerPop

TinkerGraph keeps `id()` and `label()` of a removed element on its element object, so
`select("a").id()` after the drop still returns `1` there, and properties and edges are gone, as here.
Graphersal keeps no data of a removed element (its storage place may already hold another
element), so `id()`, `label()` and returning the element fail with `ElementRemoved` instead. The same
holds for a lazy `values(k)`/`id()` taken before the drop and returned after it. `outV()`/`inV()` of a
removed edge return its endpoints in TinkerGraph and nothing here. See
[TinkerPop Deviations](../for_developers/tinkerpop_deviations.md).
