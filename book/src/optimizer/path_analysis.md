# Path Requirement Analysis

See the [Execution Options Reference](../users_guide/execution_options_reference.md) for the full
table of every `g.with()` key, including `path.analysis` below.

Steps such as `path()`, `select("a")` and `simple_path()` read the *path* of a traverser: the
objects it went through, and the step labels `as()` attached to them. Recording a path costs memory
and time, so Graphersal records only what a later step actually reads. The path requirement
analysis decides what that is. It follows the idea of TinkerPop's `PathRetractionStrategy`.

The analysis runs once for each compiled traversal, **after** all optimizer rules, because rules
fuse and remove steps. A query without a path consumer records no path at all.

## Path modes

Each step reports what it reads from the path of its incoming traversers:

| Step | Reads |
|---|---|
| `path()`, `simple_path()`, `cyclic_path()` | the full path |
| `select("a", ...)`, `math("a + b")` (every variable except `_`), `add_e().from("a").to("b")`, `where(P.eq("a"))`, `is(P.eq("a"))`, `has(key, P.eq("a"))` | those labels |
| `where()`, `not()`, `and()`, `or()`, `union()`, `coalesce()`, `optional()`, `repeat()`, `until()`, `emit()`, `by(__...)`, `property(key, __...)` | what their child traversals read |
| `as("a")` | nothing: it *produces* the label `a` |

One backward walk over each traversal level then gives every step a **mode**: what it must write
into the paths of its outputs so that every later step finds what it reads.

- `full`: the step records each output object as a new path position.
- `labels(a, ...)`: only an `as()` with one of these labels records its position, together with
  its labels. Every other step leaves the path untouched.
- `none`: the step drops the path of its outputs.

A string in `P.eq("marko")` is treated as a label only when some `as("marko")` exists in the
query, so ordinary value comparisons never turn tracking on.

## Retraction

Walking backwards, a label stops being needed once the walk passes the `as()` that produces it.
In `g.V().as("a").out().as("b").select("a")`, only `a` is stored: `as("b")` records nothing,
because no later step reads `b`. Steps before the last consumer track the path; steps after it
drop it. `full` is never retracted, because a full path covers every position from the start.

Barriers (`fold()`, `group()`, `count()`, ...) start new paths, so nothing upstream of them is
needed for the steps that follow. A pure map right before a fused `count()` is never executed,
so `g.V().out().path().count()` records nothing.

## Nested traversals

A child traversal is analysed with its own backward walk. What its first step still needs is
reported as the requirement of the step that holds it. For example,
`g.V("1").out().where(__.in().simple_path())` makes `v("1")` and `out()` record full paths.

When a child's output continues the parent path (`union()`, `coalesce()`, `optional()`), the
parent passes its own downstream needs into the child. The child then keeps writing what the
parent's later steps read.

## Loops

The body of a `repeat()` is **loop-carried**: its output feeds the steps after the loop, the
`until()`/`emit()` conditions, and its own input on the next iteration. It is therefore analysed
with the downstream need

```text
need after repeat() ∪ entry(body) ∪ entry(until) ∪ entry(emit)
```

and the walk is repeated until that need stops growing. This settles within two passes: sets only
grow over the finite set of labels in the query, and `full` absorbs everything. The `repeat()`
step reports `entry(body) ∪ entry(until) ∪ entry(emit)` upward, on top of what the steps after it
read (a loop can end before its first iteration).

For example, `g.V("1").repeat(__.out().simple_path()).times(3).values("name")` makes `v("1")` and
every step of the body record full paths, while `values("name")` records nothing:

```text
v("1") [path: full]
repeat(__.out().simple_path()).times(3)
  \> out() [path: full]
  \> simple_path() [path: full]
values("name")
```

## Observing the analysis

`.profile()` appends the mode to each step that records something:

```text
v("1") [path: full]
out() [path: full]
path()
```

## Kill switch

`g.with("path.analysis", false)` turns the analysis off: every step records its full path. Use it
to diagnose a suspected analysis bug. With the analysis on or off, a query must return the same
results.

## Storage

Paths live in a per-execution arena. A traverser holds a 4-byte handle to the last position of its
path, and positions link to their parent. Siblings produced by one fan-out share their prefix, so
extending a path is a single push and cloning a traverser copies no path data.
