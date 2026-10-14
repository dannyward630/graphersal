# Property Indexes

`g.v().has_label("person").has("title", "CEO")` reads every `person` vertex and compares its
`title`. On a large graph that dominates the query time even when only a few vertices match. A
**property index** orders the elements of one label by the value of one property, so the same
query reads only the matching vertices.

```rhai,graph=modern,script
// Declare: the `person` vertices, ordered by `age`.
g.create_index(#{label: "person", key: "age"});

// Served: equality, P.within and ranges on `age` after has_label("person").
g.v().has_label("person").has("age", 29).values("name").to_list();
g.v().has_label("person").has("age", P.between(30, 40)).values("name").to_list();

g.indexes()                                  // the indexes and what they hold
```

Indexes are **declared, never automatic**: nothing is indexed until you ask for it, and an index
costs memory and a little time on every write of its key (below).

## Declare, list, drop

| DSL | Rust (`GraphTraversalSource`) | Python (`Graph`) |
|---|---|---|
| `g.create_index(#{label, key, element?, name?})` / `g.createIndex(..)` | `create_index(name, PropertyIndexDefinition::new(kind, label, key))` | `graph.create_index(label, key, element="vertex", name=None)` |
| `g.indexes()` | `list_indexes()` | `graph.indexes()` |
| `g.drop_index(name)` / `g.dropIndex(name)` | `drop_index(name)` | `graph.drop_index(name)` |

- `element` is `"vertex"` (the default) or `"edge"`; `label` is one vertex or edge label (it need
  not exist yet); `key` is one **top-level** property name (a plain name, not a jpath: nested
  values are not indexed).
- `name` defaults to `<label>_<key>` (`<label>_<key>_edge` for an edge index), with every
  character that is not a letter, a digit or `_` replaced by `_`. It is an identifier like a
  saved query's name.
- One index per `(element, label, key)`: a second one under another name fails with a
  *Conflicting property index* error that names the first. Declaring the same name again replaces
  the index.
