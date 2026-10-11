# Path Windows: from() and to() on path(), simplePath() and cyclicPath()

`path()`, `simplePath()` and `cyclicPath()` take `from(label)`, `to(label)` and `by(...)`, as the
Apache TinkerPop steps do. `from()` and `to()` cut the path of each traverser down to a *window*
between two labelled positions; `by()` projects the objects of that window before the step uses
them.

```groovy
g.V().as("a").out().as("b").out().as("c").path().from("b").to("c").by("name")
// ["josh", "ripple"], ["josh", "lop"]

g.V().as("a").out().as("b").out().as("c").simplePath().by(T.label).from("b").to("c").path().by("name")
// ["marko", "josh", "ripple"], ["marko", "josh", "lop"]
```

## Rules

- **Window.** The window starts at the position labelled by `from()` (position 0 without
  `from()`) and ends at the position labelled by `to()` (the last position without `to()`), both
  included. The two bounds are found independently of each other. `from("a").to("a")` is the one
  object labelled `a`; two labels on the same position (`as("a", "b")`) work the same way.
- **`path()`** returns the windowed objects. The traverser's own path is unchanged, so a later
  `path()` is the whole path again.
- **`simplePath()` / `cyclicPath()`** test only the window (after `by()`). The traverser continues
  with its whole path; only the test sees the window.
- **`by()`** is a ring over the window: the first `by()` applies to the first object *of the
  window*, the next one to the second object, and so on, cycling. A `by()` that yields no value
  for an object (a missing property) drops the traverser, for `path()`, `simplePath()` and
  `cyclicPath()` alike. `simplePath().by(...)` compares the projected values, so two different
  vertices with the same age are a repeat.
- **Order of the modulators** does not matter: `path().by("age").from("a").to("b")` and
  `path().from("a").to("b").by("age")` are the same.
- **Cost.** The plain forms (no modulator) test the path in place without copying it. The modulated
  forms materialize the path once per traverser. `.profile()` shows the modulators, for example
  `simple_path().from("b").to("c").by(T.label) [path: full]`.

## Deviations

These are choices where the engine had to decide; none is a difference from TinkerPop's
behaviour on a feature scenario.

- **Repeated labels.** If several positions carry the same label (the same `as("a")` inside a
  `repeat()`, or `as("a")` twice), both `from()` and `to()` use the **last** position carrying
  the label. This is TinkerPop's `Path.subPath` (checked against the 3.7.2 byte code, since no
  Java sources are available offline), so a window may start at the last `a` even when an earlier
  `a` exists. A `to()` position before the `from()` position is an error.
- **Unknown label.** A `from()` or `to()` label that no position carries is an error
  (`Property '<label>' not found on element type 'path'`). TinkerPop raises an
  `IllegalArgumentException` too; only the text differs.
- **Repeated `from()`/`to()`.** A second `from()` replaces the first (TinkerPop's `addFrom` setter
  does the same).
- **Exactly one label.** `from(["a", "b"])` (a list) is an `InvalidModulator` error, detected before
  the traversal runs. TinkerPop's `from()` takes one label (or a traversal, which Graphersal does
  not support for `path()`).
- **No value from `by()`.** A `by()` that produces nothing drops the traverser (as `path().by()`
  already did); TinkerPop does the same without `ProductiveByStrategy`.
- **Empty argument list.** `from([])`/`to([])` in a script is the no-argument form and is ignored,
  as it is for `addE()`.
