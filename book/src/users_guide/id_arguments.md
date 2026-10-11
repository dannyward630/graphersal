# Id Arguments of V(), E() and hasId()

`g.V(ids...)`, `g.E(ids...)`, the nested `__.V(ids...)`/`__.E(ids...)` and `hasId(ids...)` take
element ids. This page lists where Graphersal differs from Apache TinkerPop 3.8.2.

## Arrays flatten in the first position only

- **TinkerPop:** `hasId(id, otherIds...)` treats a list in a later position as one id that matches
  nothing; a list as the first argument is flattened.
- **Graphersal:** the same observable result, modelled explicitly. Only the first argument
  flattens an array (recursively); an array in any later position contributes the empty id, which
  matches no element.
- **Why:** the TinkerPop scenario `HasId::g_VX1X_out_hasIdX2_listXid3_id4XX` depends on it.

```rhai,graph=modern
g.V(["1", "2"]).values("name").toList()      // "marko", "vadas"
g.V("1", ["2", "3"]).values("name").toList() // "marko" only
```

## null is the empty id

- **TinkerPop:** a null id is not a defined element id.
- **Graphersal:** `()` (the DSL's null) contributes the empty id, which matches nothing, so
  `g.E(())` and `hasId(())` stay empty instead of widening to every element.

## Elements and maps as ids (extension)

A map with an `"id"` key, as returned by `next()`/`toList()`, contributes that id, so
`g.E(e)` works with a previously fetched edge. Any other map is rejected with an
`ArgumentMismatch` error instead of being turned into an id that never matches.

## Predicates

`hasId(P)` evaluates the predicate over the id; see
[Predicates: Comparison and Resolution](predicates.md).
