# Query Optimizer

Before a traversal runs, Graphersal's rule-based optimizer rewrites its steps into a cheaper
physical plan: it folds filters into the start step, fuses counts, inserts merge points and so on.
`.profile()` always shows the plan **after** optimization, with fused steps under their real names
(for example `v(labels: ["person"]).count()`).

See the [Execution Options Reference](../users_guide/execution_options_reference.md) for the full
table of every `g.with()` key, including `optimizer.disabled`/`optimizer.enabled` below.

## Rules, order and default state

The rules run in this order, once per traversal level. Each has a stable name: the string
`.profile()` reports and the string `g.with("optimizer.disabled", [...])` /
`g.with("optimizer.enabled", [...])` accept.

| # | Rule | Name | Default | Rewrites |
|---|---|---|---|---|
| 1 | [Where Unnest](where_unnest.md) | `where_unnest` | on | `where(__.has...)` of local filters → the filters inline |
| 2 | [Add Property Fold](add_property_fold.md) | `add_property_fold` | on | `add_v(..).property(k, v)...` → one creation with all properties |
| 3 | [Filter Reorder](filter_reorder.md) | `filter_reorder` | on | adjacent filters sorted cheapest first |
| 4 | [Has ID Pushdown](has_id_pushdown.md) | `has_id_pushdown` | on | `v().has_id(..)` → `v(ids)` |
| 5 | [Source Filter Pushdown](source_filter_pushdown.md) | `source_filter_pushdown` | on | `v().has_label(..)`/`has_id(..)`/`has(k, v)` → `v(labels, ids).has(k, v)` (checked while scanning) |
| 6 | [Property Index](property_index.md) | `property_index` | on | `v(labels).has(k, ..)` reads a declared [property index](../users_guide/property_indexes.md) (`[index: ..]`) |
| 7 | [Barrier Pushdown](barrier_pushdown.md) | `barrier_pushdown` | on | `out_e().has_label(..)` → `out_e(labels)` |
| 8 | [Group Count Pushdown](group_count_pushdown.md) | `group_count_pushdown` | on | `v().group_count().by(T.label)` → one fused step |
| 9 | [Count Pushdown](count_pushdown.md) | `count_pushdown` | on | `v().count()` → one fused step |
| 10 | [Lazy Barrier](lazy_barrier.md) | `lazy_barrier` | on | inserts `lazy_barrier()` between adjacent adjacency steps |

All 10 rules ship on by default; none is opt-in today. The order matters: the early rules
bring filters into the position where the later ones can fold them. For example
`g.v().where(__.has_label("person")).count()` becomes `v(labels: ["person"]).count()` through
`where_unnest`, `source_filter_pushdown` and `count_pushdown` together.

**Nested traversals** (the children of `where()`, `union()`, `repeat()`, `by(__...)`, ...) are
optimized first, bottom-up, with the same rules, before their parent level. The rules that fold
into a start step only fire on a level that begins with `v()`/`e()`, so they rarely apply inside a
child; `where_unnest`, `filter_reorder`, `barrier_pushdown` and `lazy_barrier` do.

**Modulators.** A `by()` belongs to the step in front of it and moves with it: no rule moves a
filter across a `by()`, `from()`/`to()` or `as()`. `filter_reorder`, `has_id_pushdown`,
`source_filter_pushdown` and `barrier_pushdown` stop at such a step; `lazy_barrier` and
`add_property_fold` look past modulators (and `add_property_fold` past `as()`) because these do not
change their stream. `group_count_pushdown` reads the `by()` modulators of the grouping step: only
`by(T.label)` (plus `by(Count)` for `group()`) can be fused.

## See which rules fired

`.profile()`/`.profile_with(...)` end with an "Optimizer rules applied: ..." line after the
timing table, naming every rule (top-level and nested, deduplicated, in pipeline order) that
actually changed the plan. The line is omitted when no rule fired.

```
$ graphersal -e 'g.v().has_label("person").count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"]).count()                                   1       0       4    4.042µs    10.58
                                                      TOTAL:             execute:   38.208µs    10.58
=====================================================================================================
Optimizer rules applied: source_filter_pushdown, count_pushdown
```

The same list is available as data: `optimizer_rules_applied` on the `TraversalMetrics` value
(a property in Rhai, `TraversalMetrics::optimizer_rules_applied()` in Rust).

```
$ graphersal -e 'g.v().has_label("person").count().profile().optimizer_rules_applied'
"source_filter_pushdown"
"count_pushdown"
```

## Toggle rules for one query

**Disable a rule**, e.g. to compare a plan with and without it, or to work around a rule producing
an unwanted plan:

```rhai,graph=modern
g.with("optimizer.disabled", ["filter_reorder"]).v().has_label("person").count().profile()
```

**Disable every rule at once** with the reserved name `"all"`, to see a query's genuinely
unoptimized plan (the "Optimizer rules applied" line then disappears):

```
$ graphersal -e 'g.with("optimizer.disabled", ["all"]).v().has_label("person").count().profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    3.041µs    10.21
has_label("person")                                             1       6       4    2.000µs     6.71
count()                                                         1       4       1       42ns     0.14
                                                      TOTAL:             execute:   29.791µs    17.06
=====================================================================================================
```

**Force-enable an off-by-default rule** with `optimizer.enabled` (it also accepts `"all"`). This
is a no-op today, since no rule defaults to off, but it is ready for the first rule that does.

A name that is neither a rule above nor `"all"`, or a rule named in both lists, fails with
`InvalidOption` when the traversal executes (not at `with()` time):

```
$ graphersal -e 'g.with("optimizer.disabled", ["no_such_rule"]).v().count().to_list()'
Error: Invalid value for option 'optimizer.disabled': unknown optimizer rule "no_such_rule"; valid names are: where_unnest, add_property_fold, filter_reorder, has_id_pushdown, source_filter_pushdown, property_index, barrier_pushdown, group_count_pushdown, count_pushdown, lazy_barrier, all
Help: Set 'optimizer.disabled'/'optimizer.enabled' to an array of optimizer rule names ...
```

## Do the rules change results?

A rule changes the *plan*, not the *result*, with these exceptions, each described on the rule's
page:

- [Add Property Fold](add_property_fold.md) decides whether `addV(..).property(..)` can satisfy a
  schema with required properties at all. Without it, the creation fails.
- The *order* of results is not part of the guarantee: [Source Filter
  Pushdown](source_filter_pushdown.md) reads elements label by label from the index, and
  [Property Index](property_index.md) reads them in index order, and
  [Lazy Barrier](lazy_barrier.md) can reorder duplicates. Use `order()` when the order matters.

## Path requirement analysis

After the rules, the [path requirement analysis](path_analysis.md) walks every level of the final
plan backwards and decides what each step must record into traverser paths (`none`, only some
labels, or `full`) so that later readers such as `path()`, `select("a")` or `simple_path()` find
what they need. `.profile()` shows the decision as `[path: ...]` on each step that records
something, and `g.with("path.analysis", false)` forces full recording everywhere for diagnosis. It
runs after the rules because they fuse and remove steps, and it is not a rule: it cannot be
disabled through `optimizer.disabled`.

## Optimizer cost

The optimizer's own cost is not broken out as a separate row in `.profile()`. On a real graph it
is negligible next to the actual traversal work; on graphs the size of `tinkerpop_modern` the whole
query runs in microseconds, a scale where debug-build measurement noise dominates any number this
crate could report.
