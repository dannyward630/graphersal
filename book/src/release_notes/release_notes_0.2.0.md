# Release 0.2.0

The second minor release of Graphersal. It brings date and time values, property indexes, schema
defaults that are applied when an element is created, and vectors with exact similarity search.
It changes the storage format and a few APIs: read [Breaking changes](#breaking-changes) and the
[upgrade guide](#upgrading-from-01x) before you move a store or a storage implementation to it.
What 0.2.0 does not do yet is on [Current Limitations](../users_guide/current_limitations.md).

## Highlights

- **Dates and times** ([#3](https://github.com/graphersal/graphersal/issues/3);
  [Dates and Times](../dates/index.md)): five value types modelled on PostgreSQL and the SQL
  standard, `datetime` (an instant with its UTC offset), `localdatetime`, `date`, `time` and
  `duration` (months, days and microseconds kept apart, like `interval`), with microsecond
  resolution over the years -9999 to 9999. Time zones come from the bundled IANA database
  ([Time Zones](../dates/time_zones.md)): `date_add` and `date_trunc` in a zone, `at_zone`,
  `as_local`, daylight-saving gaps and overlaps resolved as PostgreSQL does. Calendar-correct
  [arithmetic](../dates/arithmetic.md) (`date_add`, `date_diff`, `age`), truncation and parts,
  [conversions](../dates/conversions.md), [`now()` and `today()`](../dates/now_and_today.md) (one
  time per script or unit), comparison and sorting, and the schema formats `date-time`, `date`,
  `local-date-time`, `time` and `duration` with `formatMinimum`/`formatMaximum`
  ([Dates in a Schema](../dates/schema.md)). TinkerPop's `datetime()`, `asDate()`, `dateAdd()`,
  `dateDiff()` and `DT` behave as TinkerPop 3.8 specifies them
  ([TinkerPop Compatibility](../dates/tinkerpop.md)). Dates are values in Rust, Rhai, Python
  (`datetime` objects and `graphersal.Interval`), GraphSON (the `java.time` types), the
  persistence format and the playground's editors ([Storage and Exchange](../dates/storage.md)).
  A string that looks like a date stays a string: a date comes from a constructor, a conversion,
  a schema declaration or a typed value, never from guessing.
- **Property indexes** ([#4](https://github.com/graphersal/graphersal/issues/4);
  [Property Indexes](../users_guide/property_indexes.md)): `g.create_index(..)` declares an
  ordered index on a label's top-level property; the new optimizer rule
  [`property_index`](../optimizer/property_index.md) answers `has()` equality, `P.within` and
  range filters from it (`.profile()` shows `[index: ..]`), with the same results as a scan. The
  declaration is stored in the catalog and travels with the graph; the entries are rebuilt at every
  load. Every index has a type: `memory-ordered` is the one type of 0.2 (an index type a later
  version adds is kept and listed as not served, never misread), and an index on a key the schema
  declares as a vector, list or map is refused with a clear message
  ([Index types](../users_guide/property_indexes.md#index-types)). Python `Graph.create_index`,
  the playground's **Catalog ▸ Property indexes**, the dev server's `/api/indexes` and MCP tools.
- **Schema defaults on create** ([#5](https://github.com/graphersal/graphersal/issues/5);
  [Defaults on create](../users_guide/schema_enforcement.md#defaults-on-create)): a schema's
  `default` now has SQL `DEFAULT` semantics on a label's top-level properties, and the new keyword
  `defaultFunction` fills `uuidv4`, `uuidv7` (time-ordered, RFC 9562), `now` or `today`.
  `add_v`/`add_e`, `mergeV`/`mergeE` on create and the playground's create dialog fill every
  missing default before the element is checked, in mode `open` and `closed`; explicit values
  (`null` too) win, and existing data is never changed.
- **Vectors** ([Vectors](../users_guide/vectors.md)): a value type for embeddings with `f32`,
  `f16`, `i8` (scalar quantization) and `bit` components, up to 65 536 of them; `vector([..])`,
  `as_vector()`, `quantize(kind)`, `GType.VECTOR`, and the schema form
  `{"type": "array", "format": "vector", "element": .., "dimension": .., "metric": ..}`, which
  coerces a list of numbers on write and can store a property quantized from the start. Exact
  search with `similarity()`, `nearest()` and `vector_mean()` over cosine, dot product, L2 and
  Hamming, combined freely with graph steps (filtered search, GraphRAG, recommendations). Vectors
  are values in Python (`graphersal.Vector`, also from a `numpy` array), the playground, GraphSON,
  GraphML (through the schema) and the persistence format. Embeddings are computed outside the
  engine; [Embeddings and Vector Search](../getting_started/embeddings.md) walks through it, in the
  playground without a model and from Python with one, and the playground's **Examples** have an
  *Embeddings (vectors)* group.
- **Playground**: the schema editor's tables edit `defaultFunction` and a vector's element,
  dimension and metric; the Graph and Mermaid drawings always show the schema the tables show
  (also a draft); the Mermaid diagram parses whatever labels and property names hold; the
  Detailed drawing zooms by mouse wheel as fast as the Fast one; a click on a panel's title
  collapses it; the header shows the engine version and the build.
- **Properties named `id` or `label`** ([#1](https://github.com/graphersal/graphersal/issues/1);
  [Properties Named id or label](../users_guide/id_and_label_keys.md)) are read as properties,
  never as the element's id or label, and **vertices and edges returned by a terminal are values**
  with their id, label and properties apart
  ([#2](https://github.com/graphersal/graphersal/issues/2);
  [Vertices and Edges as Values](../users_guide/vertex_and_edge_values.md)).
- **Store upgrade**: `graphersal store upgrade` (Rust `Store::upgrade`, Python
  `Store.upgrade`) converts a store, backup or single-file store of 0.1.x to the 0.2 format with
  this binary alone, keeping the whole history ([Format Versions and
  Upgrade](../persistence/store_versions.md#upgrading-a-store-of-an-older-version)).
- **Files of a newer Graphersal are refused, never taken for damage**
  ([#6](https://github.com/graphersal/graphersal/issues/6)), as in 0.1.1; which older versions
  are upgraded, and how, is one table in the code, shown on
  [Format Versions and Upgrade](../persistence/store_versions.md#the-upgrade-table).

## TinkerPop conformance

The vendored Apache TinkerPop 3.8.2 suite: **1631 of 1675** in-scope scenarios pass (**97.4 %**,
0.1.x: 1599, 95.5 %), 39 are unsupported (0.1.x: 71) and **0** supported steps behave differently
from the specification. The date and time features (`DateTime`, `AsDate`, `DateAdd`, `DateDiff`)
pass completely; the remaining gaps are mainly `call()` and the set type
([TinkerPop Compliance](../for_developers/tinkerpop_compliance.md)).

## Breaking changes

0.2.0 is a minor release before 1.0 and may break the 0.1 API
([Releases and Versioning](../for_developers/release_plan.md)). Every break, and what to do:

### Stored data and files

- **Store format version 3, file format version 2.** 0.2.0 writes new snapshot, WAL and packed
  snapshot files of file format version 2 (the value tags of the date and time types and of
  vectors) and marks its stores with store format version 3. It reads every 0.1.x file as it is:
  a `.gsnap` of 0.1.x loads unchanged. A **store** of 0.1.x (store format version 2) is refused
  with `OlderFormat` until it is upgraded once with `graphersal store upgrade`
  ([below](#upgrading-from-01x)); nothing is changed until you do.
- **There is no way back.** An upgraded store, and every store or file 0.2.0 writes, is refused
  by 0.1.x. **0.1.1** refuses it cleanly as "written by a newer Graphersal" in every operation.
  **0.1.0 does not**: it refuses to open such a store, but `store info` calls it corrupt,
  `store verify` lists damage, and a newer snapshot or WAL file can make it open a store in
  maintenance mode and offer a repair that loses data. Replace 0.1.0 with 0.1.1 wherever a 0.2
  store or backup may be seen by an older binary.

### Behaviour

- **A schema's `default` is applied.** In 0.1.x `default` was an annotation only. Now a
  top-level `default` of a label is stored when an element is created without that property, in
  mode `open` and `closed`. Check the schemas you keep before you upgrade: remove a `default` that
  was only meant as documentation, or move it into `description`. `set_schema`, `patch_schema`
  and `validate_schema` now refuse a `default` that is not a valid value of its property (a stored
  schema with one still loads, but creating an element would then fail its check: run
  `g.validate_schema(g.get_schema())`). `diff_schema` reports `default_changed`.
- **Vertices and edges returned as values** ([#2](https://github.com/graphersal/graphersal/issues/2)).
  `to_list()`/`next()` return `VertexValue`/`EdgeValue` in Rust, `Vertex`/`Edge` in Rhai and
  `graphersal.Vertex`/`graphersal.Edge` in Python, instead of a flat map of id, label and
  properties. Read `v.id`, `v.label` and `v["name"]` (Python: `v.id`, `v.label`, `v["name"]`),
  `v.properties` for the map. The JSON form (playground, MCP, `to_json()`) is untyped GraphSON 3:
  a vertex `{id, label, properties}`, an edge `{id, label, inVLabel, outVLabel, inV, outV,
  properties}`; `_from`/`_to` are gone (use `outV`/`inV`). `g.inject(v)` of an element value
  emits the stored element and asks the authorizer for Read on Data.
- **`id` and `label` as property names** ([#1](https://github.com/graphersal/graphersal/issues/1)).
  `by("label")`/`by("id")` read a property, as in TinkerPop: use `by(T.label)`/`by(T.id)` for the
  element's own. `select("id")`/`select("label")` on an `elementMap()`/`valueMap(true)` result
  read a property only: use `select(T.id)`/`select(T.label)`. An edge's `elementMap()` has
  `Direction.OUT`/`Direction.IN` entries.
- **GraphSON converts dates instead of skipping them.** `g:Date`, `g:Timestamp`, `gx:Instant`,
  `gx:OffsetDateTime`, `gx:ZonedDateTime` (zone id dropped, reported), `gx:LocalDate`,
  `gx:LocalDateTime`, `gx:LocalTime`, `gx:Duration` and `gx:Period` become date and time values;
  0.1.x skipped them, so the same file now yields properties that were absent before. A date-time
  text without an offset is refused, never read as UTC. The export writes the `java.time` tags
  ([GraphSON](../users_guide/graphson.md)).

### API for storage implementers (EXPERIMENTAL)

- **`insert_*` is the storage primitive, `add_*` applies defaults**
  ([#5](https://github.com/graphersal/graphersal/issues/5)). The required `GraphStorage` methods
  are renamed: implement `insert_vertex`/`insert_edge` (formerly `add_vertex`/`add_edge`, same
  arguments), which store exactly what they are given. `add_vertex`/`add_edge` are now provided
  methods that fill the schema's create-time defaults and then call `insert_*`; do not override
  them. `add_vertex_with`/`add_edge_with` take a `CreateContext` (`graphersal::storage`: the time
  `now`/`today` store, from `CreateContext::for_storage`) and are the one override point per kind.
  Code that copies or replays data (imports, `apply_change_set`, recovery) calls `insert_*`.
  `TraversalGraph::add_vertex`/`add_edge` apply defaults too; `insert_vertex`/`insert_edge` are
  added. `graphersal-storage-tests` has the new item `add_applies_schema_defaults_insert_does_not`.
- New provided trait methods (defaults keep a storage compiling): `property_indexes()` and the
  index lookups, `unit_started_at()`; `StorageCapabilities::property_indexes`;
  `MemoryStats::property_index_bytes`.
- `ElementProperty` and `ValueType` have new variants (dates, vectors), and
  `ElementProperty::Vertex`/`Edge` carry `VertexValue`/`EdgeValue`. Downstream `match`es need a
  `_` arm, as the enums are `#[non_exhaustive]`.

## Upgrading from 0.1.x

1. **Replace any 0.1.0 binary with 0.1.1** on machines that may meet an upgraded store or backup
   (see [There is no way back](#stored-data-and-files)).
2. **Back up the store** with 0.1.x while it is closed: `graphersal store backup my_store/
   backup_0_1/` (or a copy of the closed directory or `.gstore` file).
3. **Upgrade it with 0.2.0**, into a new store (the old one is only read):
   `graphersal store upgrade my_store/ my_store_0_2/`, or in place:
   `graphersal store upgrade my_store/ --in-place`. The store is verified first; everything is
   carried, history, marks, attic and creation parameters included, and only `GRAPH` is rewritten.
   A backup directory stays a backup (`store upgrade old_backup/ new_backup/`), a ZIP backup is
   upgraded as it is restored (`store restore old_backup.zip new_store/`). In Python:
   `graphersal.Store.upgrade("my_store", "my_store_0_2")` or `in_place=True`. Details:
   [Format Versions and Upgrade](../persistence/store_versions.md#upgrading-a-store-of-an-older-version)
   and [`store upgrade`](../persistence/store_cli.md#upgrade).
4. **Review the stored schema's `default`s** (above): `graphersal --graph my_store_0_2/ -e
   'g.validate_schema(g.get_schema())'`.
5. **Update code that reads returned elements**: maps become `Vertex`/`Edge` values; `by("label")`
   becomes `by(T.label)` where the element's label is meant. In Python, `result["name"]` keeps
   working on a `graphersal.Vertex`, while `result["id"]`/`result["label"]` now read properties
   (a `KeyError` when there is none):
   use `result.id`/`result.label`.
6. **GraphSON files** with `java.time` values now import them as dates; a query that relied on
   their absence, or a schema that declared them as `string`, needs a look. Packed snapshots
   (`.gsnap`) of 0.1.x need no step.
7. **Storage implementers**: rename `add_vertex`/`add_edge` to `insert_vertex`/`insert_edge`, pin
   `graphersal = "=0.2.0"` and `graphersal-storage-tests = "=0.2.0"`, and run the conformance suite
   ([Custom Storages](../for_developers/custom_storage.md)).

## Dependencies

- `jiff` (time zone rules and the bundled IANA database; the only new dependency).
- `rustls` 0.23.46 and `ruzstd` at least 0.9.1 (RustSec advisories), as in 0.1.1.

## Compatibility

- Minimum supported Rust version: 1.88.
- Python 3.10+ (abi3 wheels).
- Versioning and what is stable in 0.2.x: [Releases and Versioning](../for_developers/release_plan.md).
