# Step - group

`group()` is a reducing barrier: it collects the whole stream into one map. The first `by()` gives
the key of each traverser, the second `by()` its value; without `by()` the key is the traverser itself
and the value is the list of its members.

```rhai,graph=modern
g.V().group().by(T.label).by("name").next()
// {"person": ["marko", "vadas", "josh", "peter"], "software": ["lop", "ripple"]}
g.V().group().by(T.label).by(__.count()).next()      // {"person": 4, "software": 2}
```

How the value `by()` reduces the members of one key:

- a property key, `T.id`, `T.label`, `T.key`, `T.value`, a `jpath(..)` or a bare `by()`: a list
  with one projected value per member (`map(projection).fold()` in TinkerPop). A key whose members
  all lack the property maps to `[]`;
- `by(Count)` or `by(__.count())`: the member count;
- a traversal without a barrier: the first result of the last member that produced one;
- a traversal with a barrier (`fold()`, `count()`, `sum()`, `order()`, `limit()`, ...): the traversal
  runs once over all members of the key, and the key maps to its first result. A key whose run
  produces nothing is left out.

## The side-effect form: `group("key")`

`group("a")` does the same grouping into the side effect `a` and passes every traverser on
unchanged (TinkerPop's `GroupSideEffectStep`). Read the map with `cap("a")`, `select("a")` or
`where(P.within("a"))`.

```rhai,graph=modern
g.V().group("a").by(T.label).by("name").cap("a").next()
// {"person": ["marko", "vadas", "josh", "peter"], "software": ["lop", "ripple"]}
g.V().hasLabel("person").as("p").out("created").
  group("a").by("name").by(__.select("p").values("age").sum()).cap("a").next()
// {"lop": 96, "ripple": 32}
```

- The map grows while the traversal runs: a reader later in the same traversal sees everything
  the step has collected so far. `g.V().groupCount("a").select("a")` sees all six vertices (one
  execution of the step handles the whole batch), `local(groupCount("a").select("a"))` a map that
  grows by one per vertex, as in TinkerPop.
- A value traversal with a barrier is re-run over all members of each key that grew, at the end of
  every execution of the step; inside `repeat()` that is once per iteration.
- The side effect exists, as `{}`, as soon as the step has run, even when no traverser reached it:
  `g.V().has("no").groupCount("a").cap("a")` is `{}`.
- Several group steps may fill one key (`union(__.groupCount("m").by(..), __.groupCount("m").by(..))`)
  as long as they reduce a key the same way; a key that holds another kind of side effect
  (`aggregate("a")`, `withSideEffect("a", ..)`, `tree("a")`) is an error. **Deviation from
  TinkerPop**, which merges the map into whatever the key holds.

Rust: `group()`, `group_side_effect("a")` (and the same on `__`/`AnonymousTraversal`).

See also [group_count](group_count.md) and [by](by.md).
