# Step - group_count

`groupCount()` counts the traversers per key into one map. Its single `by()` gives the key; without
it the key is the traverser itself. A second `by()` is an `InvalidModulator` error.

```rhai,graph=modern
g.V().values("age").groupCount().next()              // {29: 1, 27: 1, 32: 1, 35: 1}
g.V().groupCount().by(T.label).next()                // {"person": 4, "software": 2}
```

`groupCount("key")` is the side-effect form: it counts into the side effect `key` and passes every
traverser on (TinkerPop's `GroupCountSideEffectStep`). The same rules as for
[`group("key")`](group.md#the-side-effect-form-groupkey) apply.

```rhai,graph=modern
g.V().out("created").groupCount("a").by("name").cap("a").next()   // {"lop": 3, "ripple": 1}
g.V("1").repeat(__.groupCount("m").by(__.loops()).out()).times(3).cap("m").next()
// {0: 1, 1: 3, 2: 2}
```

Rust: `group_count()`, `group_count_side_effect("a")`; the DSL spells both `groupCount` and
`group_count`.
