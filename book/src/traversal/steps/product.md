# Step - product

`product(list)` is the Cartesian product of the list a traverser holds and `list`: one
two-element list `[a, b]` per pair, `a` from the incoming list (outer loop) and `b` from `list`
(inner loop). Duplicates are kept; an empty side gives an empty list (see
[List Functions](list_functions.md) for the input rules).

```
graphersal> g.inject(["a", "b"]).product([1, 2])
[[a, 1], [a, 2], [b, 1], [b, 2]]

graphersal> g.V().values("age").order().limit(2).fold().product(__.V().hasLabel("software").values("name").fold()).unfold()
[27, lop]
[27, ripple]
[29, lop]
[29, ripple]
```

Rust: `product(vec![1, 2])` and `product_traversal(__::v(None).values("name").fold())`.