- `g.indexes()` lists each index with `served` (whether the storage serves it, see
  [below](#other-storages)) and its current `entries` (indexed elements), `distinct_values` and
  `bytes` (an estimate of its memory).

```rhai,graph=modern,script
g.create_index(#{element: "edge", label: "knows", key: "weight"});
g.e().has_label("knows").has("weight", P.gt(0.6)).count().next()
```

## What is served

A `v()`/`e()` start step reads an index when

- it is the **start** of the traversal (or of a child traversal), the optimizer folded a
  `has_label(..)` into it, and **every** folded label has an index on the key: the index of
  `has_label("person")`, or of `has_label("person", "employee")` when both labels have one;
- and a `has()` on the indexed key follows directly (only `has()`, `has_label()`, `has_id()`,
  `has_key()`, `has_not()` in between):

| Condition | Served |
|---|---|
| `has(key, value)` | equality |
| `has(key, P.eq(v))`, `has(key, P.within(a, b, ..))` | one slice per value |
| `has(key, P.gt/gte/lt/lte(v))` | one range |
| `has(key, P.between(a, b))`, `P.inside(a, b)` | one range |
| `has(key, P.outside(a, b))` | two ranges |
| `P.neq`, `P.without`, text predicates, `P.typeOf`, `and`/`or`/`not` composites | not served (scan) |
| a value that is a list or a map (`has("tags", ["a"])`) | not served (scan) |
| `v().has("title", "CEO")` without a label, a jpath key | not served (scan) |

With several served conditions the optimizer picks the **most selective** index from the index
statistics (an equality reads about `entries / distinct_values` elements of each label, a range
about half the entries).

**Results are the same as without the index**, always: the index returns a superset of the
matching elements (numbers are ranged by their value as `float64`, so `29` and `29.0` are both
read), and the start step and the `has()` step still check every condition on each candidate. The
comparison is exactly that of `has()`: `has("age", 29)` matches an `int64` 29 only,
`has("age", P.eq(29.0))` and the ranges compare numbers across `int64` and `float64`, a string never
equals a uuid, `null` equals only `null`. Dates and times are index keys with the same rules
([Comparing and Sorting](../dates/comparing.md)): each of `date`, `localdatetime`, `datetime`,
`time` and `duration` is its own kind (a `date` never equals a `datetime` or a string, and a range
reads only values of its bounds' type), a `datetime` is keyed by its instant (`12:00Z` and
`14:00+02:00` are one key), and a `duration` is keyed by all its fields (`P1M` and `P30D` are two
keys) and ranged by its length, a month as 30 days, ties broken by the fields: equality,
`P.within` and every range on durations are served. Only the **order** of the elements may differ (the index
reads them by value): use `order()` when the order matters.

A `P.eq("a")` with a string inside a child traversal is not served: there a string first names a
path label (`where(P.eq("a"))`), which an index cannot know.

## See it in `.profile()`

The start step names the index it reads, and the rule `property_index` is listed:

```
$ graphersal -e 'g.create_index(#{label: "person", key: "age"})' \
             -e 'g.v().has_label("person").has("age", P.gt(30)).values("name").profile()'
Traversal Metrics
Step                                                         Call      In     Out       Time    % Dur
=====================================================================================================
v(labels: ["person"]) [index: vertex person.age]                1       0       2   55.666µs     7.53
has("age", P.gt(30))                                            1       2       2   42.625µs     5.77
values("name")                                                  1       2       2    1.834µs     0.25
                                                      TOTAL:             execute:  738.958µs    13.55
=====================================================================================================
Optimizer rules applied: source_filter_pushdown, property_index
```

An equality folded into the start step shows as
`v(labels: ["person"]).has("title", "CEO") [index: vertex person.title]`. To compare with the scan,
turn the rule off for one query: `g.with("optimizer.disabled", ["property_index"])` (see
[Property Index Rule](../optimizer/property_index.md)).

## Kept exact

An index is maintained on every write: adding and dropping elements (a vertex drop takes its edges
out of the edge indexes), writing and removing the key (also through a jpath write that changes
it), adding and removing a vertex label (a vertex with several labels is in the index of each of
them), changing an edge label. A rolled-back unit restores the index with the data; `replay`,
`apply_change_set` and a store's recovery rebuild it exactly. A string kept
[compressed](compressed_properties.md) is indexed by its text (the index holds the plain text, so
indexing a long compressed field costs the memory compression saved).

## Persistence

An index is a catalog definition (kind `property_index`, like [saved queries](saved_queries.md)
and compression rules), so its **declaration** travels with the graph: the journal (WAL), snapshots,
backups, forks, rollbacks and the catalog file. Its **entries are never stored**: they are rebuilt
from the data whenever a graph is loaded, by a store open, a read-only view (also of a past state),
a backup open, a recovery, a snapshot load. A read-only view is therefore always consistent with
the data it shows. GraphML and GraphSON files carry no catalog, so no index.

## Memory

Each indexed element costs one entry: its value (strings longer than 24 bytes on the heap) plus
about 50 bytes of ordered-set bookkeeping. `g.memory_usage()` reports the indexes as
`property_index_bytes` (part of `total_bytes`, so the [memory budget](resource_limits.md) sees
them), `g.indexes()` per index. Building an index reads every element of its label once.

## Other storages

Indexes are a capability of the storage (`StorageCapabilities::property_indexes`); the built-in
graph has it. On a [custom storage](../for_developers/custom_storage.md) without it,
`create_index` fails with *unsupported: property indexes*; an index definition can still be stored
in its catalog (a catalog file loaded into it, say) and is listed with `served: false`, but no
query reads it. A storage that serves indexes implements `GraphStorage::property_indexes` and the
lookups `vertices_by_property_index`/`edges_by_property_index`.

## In the playground, the dev server and MCP

- **Playground** (WebAssembly and over the dev server): **Catalog ▾ ▸ Property indexes** lists the
  indexes with their entries, distinct values, elements per value (what an equality reads) and
  memory, creates one from a form (element, label, key, optional name; the default name is shown
  as you type) and drops one after a confirmation. The **Statistics** tab of the Profile panel
  shows *Property indexes* with their memory when the graph has some. Indexes travel with the
  catalog file (**Catalog ▾ ▸ Save catalog**).
- **Dev server**: `GET /api/indexes`, `POST /api/indexes/create` `{label, key, element?, name?}`,
  `POST /api/indexes/drop` `{name}` (see [the dev server](../persistence/dev_server.md)); a
  create or drop is a commit recorded in the change feed, so open browsers reload. The start
  block on stderr names the index memory.
- **MCP**: the tools `list_indexes` (also with `--mcp-read-only`), `create_index` and
  `drop_index` (not offered read-only, where `query` refuses `g.create_index` too).

## Limits

- Not indexed: nested values (jpath keys), lists and maps as values, labels other than the one
  declared (an index on `person` does not serve `v().has("title", ..)` without `has_label`).
- No unique indexes (constraints) in this version.
- `order().by(key)` does not read the index.
