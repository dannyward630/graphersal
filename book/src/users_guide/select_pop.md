# Repeated Labels: select with Pop

A step label can occur several times on one path: `as("a")` inside `repeat()` sets it once per iteration, and a query can reuse a name at several steps. `select("a")` reads the **newest** occurrence. The `Pop` token chooses which occurrence `select` reads, as in TinkerPop:

| Pop | Reads |
|---|---|
| `Pop.first` | the oldest occurrence |
| `Pop.last` | the newest occurrence (what `select("a")` does) |
| `Pop.all` | a list of every occurrence, in path order; always a list, also for one occurrence |
| `Pop.mixed` | the value itself for one occurrence, a list in path order for several |

`Pop` goes first: `select(Pop.all, "a")`, `select(Pop.first, "a", "b")` (a map, one entry per label), `select(Pop.all, ["a", "b"])`.

```rhai,graph=modern
g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop.first, "a").by(__.unfold().id().fold())  // ["1"]  ["1"]
g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop.last,  "a").by(__.unfold().id().fold())  // ["5"]  ["3"]
g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop.all,   "a").by(__.unfold().values("name").fold())
// ["marko", "josh", "ripple"]   ["marko", "josh", "lop"]
```

`Pop.all` returns one list per traverser; the elements stay lazy handles until a terminal materializes them. In Rust:

```rust
use graphersal::prelude::*;

let graph = GraphSource::tinkerpop_modern();
let lock = graph.read();
let lists = lock
    .traversal()
    .v("1")
    .as_("a")
    .repeat(__::out(None).as_("a"))
    .times(2)
    .select_pop(Pop::All, "a")
    .to_list()
    .unwrap();
assert_eq!(lists.len(), 2);
```

`select_pop(pop, labels)` exists on the traversal source, on `AnonymousTraversal` and as `__::select_pop`; `select(labels)` is `Pop::Last`.

## Lookup order

For each key, `select` looks in this order, and `Pop` applies **only to the last source**:

1. the traverser's own value, when it is a map that contains the key (`valueMap().select(Pop.all, "name")` returns the map's list for `name`, not a wrapped list);
2. a side effect of that name (`aggregate("a")`, `store("a")`, ...);
3. a path label: here the pop chooses among the occurrences.

If the key is found nowhere, the traverser is filtered out. With several keys, the result is a map and one missing key filters the whole traverser. Several labels on one position (`as("a").as("a")`, or `as("a").has(..).as("a")`: a filter adds no position) are **one** occurrence, so `Pop.all` has one element for it.

`by()` applies to the selected value as a whole, never per element: after `select(Pop.all, "a")` it receives the list. `by("name")` on a list fails with `PropertyNotFound` (a list has no properties); use `by(__.unfold().values("name").fold())` as above. With several keys the existing `by()` ring applies (one `by()` per key in turn, the last one wins for a repeated key).

## Notations

The Rhai DSL accepts both the Gremlin (Java/Groovy) and the Rust spellings, so a Gremlin query can be pasted as it is:

| Spelling | Example |
|---|---|
| dot (Gremlin) | `Pop.first`, `Pop.last`, `Pop.all`, `Pop.mixed` |
| `::` (Rust style) | `Pop::first`, `Pop::all` ... |
| PascalCase aliases | `Pop.First`, `Pop::All` ... |
| bare constants (Groovy `import static Pop.*`) | `first`, `last`, `all`, `mixed` |
| Rust | `Pop::First`, `Pop::All` ..., `select_pop(Pop::All, "a")` |

Notes:

- The bare names are scope constants, like `keys` and `values` for `Column`. They coexist with steps and methods of the same name (`g.V().all(P.gt(0))`, `[1, 2].all(|x| x > 0)`): a constant and a method share no namespace. A script variable named `first`/`last`/`all`/`mixed` shadows the constant for the rest of the script (after `let all = 1`, `select(all, "a")` reads the labels `1` and `a`, which the undeclared-label diagnostic catches).
- `eval_with_params` reserves the class name `Pop` as a parameter name, but **not** the bare names (`first`, `last`, `all` are likely parameter names; a parameter named `first` wins over the constant).
- `Pop` must be the first argument. A lone `Pop`, a `Pop` in any other position and a non-string label after it are an `ArgumentMismatch` with a `help()` that shows the forms, never a silent `select("first", "a")`.
- `Pop.first == Pop::first` works (`==` and `!=` are registered for the token).
- Only `Column` (`keys`, `values`) and `Pop` have bare constants in the DSL. Every other token needs its class (`Scope.local`, `Order.desc`, `Direction.OUT`, `T.label`, `Operator.sum`): a bare `local`, `desc` or `OUT` is "Variable not found" in a script (the TinkerPop test harness rewrites them, a user query does not).
- `select(Pop, traversal)` and `select(traversal)` take a traversal as the key, see the next section.

