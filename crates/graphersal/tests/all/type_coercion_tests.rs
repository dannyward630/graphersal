//! Integration tests for coercion on write and strict logical validation (type system step 3).
//!
//! In `Open`/`Closed` mode a declared field coerces the incoming value (a canonical string to
//! `uuid`, an `int64` to `float64`) and the coerced value is then compared exactly against every
//! declaring label. Reads never coerce, and a vertex or an edge can never be stored as a property
//! value, in any mode.

use graphersal::prelude::PropertyMap;
use graphersal::prelude::*;
use graphersal::schema::{PropertySchema, ValueType};

const CANONICAL: &str = "550e8400-e29b-41d4-a716-446655440000";
const CANONICAL_BITS: u128 = 0x550e8400_e29b_41d4_a716_446655440000;

/// An optional, nullable key of the given type, so `Closed` mode does not require it on every
/// vertex.
fn optional(ty: &ValueType) -> PropertySchema {
    let node = match ty {
        ValueType::Uuid => PropertySchema::uuid(),
        ValueType::String => PropertySchema::string(),
        ValueType::Float64 => PropertySchema::number(),
        ValueType::Int64 => PropertySchema::integer(),
        other => panic!("no node for {other}"),
    };
    node.nullable()
}

/// A schema whose vertex labels declare the given `(label, [(key, type)])` fields.
fn schema_with(mode: SchemaMode, labels: &[(&str, &[(&str, ValueType)])]) -> GraphSchema {
    let mut schema = GraphSchema::new().with_mode(mode);
    for (label, fields) in labels {
        let mut root = PropertySchema::object();
        for (key, ty) in fields.iter() {
            root = root.with_property(*key, optional(ty));
        }
        schema = schema.with_vertex(*label, root);
    }
    schema
}

/// The `Open` schema of the acceptance scenario: `person` declares `uid: uuid`, `name: string`
/// and `w: float64`.
fn person_schema(mode: SchemaMode) -> GraphSchema {
    schema_with(
        mode,
        &[(
            "person",
            &[
                ("uid", ValueType::Uuid),
                ("name", ValueType::String),
                ("w", ValueType::Float64),
            ],
        )],
    )
}

/// The modern graph with [`person_schema`] set in `Open` mode.
fn modern_with_person_schema() -> Graph {
    let graph = GraphSource::tinkerpop_modern();
    graph
        .write()
        .set_schema(person_schema(SchemaMode::Open))
        .unwrap();
    graph
}

/// Adds a `person` with one property through the traversal API.
fn add_person(
    graph: &Graph,
    key: &str,
    value: impl Into<ElementProperty>,
) -> Result<(), Box<TraverserError>> {
    let mut writable = graph.write();
    writable
        .traversal_mut()
        .add_v("person")
        .property(key, value.into())
        .next()
        .map(|_| ())
}

/// The schema error at the root of a failed write.
fn schema_error(err: &TraverserError) -> SchemaError {
    match err.root_cause() {
        TraverserError::Graph(GraphError::Schema(err)) | TraverserError::Schema(err) => err.clone(),
        other => panic!("expected a schema error, got {other:?}"),
    }
}

/// The stored value of `key` on the only vertex that has it.
fn stored(graph: &Graph, key: &str) -> ElementProperty {
    let locked = graph.read();
    let values = locked.traversal().v(None).values(key).to_list().unwrap();
    assert_eq!(
        values.len(),
        1,
        "expected exactly one '{key}' value, got {values:?}"
    );
    values.into_iter().next().unwrap()
}

fn count_uuid_typed(graph: &Graph, key: &str) -> ElementProperty {
    graph
        .read()
        .traversal()
        .v(None)
        .values(key)
        .is(P::type_of(GType::UUID))
        .count()
        .next()
        .unwrap()
        .unwrap()
}

#[test]
fn open_schema_coerces_a_canonical_string_to_uuid() {
    let graph = modern_with_person_schema();
    add_person(&graph, "uid", CANONICAL).expect("a canonical UUID string must be accepted");

    assert_eq!(stored(&graph, "uid"), ElementProperty::Uuid(CANONICAL_BITS));
    assert_eq!(count_uuid_typed(&graph, "uid"), ElementProperty::from(1i64));
}

