//! `Vertex` and `Edge`: a vertex or an edge a query returned, as Python sees it (gremlin-python's
//! `Vertex`/`Edge`): id, label and properties kept apart.

use graphersal::prelude::{CompactString, EdgeValue, ElementProperty, VertexRef, VertexValue};
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// A vertex a query returned (detached: id, labels and a snapshot of its properties).
///
/// `v.id`, `v.label` (the primary label, `None` without one) and `v.labels` are the vertex's own;
/// `v.properties` (a dict) and `v["name"]` are its properties only, so a property named `label`
/// is `v["label"]` and never hides `v.label`. Equal to another vertex of the same id; prints as
/// `v[1]`. An edge's endpoints (`e.out_v`, `e.in_v`) are vertices without properties. A vertex
/// can be passed back as a parameter (`g.V(v)`, `inject(v)`).
#[pyclass(module = "graphersal", frozen)]
pub struct Vertex {
    id: String,
    labels: Vec<String>,
    properties: Py<PyDict>,
}

#[pymethods]
impl Vertex {
    /// The vertex's id.
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    /// The primary label, `None` for a vertex without a label.
    #[getter]
    fn label(&self) -> Option<&str> {
        self.labels.first().map(String::as_str)
    }

    /// The full label set, primary label first.
    #[getter]
    fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }

    /// The properties (never the id or the label), as a dict.
    #[getter]
    fn properties(&self, py: Python<'_>) -> Py<PyDict> {
        self.properties.clone_ref(py)
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.properties.bind(py).get_item(key)? {
            Some(value) => Ok(value.unbind()),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .cast::<Vertex>()
            .is_ok_and(|other| other.get().id == self.id)
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ("v", &self.id).hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!("v[{}]", self.id)
    }
}

/// An edge a query returned (detached: id, label, endpoints and a snapshot of its properties).
///
/// `e.id` and `e.label` are the edge's own, `e.out_v` / `e.in_v` its endpoints (vertices without
/// properties), `e.properties` and `e["weight"]` its properties only. Equal to another edge of the
/// same id; prints as `e[0][1-knows->2]`.
#[pyclass(module = "graphersal", frozen)]
pub struct Edge {
    id: String,
    label: Option<String>,
    out_v: Py<Vertex>,
    in_v: Py<Vertex>,
    properties: Py<PyDict>,
}

#[pymethods]
impl Edge {
    /// The edge's id.
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    /// The edge's label, `None` for an edge without a label.
    #[getter]
    fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The outgoing vertex (id and label only).
    #[getter]
    fn out_v(&self, py: Python<'_>) -> Py<Vertex> {
        self.out_v.clone_ref(py)
    }

    /// The incoming vertex (id and label only).
    #[getter]
    fn in_v(&self, py: Python<'_>) -> Py<Vertex> {
        self.in_v.clone_ref(py)
    }

    /// The properties (never the id, the label or the endpoints), as a dict.
    #[getter]
    fn properties(&self, py: Python<'_>) -> Py<PyDict> {
        self.properties.clone_ref(py)
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.properties.bind(py).get_item(key)? {
            Some(value) => Ok(value.unbind()),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .cast::<Edge>()
            .is_ok_and(|other| other.get().id == self.id)
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ("e", &self.id).hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "e[{}][{}-{}->{}]",
            self.id,
            self.out_v.get().id,
            self.label.as_deref().unwrap_or(""),
            self.in_v.bind(py).get().id
        )
    }
}

/// A Python `Vertex` of a vertex value, its properties converted by `property`.
pub(crate) fn vertex_to_python(
    py: Python<'_>,
    vertex: &VertexValue,
    property: impl Fn(Python<'_>, &ElementProperty) -> PyResult<Py<PyAny>>,
) -> PyResult<Py<Vertex>> {
    let properties = PyDict::new(py);
    for (key, value) in vertex.properties().iter() {
        properties.set_item(key.as_str(), property(py, value)?)?;
    }
    Py::new(
        py,
        Vertex {
            id: vertex.id().to_string(),
            labels: vertex.labels().iter().map(|l| l.to_string()).collect(),
            properties: properties.unbind(),
        },
    )
}

/// A Python `Vertex` without properties for an edge endpoint.
fn vertex_ref_to_python(py: Python<'_>, vertex: &VertexRef) -> PyResult<Py<Vertex>> {
    Py::new(
        py,
        Vertex {
            id: vertex.id().to_string(),
            labels: vertex.label().map(str::to_string).into_iter().collect(),
            properties: PyDict::new(py).unbind(),
        },
    )
}

/// A Python `Edge` of an edge value, its properties converted by `property`.
pub(crate) fn edge_to_python(
    py: Python<'_>,
    edge: &EdgeValue,
    property: impl Fn(Python<'_>, &ElementProperty) -> PyResult<Py<PyAny>>,
) -> PyResult<Py<Edge>> {
    let properties = PyDict::new(py);
    for (key, value) in edge.properties().iter() {
        properties.set_item(key.as_str(), property(py, value)?)?;
    }
    Py::new(
        py,
        Edge {
            id: edge.id().to_string(),
            label: edge.label().map(str::to_string),
            out_v: vertex_ref_to_python(py, edge.out_v())?,
            in_v: vertex_ref_to_python(py, edge.in_v())?,
            properties: properties.unbind(),
        },
    )
}

/// A `Vertex` or an `Edge` passed as a parameter: the element value of its id and label, without
/// properties (a query only needs its identity: `g.V(v)`, `inject(v)`). `None` for any other
/// Python value.
pub(crate) fn python_element_to_property(value: &Bound<'_, PyAny>) -> Option<ElementProperty> {
    if let Ok(vertex) = value.cast::<Vertex>() {
        let vertex = vertex.get();
        return Some(ElementProperty::Vertex(Box::new(VertexValue::new(
            vertex.id.as_str(),
            vertex.labels.iter().map(CompactString::new).collect(),
            Default::default(),
        ))));
    }
    if let Ok(edge) = value.cast::<Edge>() {
        let edge = edge.get();
        let endpoint = |vertex: &Py<Vertex>| {
            let vertex = vertex.get();
            VertexRef::new(
                vertex.id.as_str(),
                vertex.labels.first().map(CompactString::new),
            )
        };
        return Some(ElementProperty::Edge(Box::new(EdgeValue::new(
            edge.id.as_str(),
            edge.label.as_deref().map(CompactString::new),
            endpoint(&edge.out_v),
            endpoint(&edge.in_v),
            Default::default(),
        ))));
    }
    None
}