## A traversal as the key

The key does not have to be a literal: `select(Pop.x, <traversal>)` and plain `select(<traversal>)` (TinkerPop's `TraversalSelectStep`; plain `select(traversal)` is `Pop.last`) compute it.

```rhai,graph=modern
g.V().as("a").out("knows").as("a").select(Pop.all, __.constant("a")).by(__.unfold().values("name").fold())
// ["marko", "josh"]   ["marko", "vadas"]
```

- The key traversal runs on the **incoming traverser with its path**, so it can read labels itself (`__.select("k")` where `k` holds the name of the label to read).
- Its **first result** is the key; the other results are ignored. No result filters the traverser. A result that is not a string (a vertex, a number) can only be a key of the incoming map, for example `group().by().by(..)` keyed by vertices: `g.V().as("a").group("m").by().by(__.bothE().count()).barrier().select("m").select(__.select("a"))`. A key that is not found filters the traverser (TinkerPop raises `KeyNotFoundException`, which `select` turns into an empty traverser).
- The key is then looked up exactly like a literal one: a key of the traverser's own map, then a side effect, then a path label read with the `Pop` (the pop applies to the last source only).
- One `by()` applies, the last one, to the whole selected value; a `by()` that yields nothing filters the traverser. A second `by()` is not an error (TinkerPop's ring has size 1 and the last modulator wins).
- Exactly one key traversal is allowed: `select(Pop.all, __.constant("a"), "b")` is an `ArgumentMismatch`, as TinkerPop has no such overload.
- The key is only known at run time, so the plan cannot name the label it needs and records the **full path** upstream (`[path: full]` in `.profile()`, step text `select(Pop.all, __.constant("a"))`). The plan-time undeclared-label diagnostic does not apply to a traversal key.

In Rust: `select_traversal(Pop::All, __::constant("a"))` on the traversal source, on `AnonymousTraversal` and as `__::select_traversal`; plain `select(traversal)` is `select_traversal(Pop::Last, ..)`.

Reading a property of every element of a selected list: `by("name")` is applied to the list itself and fails with `PropertyNotFound` (the `help()` of the error shows the recipe). Use a child traversal, `.by(__.unfold().values("name").fold())`.

## How the occurrences are found

Only `Pop.last` with literal labels keeps the cheap recording: the path analysis records just the labelled positions it needs. Every other pop needs *all* occurrences, so the plan records **full paths** upstream of the `select` (`.profile()` shows `[path: full]` on every step before it, and a select text such as `select(Pop.all, "a")`). Full paths also switch off bulk merging for those steps, so a `repeat()` that explodes into many equal paths is slower and uses more memory with `Pop.all` than with `select("a")`. A plan without a Pop `select` is not affected. Measured on the 110k-element `large` graph (`g.V().as("a").repeat(__.out().as("a")).times(3)....count()`, debug build): execute time 117 ms for `select("a")`, 164 ms for `select(Pop.all, "a")` (about 1.4x); these forms record full paths.

## Copy-paste cheat sheet

The DSL is Gremlin with Rhai syntax. A Gremlin Groovy line pastes as it is, except for the few places below; the `Pop` forms need no edit at all.

| Gremlin Groovy | Rhai DSL | Why |
|---|---|---|
| `select(Pop.all, 'a')`, `select(Pop.first, "a")` | the same (`Pop.all`, `Pop::all`, `Pop.All`, `Pop::All` or bare `all`) | `Pop` is a registered class; `Pop.all` and `Pop::all` are equal |
| `select(first, 'a')` (static import) | `select(first, "a")` | bare `first`/`last`/`all`/`mixed` are constants, but a script variable of that name shadows them |
| `'a'` single quotes | `"a"` | Rhai strings: use double quotes (single quotes are a char literal) |
| `count(local)`, `sum(local)` | `count(Scope.local)` | bare `local` exists only through the test-harness translator |
| `order().by(desc)`, `by('age', asc)` | `by(Order.desc)`, `by("age", Order.asc)` | same: bare `asc`/`desc`/`shuffle` are harness-only |
| `toE(OUT, 'x')`, `property(list, 'k', 1)` | `toE(Direction.OUT, "x")`, `property(Cardinality.list, "k", 1)` | bare token names are harness-only; `T.label`/`T.id` already carry the class |
| `by(label)`, `group().by(values)` | `by(T.label)`, `by(Column.values)` | `keys`/`values` also work bare for `Column`, `label` does not |
| `where(out())`, `union(out(), in())`, `not(has('x'))` | `where(__.out())`, `union(__.out(), __.in())`, `not(__.has("x"))` | an anonymous step needs the `__.` prefix, also for `V()` (`__.V()`) |
| `by(constant(1))` | `by(__.constant(1))`; never `by(1)` | a literal in `by()` is a property key (`by(1)` reads the property `1` and finds nothing), not a constant |
| `1L`, `1.5d` | `1`, `1.5` | Rhai integers are 64-bit, floats 64-bit |
| `[1, 2]`, `[a: 1]` | `[1, 2]`, `#{a: 1}` | Rhai object maps use `#{ }` |
| `g.V().as('a')`, `hasLabel` | the same; `as_`/`has_label` also work | every step is registered in both snake_case and camelCase |
| Rust: `select_pop(Pop::All, "a")`, `select_traversal(Pop::All, __::constant("a"))` | `select(Pop::All, "a")`, `select(Pop::All, __.constant("a"))` | the DSL has one `select`; the token tells which form it is |

A snake_case query with a Gremlin token and a camelCase query with a Rust token both work: the receiver spelling and the token spelling are independent (the matrix in `tests/all/pop_notation_tests.rs` runs every combination).

## Deviations from TinkerPop

All also listed in [TinkerPop Deviations](../for_developers/tinkerpop_deviations.md).

- **Undeclared label.** `g.V().select(Pop.first, "a")` where nothing in the query declares `a` raises the same plan-time diagnostic as `g.V().select("a")`; TinkerPop returns nothing. Six `map/Select.feature` scenarios (`g_V_selectXfirst_aX`, `g_V_selectXfirst_a_bX`, `g_V_selectXlast_aX`, `g_V_selectXlast_a_bX`, `g_V_selectXall_aX`, `g_V_selectXall_a_bX`) are therefore *deliberate incompatibilities* and leave the compatibility scope. A map-producing step upstream (`valueMap()`, `inject()`) disables the check, so `g.V().valueMap().select(Pop.first, "a")` filters to nothing like TinkerPop (those six scenarios pass).
- **Full path recording** (above, and for every traversal key) is a cost difference, not a result difference.
- **`Pop.mixed` with a list-valued oldest occurrence.** TinkerPop's `Path.get(label)` appends the later occurrences to the stored list *in place* when the oldest occurrence is itself a list. Graphersal gives the same result (the list's elements followed by the later occurrences) on a copy and never changes the stored value.

### The list-valued occurrence

A label whose single occurrence is itself a list (`g.V("1").values("name").fold().as("a")`) is returned whole by `first`, `last` and `mixed`, and `all` wraps it: `[[ "marko" ]]`. This is TinkerPop's behaviour for the path every live traversal uses (no deviation): `javap -c` of gremlin-core 3.7.2 shows that `ImmutablePath`, the only path the traverser classes of that version create, overrides `Path.get(Pop, String)` and returns an occurrence as it is (`first`/`last` the occurrence, `all` a list of the occurrences, so a list-valued occurrence stays nested). `MutablePath` also returns `first`/`last` as they are; only its `all` unwraps a single list-valued occurrence, and no traverser creates one. The *interface default* of `Path.get(Pop, String)` does unwrap a list (`first`/`last` return its first/last element, `all` returns it unwrapped), but only paths that do not override it use it (`DetachedPath`, `ReferencePath`, `EmptyPath`: results already detached from a traversal); `select` never reads those. No scenario of the vendored 3.8.2 feature files depends on it; the 3.8.2 sources were not available, so 3.7.2 is the source for this rule.
