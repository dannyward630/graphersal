//! Vertices and edges as the script sees them (`ScriptVertex`, `ScriptEdge`, `ScriptVertexRef`).

use crate::script::*;
use crate::value::{EdgeValue, VertexRef, VertexValue};

/// A vertex a terminal (`next()`, `to_list()`) returned, as the script sees it: TinkerPop's
/// detached vertex. Registered with Rhai as `Vertex`.
///
/// `v.id`, `v.label` and `v.labels` are the vertex's own id, primary label and label set;
/// `v["name"]` (and `v.name` for a name that is no getter) and `v.properties` (a map) are its
/// properties only, so a property named `label` is `v["label"]` and never hides `v.label`. It
/// prints as `v[1]`, equals another vertex of the same id, and can be passed back into a query
/// (`g.V(v)`, `has_id(v)`, `inject(v)`). Public so hosts can downcast it, like [`ScriptUuid`].
#[derive(Debug, Clone)]
pub struct ScriptVertex(pub(crate) VertexValue);

impl ScriptVertex {
    /// The wrapped vertex value.
    pub fn as_vertex_value(&self) -> &VertexValue {
        &self.0
    }
}

/// An edge a terminal returned, as the script sees it: TinkerPop's detached edge. Registered with
/// Rhai as `Edge`.
///
/// `e.id` and `e.label` are the edge's own; `e.out_v` / `e.outV` and `e.in_v` / `e.inV` its
/// endpoints (a
/// [`ScriptVertexRef`]); `e["weight"]` and `e.properties` its properties only. It prints as
/// `e[0][1-knows->2]` and equals another edge of the same id.
#[derive(Debug, Clone)]
pub struct ScriptEdge(pub(crate) EdgeValue);

impl ScriptEdge {
    /// The wrapped edge value.
    pub fn as_edge_value(&self) -> &EdgeValue {
        &self.0
    }
}

/// An edge endpoint (`e.out_v`, `e.in_v`): the vertex's id and primary label, without properties
/// (TinkerPop's reference vertex). Registered with Rhai as `VertexRef`; prints as `v[1]`.
#[derive(Debug, Clone)]
pub struct ScriptVertexRef(pub(crate) VertexRef);

impl ScriptVertexRef {
    /// The wrapped endpoint reference.
    pub fn as_vertex_ref(&self) -> &VertexRef {
        &self.0
    }
}

/// An optional text as a script value: the string, or `()`.
fn text_or_unit(text: Option<&str>) -> Dynamic {
    text.map_or(Dynamic::UNIT, |text| Dynamic::from(text.to_string()))
}

/// A property map as a Rhai object map.
fn properties_to_map(properties: &crate::value::PropertyMap) -> rhai::Map {
    properties
        .iter()
        .map(|(key, value)| {
            (
                key.as_str().into(),
                element_property_to_dynamic(value.clone()),
            )
        })
        .collect()
}

/// One property as a script value, `()` when the element has none of that name.
fn property_of(properties: &crate::value::PropertyMap, key: &str) -> Dynamic {
    properties.get(key).map_or(Dynamic::UNIT, |value| {
        element_property_to_dynamic(value.clone())
    })
}

/// `v[1]`, as TinkerPop prints a vertex.
pub(crate) fn vertex_text(vertex: &VertexValue) -> String {
    format!("v[{}]", vertex.id())
}

/// `e[0][1-knows->2]`, as TinkerPop prints an edge.
pub(crate) fn edge_text(edge: &EdgeValue) -> String {
    format!(
        "e[{}][{}-{}->{}]",
        edge.id(),
        edge.out_v().id(),
        edge.label().unwrap_or(""),
        edge.in_v().id()
    )
}

/// Registers the three types, their getters, indexers, text forms and equality.
pub(crate) fn register_element_types(registry: &mut crate::script::register::Registry) {
    registry
        .register_type_with_name::<ScriptVertex>("Vertex")
        .register_get("id", |v: &mut ScriptVertex| v.0.id().to_string())
        .register_get("label", |v: &mut ScriptVertex| text_or_unit(v.0.label()))
        .register_get("labels", |v: &mut ScriptVertex| {
            v.0.labels()
                .iter()
                .map(|label| Dynamic::from(label.to_string()))
                .collect::<rhai::Array>()
        })
        .register_get("properties", |v: &mut ScriptVertex| {
            properties_to_map(v.0.properties())
        })
        .register_indexer_get(|v: &mut ScriptVertex, key: &str| property_of(v.0.properties(), key))
        .register_fn_as(&["to_string", "to_debug"], |v: &mut ScriptVertex| {
            vertex_text(&v.0)
        })
        .register_fn("==", |a: ScriptVertex, b: ScriptVertex| {
            a.0.id() == b.0.id()
        })
        .register_fn("!=", |a: ScriptVertex, b: ScriptVertex| {
            a.0.id() != b.0.id()
        });
    registry
        .register_type_with_name::<ScriptEdge>("Edge")
        .register_get("id", |e: &mut ScriptEdge| e.0.id().to_string())
        .register_get("label", |e: &mut ScriptEdge| text_or_unit(e.0.label()))
        .register_get("out_v", |e: &mut ScriptEdge| {
            ScriptVertexRef(e.0.out_v().clone())
        })
        .register_get("outV", |e: &mut ScriptEdge| {
            ScriptVertexRef(e.0.out_v().clone())
        })
        .register_get("in_v", |e: &mut ScriptEdge| {
            ScriptVertexRef(e.0.in_v().clone())
        })
        .register_get("inV", |e: &mut ScriptEdge| {
            ScriptVertexRef(e.0.in_v().clone())
        })
        .register_get("properties", |e: &mut ScriptEdge| {
            properties_to_map(e.0.properties())
        })
        .register_indexer_get(|e: &mut ScriptEdge, key: &str| property_of(e.0.properties(), key))
        .register_fn_as(&["to_string", "to_debug"], |e: &mut ScriptEdge| {
            edge_text(&e.0)
        })
        .register_fn("==", |a: ScriptEdge, b: ScriptEdge| a.0.id() == b.0.id())
        .register_fn("!=", |a: ScriptEdge, b: ScriptEdge| a.0.id() != b.0.id());
    registry
        .register_type_with_name::<ScriptVertexRef>("VertexRef")
        .register_get("id", |v: &mut ScriptVertexRef| v.0.id().to_string())
        .register_get("label", |v: &mut ScriptVertexRef| text_or_unit(v.0.label()))
        .register_fn_as(&["to_string", "to_debug"], |v: &mut ScriptVertexRef| {
            format!("v[{}]", v.0.id())
        })
        .register_fn("==", |a: ScriptVertexRef, b: ScriptVertexRef| {
            a.0.id() == b.0.id()
        })
        .register_fn("!=", |a: ScriptVertexRef, b: ScriptVertexRef| {
            a.0.id() != b.0.id()
        })
        .register_fn("==", |a: ScriptVertex, b: ScriptVertexRef| {
            a.0.id() == b.0.id()
        })
        .register_fn("==", |a: ScriptVertexRef, b: ScriptVertex| {
            a.0.id() == b.0.id()
        });
}
