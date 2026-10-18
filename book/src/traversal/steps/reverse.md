# Step - reverse

`reverse()` reverses a string or a list. One step covers both.

- A string comes back with its characters in reverse order (Unicode scalar values).
- A list comes back with its elements in reverse order. This covers a `fold()`, a `path()` (which
  becomes a list) and a list constant or property.
- Every other value passes through unchanged: `null`, numbers, booleans, maps, vertices and edges.

```rhai
graphersal> g.inject("feature", ()).reverse().to_list()
"erutaef"
()

graphersal> g.V().out().out().path().by("name").reverse().to_list()
["ripple", "josh", "marko"]
["lop", "josh", "marko"]

graphersal> g.V().values("age").fold().reverse().to_list()
[35, 32, 27, 29]
```

`g.V().values("age").reverse()` returns the ages unchanged, because a number is neither a string
nor a list.
