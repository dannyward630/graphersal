//! [`VertexValue`] and [`EdgeValue`]: a vertex or an edge returned as a value (a terminal's
//! result), with its id, label(s), endpoints and properties kept apart.
//!
//! The shape follows TinkerPop's detached elements and GraphSON 3: a property named `id` or
//! `label` is a property like any other and never takes the place of the element's id or label.

use compact_str::CompactString;

use crate::value::{PropertyMap, compact_string_heap_bytes, estimated_element_heap_bytes};

/// A vertex returned as a value: its id, its label set and a snapshot of its properties.
///
/// The first label is the primary one ([`VertexValue::label`], what `label()` returns); a vertex
/// without a label has an empty set. The properties are a snapshot taken when the vertex was
/// materialized; property values may be nested objects and lists.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VertexValue {
    id: CompactString,
    labels: Vec<CompactString>,
    properties: PropertyMap,
}

impl VertexValue {
    /// A vertex value of `id`, `labels` (primary label first) and `properties`.
    pub fn new(
        id: impl Into<CompactString>,
        labels: Vec<CompactString>,
        properties: PropertyMap,
    ) -> Self {
        Self {
            id: id.into(),
            labels,
            properties,
        }
    }

    /// The vertex's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The primary label, `None` for a vertex without a label.
    pub fn label(&self) -> Option<&str> {
        self.labels.first().map(CompactString::as_str)
    }

    /// The full label set, primary label first (a Graphersal multi-label vertex).
    pub fn labels(&self) -> &[CompactString] {
        &self.labels
    }

    /// The properties (never the id or the label).
    pub fn properties(&self) -> &PropertyMap {
        &self.properties
    }

    /// The properties, mutable.
    pub fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }

    /// The properties, consuming the value.
    pub fn into_properties(self) -> PropertyMap {
        self.properties
    }

    /// This vertex as an endpoint reference (id and primary label).
    pub fn to_ref(&self) -> VertexRef {
        VertexRef::new(self.id.clone(), self.label().map(CompactString::new))
    }

    /// The heap bytes this value owns outside its own struct: the id and labels when they
    /// spilled to the heap, the label `Vec` and the property map (an estimate, like
    /// [`crate::ElementProperty::estimated_heap_bytes`]).
    pub(crate) fn heap_bytes(&self) -> u64 {
        compact_string_heap_bytes(&self.id)
            + (self.labels.capacity() * std::mem::size_of::<CompactString>()) as u64
            + self
                .labels
                .iter()
                .map(compact_string_heap_bytes)
                .sum::<u64>()
            + estimated_element_heap_bytes(&self.properties)
    }
}

/// An edge endpoint: the vertex's id and primary label, as GraphSON's `outV`/`outVLabel` and
/// `inV`/`inVLabel` carry it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct VertexRef {
    id: CompactString,
    label: Option<CompactString>,
}

impl VertexRef {
    /// An endpoint reference of `id` and `label` (`None` for a vertex without a label).
    pub fn new(id: impl Into<CompactString>, label: Option<CompactString>) -> Self {
        Self {
            id: id.into(),
            label,
        }
    }

    /// The vertex's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The vertex's primary label, `None` for a vertex without a label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The heap bytes of the id and the label when they spilled to the heap.
    fn heap_bytes(&self) -> u64 {
        compact_string_heap_bytes(&self.id)
            + self.label.as_ref().map_or(0, compact_string_heap_bytes)
    }
}

/// An edge returned as a value: its id, label, both endpoints and a snapshot of its properties.
///
/// Edge property values may be nested objects and lists, exactly like vertex property values.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgeValue {
    id: CompactString,
    label: Option<CompactString>,
    out_v: VertexRef,
    in_v: VertexRef,
    properties: PropertyMap,
}

impl EdgeValue {
    /// An edge value of `id`, `label`, its outgoing (`out_v`) and incoming (`in_v`) vertex and
    /// `properties`.
    pub fn new(
        id: impl Into<CompactString>,
        label: Option<CompactString>,
        out_v: VertexRef,
        in_v: VertexRef,
        properties: PropertyMap,
    ) -> Self {
        Self {
            id: id.into(),
            label,
            out_v,
            in_v,
            properties,
        }
    }

    /// The edge's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The edge's label, `None` for an edge without a label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The outgoing vertex (the edge's tail, `Direction.OUT`).
    pub fn out_v(&self) -> &VertexRef {
        &self.out_v
    }

    /// The incoming vertex (the edge's head, `Direction.IN`).
    pub fn in_v(&self) -> &VertexRef {
        &self.in_v
    }

    /// The properties (never the id, the label or the endpoints).
    pub fn properties(&self) -> &PropertyMap {
        &self.properties
    }

    /// The properties, mutable.
    pub fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }

    /// The properties, consuming the value.
    pub fn into_properties(self) -> PropertyMap {
        self.properties
    }

    /// The heap bytes this value owns outside its own struct: the id, label and endpoint
    /// strings when they spilled to the heap, and the property map (an estimate, like
    /// [`crate::ElementProperty::estimated_heap_bytes`]).
    pub(crate) fn heap_bytes(&self) -> u64 {
        compact_string_heap_bytes(&self.id)
            + self.label.as_ref().map_or(0, compact_string_heap_bytes)
            + self.out_v.heap_bytes()
            + self.in_v.heap_bytes()
            + estimated_element_heap_bytes(&self.properties)
    }
}
