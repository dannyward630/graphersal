# Step - conjoin

`conjoin(delimiter)` joins the elements of the list a traverser holds into one string, with
`delimiter` between two elements. `null` elements are skipped (no delimiter is written for them),
and an empty list gives the empty string. The incoming value must be a list (a `fold()`, a
`path()`, a list constant or property; see [List Functions](list_functions.md)).

```rhai
graphersal> g.V().values("age").order().fold().conjoin(";")
27;29;32;35

graphersal> g.V("1").out().path().by("name").conjoin(" -> ")
marko -> josh
marko -> lop
marko -> vadas

graphersal> g.inject(["a", (), "b"]).conjoin("+")
a+b
```

Each element is written as [`as_string()`](../../users_guide/type_conversion.md) writes it:
numbers (`27`, `2.5`), booleans, UUIDs and strings as their text, a vertex as `v[1]`, an edge as
`e[7][1-knows->2]`.

## Deviation from TinkerPop

A nested list or a map inside the list has no string form in Graphersal: `conjoin()` fails with a
cast error there, where TinkerPop writes Java's `toString` text (`[a, b]`, `{k=v}`). Graphersal
never produces Java formats as data text (the same rule as for
[`asString()` of a map](../../for_developers/tinkerpop_deviations.md)); unfold or `conjoin()` the
inner list first. No scenario of the TinkerPop suite covers the case.
