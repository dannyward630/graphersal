# Step - split

`split(separator)` turns the string a traverser holds into a list of strings, cut around
`separator`. The separator itself is not kept.

```
graphersal> g.inject("that", "this", "test", ()).split("h").to_list()
["t", "at"]
["t", "is"]
["test"]
()
```

- A `null` (`()` in Rhai, `None` in Rust) separator splits on runs of whitespace:
  `g.inject(" hello  world ").split(())` gives `["hello", "world"]`.
- An empty separator splits into single characters: `g.inject("abc").split("")` gives
  `["a", "b", "c"]`.
- A non-empty separator follows Apache Commons' `splitByWholeSeparator`, which TinkerPop uses.
  Adjacent separators produce no empty string, but a separator at the very end leaves a trailing
  `""`: `"a--b"` split by `"-"` is `["a", "b"]`, and `"ath"` split by `"h"` is `["at", ""]`.
- A `null` input stays `null`. Any other value that is not a string is a cast error.

## Scope.local

`split(Scope.local, separator)` splits each string of the list a traverser holds, which gives a
list of lists. A `null` element stays `null`, and a single string is split as by the unscoped
step.

```
graphersal> g.V().hasLabel("person").values("name").order().fold().split(Scope.local, "a").to_list()
[["josh"], ["m", "rko"], ["peter"], ["v", "d", "s"]]
```

Rust API: `split("h")`, `split(None)`, `split_scoped(Scope::Local, "a")`.
