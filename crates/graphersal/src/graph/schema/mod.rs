//! Graph schemas: an envelope of our own (`$schema`, `meta`, `mode`, `vertices`, `edges` with
//! their `connections`) around standard JSON Schema 2020-12 (a strict subset) for each label's
//! properties. The format is described by [`META_SCHEMA_JSON`] and the book page "Schema
//! Enforcement".

/// Applying a schema to existing data: the aggregated violation report.
pub mod apply;
pub(crate) mod coerce;
pub(crate) mod defaults;
/// Schema diff and its backward-compatibility classification.
pub mod diff;
/// Per-label schemas, connections, the mode and [`ElementKind`].
pub mod elements;
/// Schema inference from stored data.
pub mod inference;
/// The schema meta-graph.
pub mod meta_graph;
/// The meta-schema describing the schema format itself.
pub mod meta_schema;
pub(crate) mod nested;
/// One JSON Schema node: [`PropertySchema`] and its parts.
pub mod node;
/// Loading a schema (JSON text or a value) and writing it back.
pub mod parse;
/// JSON Merge Patch (RFC 7396) of a schema.
pub mod patch;
/// [`ValueType`] and related type tokens.
pub mod types;
/// Schema validation on write.
pub mod validation;
/// Schema visualization (table, markdown, tree, mermaid, plantuml, jsonschema, json).
pub mod visualize;

pub use apply::*;
pub use diff::*;
pub use elements::*;
pub use inference::*;
pub use meta_schema::*;
pub use node::*;
pub use patch::*;
pub use types::*;
pub(crate) use validation::*;
pub use visualize::*;

use indexmap::IndexMap;

use crate::value::ElementProperty;

/// The `$schema` URL of the schema format this version of the engine reads and writes.
pub const SCHEMA_FORMAT_URL: &str = "https://graphersal.dev/schema/v1";

/// A graph's declared schema: the envelope (mode, free-form `meta`, labels, connections) plus
/// one JSON Schema per label for its properties.
///
/// Load one with [`GraphSchema::from_json`] / [`GraphSchema::from_value`] (strict: unknown
/// keywords are errors), write it back with [`GraphSchema::to_json`] /
/// [`GraphSchema::to_value`]; or build one with [`GraphSchema::new`] and the `with_*` methods.
///
/// ```json
/// {
///   "$schema": "https://graphersal.dev/schema/v1",
///   "mode": "closed",
///   "vertices": {
///     "person": {"schema": {"type": "object",
///       "properties": {"name": {"type": "string", "minLength": 1},
///                      "age": {"type": ["integer", "null"], "minimum": 0}},
///       "required": ["name"]}}
///   },
///   "edges": {
///     "knows": {"connections": [{"from": "person", "to": "person"}],
///               "schema": {"type": "object", "properties": {"weight": {"type": "number"}}}}
///   }
/// }
/// ```
///
/// A schema with mode `none`, no `meta` and no labels is the *empty* schema: storing it means
/// storing no schema at all ([`GraphSchema::is_empty`]).
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct GraphSchema {
    /// Enforcement mode: `None` (no validation/coercion), `Open`, or `Closed`.
    pub mode: SchemaMode,
    /// Free-form data the library stores, round-trips and never interprets (a schema version,
    /// a status, server data). Always a JSON object when set. Changing it is a schema change.
    pub meta: Option<ElementProperty>,
    /// Per-vertex-label schemas, keyed by label.
    pub vertices: IndexMap<String, VertexSchema>,
    /// Per-edge-label schemas, keyed by label.
    pub edges: IndexMap<String, EdgeSchema>,
}

impl GraphSchema {
    /// A new, empty schema with no enforcement (`SchemaMode::None`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets this schema's enforcement mode. With an inferred schema it freezes the structure a
    /// graph has: `g.set_schema(g.infer_schema()?.with_mode(SchemaMode::Closed))` (see
    /// [`GraphTraversalSource::set_schema`](crate::GraphTraversalSource::set_schema)).
    pub fn with_mode(mut self, mode: SchemaMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets `meta` to a free-form object (a non-object value is wrapped as `{"value": v}`).
    pub fn with_meta(mut self, meta: ElementProperty) -> Self {
        self.meta = Some(match meta {
            object @ ElementProperty::Object(_) => object,
            other => {
                let mut map = crate::value::PropertyMap::default();
                map.insert("value", other);
                ElementProperty::Object(Box::new(map))
            }
        });
        self
    }

    /// Declares a vertex label whose properties follow `schema` (an `object` node).
    pub fn with_vertex(mut self, label: impl Into<String>, schema: PropertySchema) -> Self {
        self.vertices
            .insert(label.into(), VertexSchema::new(schema));
        self
    }

    /// Declares an edge label.
    pub fn with_edge(mut self, label: impl Into<String>, edge: EdgeSchema) -> Self {
        self.edges.insert(label.into(), edge);
        self
    }

    /// Whether this is the empty schema (mode `none`, no `meta`, no labels): storing it stores
    /// no schema.
    pub fn is_empty(&self) -> bool {
        self.mode == SchemaMode::None
            && self.meta.is_none()
            && self.vertices.is_empty()
            && self.edges.is_empty()
    }

    /// The root (`object`) node of a label of `kind`, if the label is declared.
    pub(crate) fn label_root(
        &self,
        kind: crate::graph::GraphNodeType,
        label: &str,
    ) -> Option<&PropertySchema> {
        match kind {
            crate::graph::GraphNodeType::Vertex => self.vertices.get(label).map(|v| &v.schema),
            crate::graph::GraphNodeType::Edge => self.edges.get(label).map(|e| &e.schema),
        }
    }

    /// The schema declared for the top-level property `key` on an element of `kind` with these
    /// `labels`, whatever the schema's [`SchemaMode`]: the declaration of the first label, in
    /// order, that declares `key` (the "first declaring label" rule of write coercion).
    pub(crate) fn declared_property(
        &self,
        kind: crate::graph::GraphNodeType,
        labels: &[compact_str::CompactString],
        key: &str,
    ) -> Option<&PropertySchema> {
        labels
            .iter()
            .find_map(|label| self.label_root(kind, label.as_str())?.properties.get(key))
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::GraphSchema;
    use crate::value::ElementProperty;

    /// Serializes as the canonical schema format (the JSON of [`GraphSchema::to_value`]).
    impl Serialize for GraphSchema {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.to_value().serialize(serializer)
        }
    }

    /// Deserializes the schema format, as strictly as [`GraphSchema::from_value`].
    impl<'de> Deserialize<'de> for GraphSchema {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let value = ElementProperty::deserialize(deserializer)?;
            GraphSchema::from_value(&value).map_err(serde::de::Error::custom)
        }
    }
}