#[test]
fn open_schema_rejects_a_non_canonical_uuid_string() {
    let graph = modern_with_person_schema();
    let err = schema_error(&add_person(&graph, "uid", "not-a-uuid").unwrap_err());
    match &err {
        SchemaError::CoercionFailed {
            element_type,
            label,
            property_name,
            declared_type,
            actual_value,
            reason,
            ..
        } => {
            assert_eq!(*element_type, "vertex");
            assert_eq!(label, "person");
            assert_eq!(property_name, "uid");
            assert_eq!(declared_type, "uuid");
            assert_eq!(&**actual_value, "\"not-a-uuid\"");
            assert_eq!(*reason, "not a canonical UUID (8-4-4-4-12 lowercase hex)");
        }
        other => panic!("expected CoercionFailed, got {other:?}"),
    }
    assert_eq!(
        err.to_string(),
        "Cannot store \"not-a-uuid\" in vertex 'person.uid' declared as uuid: not a canonical \
         UUID (8-4-4-4-12 lowercase hex)"
    );
}

#[test]
fn open_schema_rejects_an_integer_for_uuid() {
    let graph = modern_with_person_schema();
    let err = schema_error(&add_person(&graph, "uid", 42i64).unwrap_err());
    assert!(
        matches!(
            &err,
            SchemaError::TypeMismatch { expected_type, actual_type, .. }
                if expected_type == "uuid" && actual_type == "int64"
        ),
        "got {err:?}"
    );
}

#[test]
fn a_string_field_no_longer_accepts_a_uuid() {
    let graph = modern_with_person_schema();
    let err = schema_error(
        &add_person(&graph, "name", ElementProperty::Uuid(CANONICAL_BITS)).unwrap_err(),
    );
    assert!(
        matches!(
            &err,
            SchemaError::TypeMismatch { expected_type, actual_type, .. }
                if expected_type == "string" && actual_type == "uuid"
        ),
        "got {err:?}"
    );
}

#[test]
fn a_float64_field_stores_an_exact_integer_as_float() {
    let graph = modern_with_person_schema();
    add_person(&graph, "w", 3i64).expect("an exact integer must be accepted");

    let value = stored(&graph, "w");
    assert!(
        matches!(&value, ElementProperty::Number(n) if n.is_f64() && n.as_f64() == Some(3.0)),
        "expected Float64(3.0), got {value:?}"
    );
}

#[test]
fn a_float64_field_rejects_an_inexact_integer() {
    let graph = modern_with_person_schema();
    let too_big = (1i64 << 53) + 1;
    let err = schema_error(&add_person(&graph, "w", too_big).unwrap_err());
    assert!(
        matches!(
            &err,
            SchemaError::CoercionFailed { declared_type, reason, .. }
                if declared_type == "float64"
                    && *reason == "integer cannot be represented exactly as float64"
        ),
        "got {err:?}"
    );
}

#[test]
fn closed_mode_checks_every_declaring_label_after_coercion() {
    // `A.k: uuid` coerces the string; `B.k: string` then rejects the coerced UUID.
    let schema = schema_with(
        SchemaMode::Closed,
        &[
            ("A", &[("k", ValueType::Uuid)]),
            ("B", &[("k", ValueType::String)]),
        ],
    );

    // On add.
    let mut graph = TraversalGraph::new();
    graph.set_schema(schema.clone()).unwrap();
    let mut props = PropertyMap::default();
    props.insert("k", ElementProperty::from(CANONICAL));
    let err = graph
        .add_vertex(Some("1".into()), vec!["A".into(), "B".into()], props)
        .expect_err("the coerced UUID must fail against B's string declaration");
    assert!(
        matches!(
            &err,
            GraphError::Schema(SchemaError::TypeMismatch { label, expected_type, actual_type, .. })
                if label == "B" && expected_type == "string" && actual_type == "uuid"
        ),
        "got {err:?}"
    );

    // On set.
    let graph = GraphSource::empty();
    graph.write().set_schema(schema).unwrap();
    graph
        .write()
        .traversal_mut()
        .add_v(vec!["A", "B"])
        .next()
        .expect("a vertex without properties satisfies both labels");
    let err = graph
        .write()
        .traversal_mut()
        .v(None)
        .property("k", CANONICAL)
        .next()
        .expect_err("the coerced UUID must fail against B's string declaration");
    assert!(
        matches!(
            err.root_cause(),
            TraverserError::Graph(GraphError::Schema(SchemaError::TypeMismatch { label, .. }))
                if label == "B"
        ),
        "got {err:?}"
    );
}

