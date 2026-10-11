# Group Count Pushdown Rule

Rule name: `group_count_pushdown` (on by default, seventh in the pipeline).

## What it rewrites

A "count per label" directly behind a leading `v()` or `e()` is fused into the start step, which
then computes the counts from the graph's label index instead of producing one traverser per element
and grouping them:

```
v().group_count().by(T.label)                 →   v().group_count().by(T.label)    (one fused step)
v().group().by(T.label).by(__.count())        →   v().group_count().by(T.label)
e().group_count().by(T.label)                 →   e().group_count().by(T.label)
v().has_label("person").group_count().by(T.label)  →  v(labels: ["person"]).group_count().by(T.label)
```

The accepted spellings are `group_count().by(T.label)`, and `group().by(T.label)` followed by
`by(Count)` or by a child traversal that is a plain `count()`. The fused step keeps any ids and
labels that [Source Filter Pushdown](source_filter_pushdown.md) already folded into the start step.

## Why

The counts per label are already known to the label index. Without the rule, the scan creates a
traverser for every element and the grouping step reads each element's label and updates a map.

## When it does not fire

- The grouping step does not directly follow the start step: anything between them that was not
  folded into `v()`/`e()` (`out()`, `has("age", ...)`, a second `has_label`) stops the rule.
- The traversal does not start with `v()`/`e()`.
- The key is not `T.label`: `group_count().by("age")`, `group_count().by(__.label())`.
- `group().by(T.label)` without a second `by()`, or with a value projection other than a count.
- The start step was already fused with a `count()` by [Count Pushdown](count_pushdown.md) (which
  runs later, so this only matters for hand-built plans).

## Example

```
$ graphersal -e 'g.v().group_count().by(T.label).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v().group_count().by(T.label)                                   1       0       1  166.792µs    86.66
                                                      TOTAL:             execute:  192.459µs    86.66
=====================================================================================================
Optimizer rules applied: group_count_pushdown
```

Disabled, the scan produces six traversers that the grouping step consumes:

```
$ graphersal -e 'g.with("optimizer.disabled", ["group_count_pushdown"]).v().group_count().by(T.label).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0       6    1.833µs     1.36
group_count().by(T.label)                                       1       6       1  106.208µs    78.70
                                                      TOTAL:             execute:  134.958µs    80.06
=====================================================================================================
```

Combined with a folded label filter:

```
$ graphersal -e 'g.v().has_label("person").group_count().by(T.label).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"]).group_count().by(T.label)                 1       0       1    5.458µs    16.21
                                                      TOTAL:             execute:   33.667µs    16.21
=====================================================================================================
Optimizer rules applied: source_filter_pushdown, group_count_pushdown
```

The rule saves the scan, not the result: the cost of building the result map stays. The `large`
graph gives every vertex its own label (`V_0_0`, `V_0_1`, ...), so the result has 111110 entries,
building it dominates both plans, and the fused plan only saves the few milliseconds of the scan:

```
$ graphersal --graph large -e 'g.v().group_count().by(T.label).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v().group_count().by(T.label)                                   1       0       1   18.220ms    99.83
                                                      TOTAL:             execute:   18.251ms    99.83
=====================================================================================================
Optimizer rules applied: group_count_pushdown

$ graphersal --graph large -e 'g.with("optimizer.disabled", ["group_count_pushdown"]).v().group_count().by(T.label).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v()                                                             1       0  111110    1.536ms     9.23
group_count().by(T.label)                                       1  111110       1   15.087ms    90.59
                                                      TOTAL:             execute:   16.653ms    99.82
=====================================================================================================
```

## Results

Unchanged. Every vertex is counted once, under its primary label, the value `label()` returns; a
vertex without a label is not counted, with or without the rule. A
[multi-label vertex](../users_guide/multi_label_vertices.md) counts once, under its first label:

```
$ graphersal --graph empty -e 'g.add_v(["Person", "Admin"]).next(); g.add_v("Person").next(); g.add_v().next(); g.v().group_count().by(T.label).to_list()'
#{"Person": 2}
```

Without a label filter, the fused step reads the per-primary-label counts from the storage's
[statistics](../users_guide/memory_and_profiling.md#graph-statistics) (`GraphStorage::statistics`),
which the graph keeps up to date on every mutation: no scan at all, one map entry per label. A
storage that keeps no statistics is scanned instead, with the same result. With a folded label
filter the fused step fetches the matching vertices once each and groups them by primary label.

In a child traversal that receives several traversers at once (`union(__.V().groupCount().by(T.label))`),
every incoming traverser stands for its own pass over the graph, so the counts are multiplied by
the number of incoming traversers, as in the unfused plan.

## Interaction with other rules

Runs after [Source Filter Pushdown](source_filter_pushdown.md) (whose folded filters it keeps) and
before [Count Pushdown](count_pushdown.md). A `count()` after the fused step is not folded again;
it counts the single map the fused step produces.
