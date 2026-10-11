//! Shared helpers of the schema test modules (not a test module itself).
#![allow(dead_code)]

use graphersal::prelude::*;
use graphersal::schema::PropertySchema;

/// Loads a schema from its JSON text, panicking with the diagnostic when it is invalid.
pub fn schema(json: &str) -> GraphSchema {
    GraphSchema::from_json(json).unwrap_or_else(|err| panic!("{}", format_diagnostic(&err)))
}

/// [`schema`] with its mode replaced.
pub fn schema_in(json: &str, mode: SchemaMode) -> GraphSchema {
    schema(json).with_mode(mode)
}

/// A label's root node from `(key, schema, required)` entries.
pub fn object(fields: Vec<(&str, PropertySchema, bool)>) -> PropertySchema {
    let mut node = PropertySchema::object();
    for (key, field, required) in fields {
        node = if required {
            node.with_required_property(key, field)
        } else {
            node.with_property(key, field)
        };
    }
    node
}

/// A required key (it must be present; `null` only when the node admits it).
pub fn req(key: &str, node: PropertySchema) -> (&str, PropertySchema, bool) {
    (key, node, true)
}

/// An optional key that also admits `null` (the old "nullable" field).
pub fn opt(key: &str, node: PropertySchema) -> (&str, PropertySchema, bool) {
    (key, node.nullable(), false)
}

/// Sets `schema` on a fresh graph.
pub fn graph_with(schema: GraphSchema) -> TraversalGraph {
    let mut graph = TraversalGraph::new();
    graph
        .set_schema(schema)
        .expect("an empty graph satisfies any schema");
    graph
}

/// A property map from `(key, value)` pairs.
pub fn props(entries: &[(&str, ElementProperty)]) -> PropertyMap {
    let mut out = PropertyMap::default();
    for (k, v) in entries {
        out.insert(*k, v.clone());
    }
    out
}
