# List Functions

The list functions work on the **list a traverser holds** and turn it into one new value per
traverser (they are map steps, 1 to 1):

| Step | Result | Page |
|---|---|---|
| `combine(list)` | the incoming list followed by `list`, duplicates kept | [combine](combine.md) |
| `conjoin(delimiter)` | the elements joined into one string | [conjoin](conjoin.md) |
| `difference(list)` | the incoming elements that are not in `list` (a set) | [difference](difference.md) |
| `disjunct(list)` | the elements in exactly one of the two lists (a set) | [disjunct](disjunct.md) |
| `intersect(list)` | the incoming elements that are also in `list` (a set) | [intersect](intersect.md) |
| `merge(list)` / `merge(map)` | the union of two lists (a set), or two maps merged | [merge](merge.md) |
| `product(list)` | every `[a, b]` pair of the two lists | [product](product.md) |

They are TinkerPop 3.7+ steps and follow the TinkerPop 3.8.2 feature files (`map/Combine.feature`
and the others: every in-scope scenario passes).

## The incoming list

A traverser holds a list after `fold()`, `path()`, a list constant (`constant([..])`,
`inject([..])`) or a list-valued property (`values("tags")`). A path counts as the list of its
objects. Anything else, `null` included, is an error that names the step and the fix:

```rhai
graphersal> g.V().values("name").combine(["x"])
Error: Step #2 'combine(["x"])' execution failed
  at #2: v().values("name").combine(["x"])
                            ^^^^^^^^^^^^^^
Caused by: The combine() step takes a list as its incoming value, but got string
Help: combine() works on the list a traverser holds (a fold(), a path(), a list-valued property). Collect the stream into a list with fold() first, for example g.V().values("name").fold().combine(["x"]).
```

A fused `count()` after the step does not skip it: `g.V().values("name").combine(["x"]).count()`
raises the same error.

## The argument

The second operand is either

- a **list constant**: `combine(["dave", "kelvin"])` (Rust: `combine(vec!["dave", "kelvin"])`), or
- a **child traversal** whose **first result** is the list: `combine(__.V().values("name").fold())`
  (Rust: `combine_traversal(__::v(None).values("name").fold())`). It runs on the incoming traverser,
  with its path, so `__.select("a")` reads a label of the traverser. End it with `fold()` to turn a
  stream into a list.

`null`, a single value (`combine(2)`) and a traversal that yields nothing, `null` or a non-list
are errors. `merge()` of a map takes a map instead (see [merge](merge.md)).

## Element equality

`difference`, `disjunct`, `intersect` and `merge` have set semantics: every element appears at most
once in the result, in the order of its first occurrence (incoming list first). Elements compare
the way `dedup()` compares traversers: vertices and edges by identity, everything else by value
(`1` and `1.0` are different values, as in TinkerPop). `null` is an element like any other. The
kept elements stay what they were, so a folded vertex list keeps its vertices:

```rhai
graphersal> g.V("1").out("knows").fold().difference(__.V("4").fold()).unfold().values("name")
vadas
```

## In the Rust API

Every list function has a constant form and a `_traversal` form on `GraphTraversalSource`,
`AnonymousTraversal` and `__`: `combine` / `combine_traversal`, `difference` /
`difference_traversal`, and so on; `conjoin(delimiter)` has one form. In the DSL both spellings are
the same name (`combine`, `conjoin`, ...), which takes a list, a map or a traversal.
