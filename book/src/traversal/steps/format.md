# Step - format

`format(pattern)` builds a string from `pattern`. Each `%{name}` placeholder is replaced by a value
read from the incoming traverser.

```rhai
graphersal> g.V().format("%{name} is %{age} years old").to_list()
"marko is 29 years old"
"vadas is 27 years old"
"josh is 32 years old"
"peter is 35 years old"
```

The two software vertices have no `age`, so they are filtered out.

## Resolving a placeholder

`%{name}` is looked up in this order:

1. the property `name` of an incoming vertex or edge;
2. as [`select(name)`](../../users_guide/select_pop.md) resolves it: the key `name` of an incoming
   map (`elementMap()`, `project()`), then a side-effect key, then the last path value labelled
   `name` (`as("name")`).

```rhai
graphersal> g.V().hasLabel("person").as("a").values("name").as("p1").select("a").in("knows").format("%{p1} knows %{name}").to_list()
"vadas knows marko"
"josh knows marko"
```

`%{_}` takes the next `by()` modulator. The `by()`s form a ring: they are used round robin, and the
ring restarts for every traverser. Without a `by()`, `%{_}` is the incoming value itself.

```rhai
graphersal> g.V().format("%{name} has %{_} connections").by(__.bothE().count()).to_list()
"marko has 3 connections"
"vadas has 1 connections"
"lop has 3 connections"
"josh has 3 connections"
"ripple has 1 connections"
"peter has 1 connections"

graphersal> g.V("1").format("%{_} %{_} %{_}").by(__.constant("hello")).by("name").to_list()
"hello marko hello"
```

When a placeholder resolves to nothing or to `null` (a missing property or label, an unproductive
`by()`, a `null` map or path value), the traverser is filtered out. A *stored* `null` property is
present, so it is written as the text `null`, as in TinkerPop (`g.V("1").property("nick", ())` then
`g.V("1").format("%{name} aka %{nick}")` gives `"marko aka null"`). `%%{x}` is
not a placeholder and stays in the output as written, and so does a `%{` without a closing `}` on
the same line.

## Value text

A value is written as [`as_string()`](../../users_guide/type_conversion.md) writes it: `29`, `true`,
a UUID as its canonical text, a vertex as `v[1]`. A list or a map has no string form here, so
`format()` fails with a cast error. This is the same deviation as for
[`conjoin()`](conjoin.md#deviation-from-tinkerpop): TinkerPop writes Java's `toString` text, and no
suite scenario covers the case.