#[test]
fn set_property_coerces_by_the_first_declaring_label_not_the_first_label() {
    // `X` is declared but does not declare `k`; `A` does.
    let schema = schema_with(
        SchemaMode::Open,
        &[
            ("X", &[("other", ValueType::String)]),
            ("A", &[("k", ValueType::Uuid)]),
        ],
    );
    let graph = GraphSource::empty();
    graph.write().set_schema(schema).unwrap();
    graph
        .write()
        .traversal_mut()
        .add_v(vec!["X", "A"])
        .next()
        .unwrap();
    graph
        .write()
        .traversal_mut()
        .v(None)
        .property("k", CANONICAL)
        .next()
        .expect("A's declaration must coerce the string");

    assert_eq!(stored(&graph, "k"), ElementProperty::Uuid(CANONICAL_BITS));
}

#[test]
fn schema_mode_none_never_coerces() {
    let graph = GraphSource::tinkerpop_modern();
    graph
        .write()
        .set_schema(person_schema(SchemaMode::None))
        .unwrap();
    add_person(&graph, "uid", CANONICAL).unwrap();
    add_person(&graph, "w", 3i64).unwrap();

    assert_eq!(stored(&graph, "uid"), ElementProperty::from(CANONICAL));
    assert!(matches!(stored(&graph, "w"), ElementProperty::Number(n) if n.is_i64()));
}

#[test]
fn reads_never_coerce() {
    let graph = modern_with_person_schema();
    add_person(&graph, "uid", CANONICAL).unwrap();

    let by_string = graph
        .read()
        .traversal()
        .v(None)
        .has("uid", CANONICAL)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(by_string, ElementProperty::from(0i64));
}

#[test]
fn a_vertex_or_edge_cannot_be_stored_as_a_property_in_any_mode() {
    for schema in [None, Some(person_schema(SchemaMode::Open))] {
        let graph = GraphSource::tinkerpop_modern();
        if let Some(schema) = schema {
            graph.write().set_schema(schema).unwrap();
        }

        let err = graph
            .write()
            .traversal_mut()
            .v("1")
            .property_by("friend", __::out(None))
            .next()
            .expect_err("a vertex must not be stored as a property value");
        match err.root_cause() {
            TraverserError::Graph(GraphError::InvalidPropertyValue {
                property_name,
                kind,
                ..
            }) => {
                assert_eq!(property_name, "friend");
                assert_eq!(*kind, "vertex");
            }
            other => panic!("expected InvalidPropertyValue, got {other:?}"),
        }

        let err = graph
            .write()
            .traversal_mut()
            .v("1")
            .property_by("e", __::out_e(None).fold())
            .next()
            .expect_err("an edge nested in an array must not be stored either");
        assert!(
            matches!(
                err.root_cause(),
                TraverserError::Graph(GraphError::InvalidPropertyValue { kind: "edge", .. })
            ),
            "got {err:?}"
        );

        // Nothing was stored.
        let stored = graph
            .read()
            .traversal()
            .v("1")
            .properties(vec!["friend", "e"])
            .count()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(stored, ElementProperty::from(0i64));
    }
}

#[test]
fn inserting_an_element_value_directly_is_rejected() {
    let mut graph = TraversalGraph::new();
    let mut props = PropertyMap::default();
    props.insert(
        "nested",
        ElementProperty::Array(vec![ElementProperty::Vertex(Box::default())]),
    );
    let err = graph
        .add_vertex(Some("1".into()), vec!["person".into()], props)
        .expect_err("a nested vertex must be rejected");
    assert_eq!(
        err.to_string(),
        "Property 'nested' cannot hold a vertex: vertices and edges are not property values"
    );
}

#[test]
fn inferred_schema_reports_uuid_for_uuid_values_and_string_for_strings() {
    let graph = GraphSource::tinkerpop_modern();
    graph
        .write()
        .traversal_mut()
        .v("1")
        .property_by("uid", __::constant(CANONICAL).cast(GType::UUID))
        .next()
        .unwrap();
    graph
        .write()
        .traversal_mut()
        .v("2")
        .property("uid_text", CANONICAL)
        .next()
        .unwrap();

    let schema = graph.read().traversal().infer_schema().unwrap();
    let person = &schema.vertices["person"].schema.properties;
    assert_eq!(person["uid"].type_name(), "uuid");
    assert_eq!(person["uid_text"].type_name(), "string");
}
