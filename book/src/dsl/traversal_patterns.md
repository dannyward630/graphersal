# Traversal Patterns

The chapters of this section describe the steps that shape a walk through the graph: loops,
labels and paths, branches, side effects and the sack. Several of them are short pages about the
exact rules in a corner of TinkerPop's semantics and the few places where Graphersal differs.

| Page | What it covers |
|---|---|
| [Recursive Traversals](../users_guide/recursive_traversals.md) | `repeat()` with `times()`, `until()`, `emit()`, `loops()`; breadth and depth first |
| [Bulk and Barriers](../users_guide/bulk_and_barriers.md) | how equal traversers are merged, `barrier()`, why it stays invisible |
| [where() Start and End Labels](../users_guide/where_scope.md) | `where()` with labels and predicates, `filter()` versus `where()` |
| [Repeated Labels: select with Pop](../users_guide/select_pop.md) | `select(Pop.first/last/all/mixed, ...)`, the Groovy copy-paste table |
| [Path Windows](../users_guide/path_windows.md) | `from()`/`to()` on `path()`, `simple_path()`, `cyclic_path()` |
| [union() as a Start Step](../users_guide/union_start.md) | `g.union(...)` without `v()` |
| [Branch Children and Global State](../users_guide/branch_batches.md) | how `union()`, `choose()` and `branch()` feed their children |
| [Option Keys of choose() and branch()](../users_guide/option_semantics.md) | `option()` keys and unmatched traversers |
| [The by() Modulator: Single-by Steps](../users_guide/by_modulator_single.md) | steps that take one `by()` |
| [Set Side Effects](../users_guide/set_side_effects.md), [tree() as a Side Effect](../users_guide/tree_side_effect.md) | `aggregate()`, `store()`, `cap()`, `tree("key")` |
| [Sack and Operators](../users_guide/sack_and_operators.md) | `with_sack()`, `sack()`, `Operator`, reducers |
