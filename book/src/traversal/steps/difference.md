# Step - difference

`difference(list)` keeps the elements of the list a traverser holds that are **not** in `list`.
The result is a set: each element at most once, in the order of its first occurrence. Elements
compare like `dedup()` compares traversers (see [List Functions](list_functions.md)).

```
graphersal> g.V().values("age").fold().difference([27, 29])
[32, 35]

graphersal> g.V().out().out().path().by("name").difference(["ripple"])
[marko, josh]
[marko, josh, lop]

graphersal> g.inject(["a", (), "b"]).difference(["a", "c"])
[null, b]
```

Rust: `difference(vec![27, 29])` and `difference_traversal(__::v(None).values("age").fold())`.
