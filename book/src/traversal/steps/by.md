# Step - by

`by()` modulates the step right before it: what to read from each traverser, or how to sort or
group it. The arguments and the steps that take `by()` are described in
[The by() Modulator](../../users_guide/by_modulator_single.md).

## `by("label")` and `by(T.label)`

A string is always a property key: `by("label")` and `by("id")` read a property of that name (as
in TinkerPop) and are unproductive on an element without one. The element's label and id are the
tokens `by(T.label)` and `by(T.id)`. See
[Properties Named id or label](../../users_guide/id_and_label_keys.md).

```rhai,graph=modern
g.V().groupCount().by(T.label).next()    // {"person": 4, "software": 2}
g.V().groupCount().by("label").next()    // {}: no vertex has a property named label
```

## `by()` without an argument

A bare `by()` is the identity: TinkerPop's `by()` is `by(__.identity())`, and Graphersal lowers it to
exactly that child traversal.

```rhai,graph=modern
g.V("1").as("a").values("name").as("b").select("a", "b").by(T.label).by().next()
// {"a": "person", "b": "marko"}
g.V().values("name").fold().order(Scope.local).by()          // the same as order(Scope.local)
g.V().repeat(__.out()).times(2).path().by().by("name").by("lang")
```

In `group()` a bare value `by()` collects the members of each key as a list, as TinkerPop's
`GroupStep` does for an identity value traversal (`map(identity).fold()`), instead of keeping one
member: `g.V().group().by(T.label).by()` maps `person` to its four vertices.

Rust: `by(__::identity())`.
