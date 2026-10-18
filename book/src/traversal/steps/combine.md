# Step - combine

`combine(list)` appends a second list to the list a traverser holds. Duplicates and order are
kept. The incoming value must be a list (see [List Functions](list_functions.md) for the input and
argument rules shared by all list functions).

```rhai
graphersal> g.V().values("name").fold().combine(["dave", "marko"])
[marko, vadas, lop, josh, ripple, peter, dave, marko]

graphersal> g.V().out().out().path().by("name").combine(["dave"])
[marko, josh, ripple, dave]
[marko, josh, lop, dave]

graphersal> g.inject([1, 2]).combine(__.constant([2, 3]))
[1, 2, 2, 3]
```

Rust: `combine(vec!["dave"])` and `combine_traversal(__::constant(vec![2, 3]))`.

The set counterpart, which keeps each element once, is [merge](merge.md).
