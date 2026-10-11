# union() as a Start Step

`g.union(branches...)` starts a traversal without `V()`. It differs from Apache TinkerPop 3.8.2
in two details.

## The seed is null

- **TinkerPop:** the start union has no incoming traverser of its own.
- **Graphersal:** a root traversal whose first step is `union()` gets exactly one seed traverser
  holding `null`; child traversals are never seeded. A branch such as `__.identity()` therefore
  yields that `null`: `g.union(__.identity())` returns one `null`.

## A branch that opens with inject() is its own start

- **Graphersal:** such a branch runs over an empty source instead of the seed.
- **Why:** otherwise `g.union(__.inject(1), __.inject(2))` would echo the seed and return
  `null, 1, null, 2` instead of `1, 2`. In the middle of a traversal `inject()` still emits the
  incoming traverser as well.

```rhai,graph=modern
g.union(__.inject(1), __.inject(2)).toList()   // 1, 2
g.union(__.identity()).toList()                // null
g.union(__.V("1"), __.V("4")).values("name").toList()
```

Nested `V()`/`E()` in a branch start from the incoming traverser and extend its path.
