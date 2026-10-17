use std::collections::HashSet;

use indexmap::IndexMap;

use crate::graph::TraversalGraph;
use crate::graph::schema::{EdgeSchema, GraphSchema, PropertySchema};
use crate::value::{CompactString, ElementProperty, PropertyMap};

/// The meta-graph properties of one label: its name, its schema (the canonical JSON Schema
/// node) and its required keys.
fn label_properties(label: &str, schema: &PropertySchema) -> PropertyMap {
    let mut props = PropertyMap::default();
    props.insert(
        CompactString::new("label_name"),
        ElementProperty::String(CompactString::new(label)),
    );
    if let Some(description) = schema.annotations().and_then(|a| a.description.as_deref()) {
        props.insert(
            CompactString::new("description"),
            ElementProperty::from(description),
        );
    }
    props.insert(CompactString::new("schema"), schema.to_value());
    props
}

impl GraphSchema {
    /// Builds a queryable `TraversalGraph` of the schema's structure.
    ///
    /// - Each vertex label is a vertex (`label: "schema_vertex"`, `id: <label>`) with the
    ///   properties `label_name`, `description` (when the label's schema has one) and `schema`
    ///   (its JSON Schema as a map, the canonical shape).
    /// - Each edge connection is an edge (`label: <edge label>`) between the vertices of its
    ///   `from` and `to` labels, with the same properties for the edge label. A connection to
    ///   `*` or to an undeclared label has no vertex to attach to and is left out.
    pub fn to_graph(&self) -> TraversalGraph {
        let mut tg = TraversalGraph::new();
        for (label, vertex) in &self.vertices {
            let _ = tg.insert_vertex(
                Some(CompactString::new(label)),
                vec![CompactString::new("schema_vertex")],
                label_properties(label, &vertex.schema),
            );
        }
        for (edge_label, edge) in &self.edges {
            for conn in &edge.connections {
                let _ = tg.add_edge_by_ids(
                    None::<CompactString>,
                    Some(CompactString::new(edge_label)),
                    &conn.from,
                    &conn.to,
                    label_properties(edge_label, &edge.schema),
                );
            }
        }
        tg
    }

    /// The schema restricted to one vertex label (see [`Self::subgraph_labels`]).
    pub fn subgraph(&self, label: &str) -> GraphSchema {
        self.subgraph_labels(&[label])
    }

    /// The schema restricted to a set of vertex labels: those vertex labels, and every edge
    /// label with the connections among them (an edge label without such a connection is left
    /// out). Mode and `meta` are kept.
    pub fn subgraph_labels<S: AsRef<str>>(&self, labels: &[S]) -> GraphSchema {
        let label_set: HashSet<&str> = labels.iter().map(|s| s.as_ref()).collect();
        let vertices = self
            .vertices
            .iter()
            .filter(|(label, _)| label_set.contains(label.as_str()))
            .map(|(label, vertex)| (label.clone(), vertex.clone()))
            .collect();
        let mut edges = IndexMap::new();
        for (edge_label, edge) in &self.edges {
            let connections: Vec<_> = edge
                .connections
                .iter()
                .filter(|c| {
                    label_set.contains(c.from.as_str()) && label_set.contains(c.to.as_str())
                })
                .cloned()
                .collect();
            if !connections.is_empty() {
                let mut kept = EdgeSchema::new(edge.schema.clone());
                kept.connections = connections;
                edges.insert(edge_label.clone(), kept);
            }
        }
        GraphSchema {
            mode: self.mode,
            meta: self.meta.clone(),
            vertices,
            edges,
        }
    }
}
