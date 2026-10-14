# Add Property Fold Rule

Rule name: `add_property_fold` (on by default, second in the pipeline).

## What it rewrites

`addV("Person").property("name", "Ann").property("age", 30)` is written as three steps, but the
graph should see **one** creation: the vertex with all its properties. The rule folds `addV()` /
`addE()` and the run of constant `property(key, value)` steps right behind it into a single
`add_vertex` / `add_edge` call. `.profile()` lists the folded keys:
`add_v("Person", properties: ["name", "age"])`.

## Why

- **Required properties.** A schema (`open` or `closed`) that lists a property in `required`
  rejects an element created without it (unless its schema has a `default`, which the creation stores). Without the fold, the empty `addV("Person")` would be validated
  before any `property()` ran and could never succeed.
- **No half-created elements.** The storage validates the complete element before inserting it,
  so a failing property (wrong type, undeclared key in a Closed schema) leaves no element behind.
- It is also faster: one storage call instead of one per property.

### What folds

The run starts directly after the add step and continues over:

- `property(key, constant)` steps with a plain string key;
- `as()` labels and the `from()` / `to()` modulators of `addE()` (they do not break the run).

The same key repeated keeps the **last** value, exactly as the unfused sequence does.

## When it does not fire (where the run stops)

The run stops at the first step it cannot reproduce exactly; that step and everything after it
stay unfused and run as separate steps on the created element:

- `property(key, __.something)`: the child traversal receives the *new element* as input, so it
  cannot be evaluated before the element exists;
- `property(Cardinality, key, value)`;
- `property(jpath("a.b"), value)`: a path write below a stored property. A required top-level
  property cannot be supplied through a path write at creation; give the whole value instead,
  e.g. `property("a", #{b: 1})`;
- `property(map)`, `property_json(..)` and any other step.

An unfolded `property()` that fails after the element was created fails the traversal, and
the failing traversal is rolled back as a whole, the created element included (every traversal is
one unit, see [Transactions](../users_guide/transactions.md)). The fold still matters: it lets a
required property be supplied at creation, where a separate `property()` would come too late.

## Example

```
$ graphersal -e 'g.add_v("Person").property("name", "Ann").property("age", 30).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
add_v("Person", properties: ["name", "age"])                    1       0       1   28.291µs    19.91
                                                      TOTAL:             execute:  142.125µs    19.91
=====================================================================================================
Optimizer rules applied: add_property_fold

$ graphersal -e 'g.with("optimizer.disabled", ["add_property_fold"]).add_v("Person").property("name", "Ann").property("age", 30).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
add_v("Person")                                                 1       0       1  262.250µs    18.84
property("name", "Ann")                                         1       1       1  115.834µs     8.32
property("age", 30)                                             1       1       1      292ns     0.02
                                                      TOTAL:             execute:    1.392ms    27.19
=====================================================================================================
```

An edge, with `as()` and `from()` inside the run:

```
$ graphersal -e 'g.v("1").as("a").v("2").add_e("knows").from("a").property("weight", 0.5).property("since", 2020).profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v("1")                                                          1       0       1    4.167µs     2.73
as("a") [path: labels(a)]                                       1       1       1    2.250µs     1.47
v("2") [path: labels(a)]                                        1       1       1    1.916µs     1.25
add_e("knows", properties: ["weight", "since"]).from("a")       1       1       1   21.125µs    13.83
                                                      TOTAL:             execute:  152.750µs    19.29
=====================================================================================================
Optimizer rules applied: add_property_fold
```

A traversal-valued `property()` ends the run; the constant `property("age", 30)` behind it is not
folded either:

```
$ graphersal -e 'g.add_v("Person").property("name", "Ann").property("nick", __.values("name")).property("age", 30).profile()'
Traversal Metrics
Step                                                                   Call      In     Out       Time    % Dur
===============================================================================================================
add_v("Person", properties: ["name"])                                     1       0       1   15.417µs     3.50
property("nick", __.values("name"))                                       1       1       1  168.833µs    38.37
  \> values("name")                                                       1       1       1    1.875µs     0.43
property("age", 30)                                                       1       1       1      292ns     0.07
                                                                TOTAL:             execute:  440.000µs    41.94
===============================================================================================================
Optimizer rules applied: add_property_fold
```

## Results

Without a schema, or with a schema the element satisfies at every intermediate step, the created
element and the results are the same. `g.add_v("Person").property("name", "Ann").property("name", "Bo").values("name")`
returns `"Bo"` either way.

This is the one rule whose absence can change a result: under a schema with required properties,
the empty creation fails without it. With this schema (both fields are required):

```
$ S='g.set_schema(`{"mode": "open", "vertices": {"Person": {"schema": {"type": "object",
     "properties": {"name": {"type": "string"}, "age": {"type": "integer"}},
     "required": ["name", "age"]}}}}`)'

$ graphersal --graph empty -e "$S" -e 'g.addV("Person").property("name", "Ann").property("age", 30).values("name").toList()'
"Ann"

$ graphersal --graph empty -e "$S" -e 'g.with("optimizer.disabled", ["add_property_fold"]).addV("Person").property("name", "Ann").property("age", 30).values("name").toList()'
Error: Step #0 'add_v("Person")' execution failed
  at #0: add_v("Person").property("name", "Ann").property("age", 30).values("name")
         ^^^^^^^^^^^^^^^
Caused by: Missing required property 'name' on vertex 'Person'
Help: Provide a value for 'name' or remove it from the label's "required" in the schema, or give it a "default" (or "defaultFunction") in the schema: it is stored when an element is created without the property (mode open or closed), but never filled into existing elements, and never by an import or a property removal.
      A required property must be supplied when the element is created: add_v("L").property("name", value) works because the optimizer rule add_property_fold folds the directly following constant property() steps into add_v()/add_e(). If a property() after it is not folded (the rule is disabled with g.with("optimizer.disabled", ["add_property_fold"]), the value is a traversal, a Cardinality or a jpath key, or another step sits in between), the element is created empty and this error is raised; move a constant property("name", value) directly behind add_v()/add_e() and keep add_property_fold enabled.
```

The failed query leaves the graph unchanged. See [Schemas](../users_guide/schema_enforcement.md).

## Interaction with other rules

Runs before every other rule except [Where Unnest](where_unnest.md). The folded `add_v`/`add_e` is
a mutating step, so [Lazy Barrier](lazy_barrier.md)'s merge points in the same plan pass
traversers through without merging.
