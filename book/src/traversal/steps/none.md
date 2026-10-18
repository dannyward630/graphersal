# Step - none

`none(P)` keeps a list traverser when **no** element of the list satisfies the predicate. It is
the sibling of `any(P)` and `all(P)` (TinkerPop 3.8 `NoneStep`). The step that filtered out every
traverser, called `none()` before TinkerPop 3.8, is now [`discard()`](discard.md).

```rhai
g.V().values("age").fold().none(P.gt(40)).to_list()   // [[29, 27, 32, 35]]
g.V().values("age").fold().none(P.gt(30)).to_list()   // []
g.inject([], [1, 2]).none(P.eq(3)).to_list()          // [[], [1, 2]]
g.inject(7).none(P.eq(8)).to_list()                   // []
```

- The list is a `fold()`, an injected or constant list, or a stored list property.
- An empty list passes: none of its elements matches.
- A value that is not a list (a number, a string, `null`, a map, a vertex) never passes, whatever
  the predicate.
- `null` elements are compared like any other value: `none(P.eq(null))` drops `[null, 1]`.

Rust: `GraphTraversalSource::none(predicate)`, `__::none(predicate)`.
