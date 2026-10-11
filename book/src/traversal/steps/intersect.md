# Step - intersect

`intersect(list)` keeps the elements of the list a traverser holds that are **also** in `list`.
The result is a set: each element at most once, in the order of its first occurrence; `null` is an
element like any other (see [List Functions](list_functions.md)).

```
graphersal> g.V().values("age").fold().intersect([27, 40])
[27]

graphersal> g.inject(["marko", "dave"]).intersect(__.V().values("name").fold())
[marko]
```

Rust: `intersect(vec![27, 40])` and `intersect_traversal(__::v(None).values("name").fold())`.
