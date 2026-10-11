# Step - disjunct

`disjunct(list)` is the symmetric difference: the elements that are in exactly one of the list a
traverser holds and `list`. The result is a set: first the incoming elements that are not in
`list`, then the elements of `list` that are not in the incoming list, each at most once (see
[List Functions](list_functions.md) for element equality).

```
graphersal> g.V().values("age").fold().disjunct([27, 40])
[29, 32, 35, 40]

graphersal> g.inject(["a", (), "b"]).disjunct(["a", "c"])
[null, b, c]
```

Rust: `disjunct(vec![27, 40])` and `disjunct_traversal(__::v(None).values("age").fold())`.
