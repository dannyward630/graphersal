# Step - merge

`merge()` has two forms.

**Lists.** `merge(list)` is the union of the list a traverser holds and `list`: a set, each element
at most once, the incoming elements first (see [List Functions](list_functions.md)). Use
[combine](combine.md) to keep duplicates.

```rhai
graphersal> g.V().values("age").fold().merge([27, 40])
[29, 27, 32, 35, 40]
```

**Maps.** When the traverser holds a map (`project()`, `elementMap()`, `valueMap()`, a map
constant), `merge(map)` adds the argument's entries to it; an entry of the argument replaces the
incoming entry with the same key.

```rhai
graphersal> g.V().hasLabel("person").project("name").by("name").merge(#{kind: "person"})
╭────────┬───────╮
│ kind   │ name  │
├────────┼───────┤
│ person │ marko │
│ person │ vadas │
│ person │ josh  │
│ person │ peter │
╰────────┴───────╯

graphersal> g.V("1").elementMap().merge(__.V("3").elementMap())
╭─────┬────┬──────────┬──────┬──────╮
│ age │ id │ label    │ lang │ name │
├─────┼────┼──────────┼──────┼──────┤
│ 29  │ 3  │ software │ java │ lop  │
╰─────┴────┴──────────┴──────┴──────╯
```

The two forms do not mix: a list with a map argument, or a map with a list argument, is an error.
The DSL writes a map constant as `#{kind: "person"}`; Rust: `merge(ElementProperty::Object(..))`
and `merge_traversal(__::v("3").element_map(None))`.

`merge()` is unrelated to the upsert steps [`mergeV()`/`mergeE()`](../../users_guide/upserts.md).
