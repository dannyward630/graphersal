# Current Limitations

What Graphersal 0.1.0 does not do, in one place. Each item links to the page with the details;
the deliberate differences from Apache TinkerPop are listed one by one in
[TinkerPop Deviations](../for_developers/tinkerpop_deviations.md), and the steps and scenarios that
are not implemented yet in [TinkerPop Compliance](../for_developers/tinkerpop_compliance.md).

## Data model

- **No date or time values.** They come with a date type in 0.2.0. A GraphSON import skips dates
  and reports them ([GraphSON](graphson.md)); a schema refuses `"format": "date-time"`
  ([Schemas](schema_enforcement.md#what-is-an-error-and-why)).
- **One value per property key.** Only `Cardinality.single` is written; store several values as
  one array ([Upserts](upserts.md)). There are no meta-properties: a GraphSON import folds them
  into the `_meta` property.
- **Ids are strings.** A numeric id becomes its text, and ids order as strings (`"10"` before
  `"9"`; see [Id Arguments](id_arguments.md) and [Predicates](predicates.md)).
- **Edges have one label**; vertices have a label set ([Multi-Label Vertices](multi_label_vertices.md)).
- **Property indexes are declared, top-level and per label.** A `has()` on a property reads an
  index only when one is declared with `g.create_index(..)` for the start step's label and the key
  ([Property Indexes](property_indexes.md)); nested values (jpath keys), `has()` without
  `has_label()` and `order()` scan.
- **The graph lives in memory.** A [Store](../persistence/store.md) makes it durable, but the
  whole graph is loaded into memory when the store is opened.

## Query language

- **The DSL is Rhai, not Groovy.** Queries read like Gremlin in either spelling (`hasLabel` or
  `has_label`), but maps are written `#{..}`, child traversals need `__.` (`where(__.out())`) and
  tokens need their class (`Order.desc`, `Scope.local`). A Gremlin text query is not parsed as it
  is.
- **No `withStrategies()` / `withoutStrategies()`.** Execution options are set with
  `g.with(key, value)` ([Execution Options Reference](execution_options_reference.md)).
- **No `g.tx()`.** Every traversal is one unit of work; a host can run a whole script as one unit
  ([Transactions](transactions.md)).
- **No Cypher or GQL** front end yet; the multi-label storage is the groundwork for one.
- **Saved queries only read** and are called at the start of a traversal (`g.query(..)`), not on
  incoming traversers ([Saved Queries](saved_queries.md)).

## Schema

- No map type (`additionalProperties` with a schema), no `oneOf`/`allOf`/`not`, no tuples, and no
  `format` other than `uuid` ([Schemas](schema_enforcement.md#what-is-an-error-and-why)).
- `default` is an annotation only: it is never written or substituted
  ([`default` is not applied](schema_enforcement.md#default-is-not-applied)).

## Security and serving

- Read security per element or label does not exist yet; an authorizer decides per request kind
  and label as the plan states it ([Permissions](permissions.md)).
- The dev server (`graphersal --server`, also MCP and AI Chat) is a local development tool:
  unstable, bound to the loopback, without authentication
  ([Dev Server and MCP](../persistence/dev_server.md)).

## Other

- Compression rules cover strings at a path of names; values inside arrays are not compressed
  ([Compressed Properties](compressed_properties.md)).
- [Generated test data](generating_test_data.md) is English only.
