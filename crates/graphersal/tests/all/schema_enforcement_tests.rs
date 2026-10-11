//! Write enforcement of a stored schema through the storage API: closed-mode labels,
//! properties, types, constraints and topology; open mode; the dry run. The node-level value
//! check is a unit test of `graph/schema/validation.rs`.
use graphersal::prelude::*;
use graphersal::schema::ViolationKind;

use crate::schema_support::schema;

/// Person(name: string 2..50, age: integer 0..150, bio?: string|null), Software(name, lang),
/// `created(weight: number 0..1): Person -> Software`.
const STRICT: &str = r#"{
  "mode": "closed",
  "vertices": {
    "Person": {"schema": {"type": "object", "properties": {
        "name": {"type": "string", "minLength": 2, "maxLength": 50},
        "age": {"type": "integer", "minimum": 0, "maximum": 150},
        "bio": {"type": ["string", "null"]}},
      "required": ["name", "age"]}},
    "Software": {"schema": {"type": "object", "properties": {
        "name": {"type": "string"}, "lang": {"type": "string"}},
      "required": ["name", "lang"]}}
  },
  "edges": {
    "created": {"connections": [{"from": "Person", "to": "Software"}],
      "schema": {"type": "object", "properties": {
        "weight": {"type": "number", "minimum": 0, "maximum": 1}}, "required": ["weight"]}}
  }
}"#;

fn build_strict_test_schema() -> GraphSchema {
    schema(STRICT)
}

#[test]
fn test_strict_mode_unknown_vertex_label() {
    let schema = build_strict_test_schema();
    let mut graph = TraversalGraph::new();
    graph.set_schema(schema).unwrap();

    let mut props = PropertyMap::default();
    props.insert("name", ElementProperty::String("ET".into()));

    let res = graph.add_vertex(Some("1".into()), vec!["Alien".into()], props);
    match res.unwrap_err() {
        GraphError::Schema(SchemaError::UnknownVertexLabel { label, allowed, .. }) => {
            assert_eq!(label, "Alien");
            assert!(allowed.contains(&"Person".to_string()));
            assert!(allowed.contains(&"Software".to_string()));
        }
        err => panic!("Expected SchemaError::UnknownVertexLabel, got {:?}", err),
    }
}

#[test]
fn test_strict_mode_missing_required_property() {
    let mut graph = TraversalGraph::new();
    graph.set_schema(build_strict_test_schema()).unwrap();

    let mut props = PropertyMap::default();
    props.insert("name", ElementProperty::String("Alice".into()));
    // Missing 'age'
    let res = graph.add_vertex(Some("1".into()), vec!["Person".into()], props);
    match res.unwrap_err() {
        GraphError::Schema(SchemaError::MissingRequiredProperty { property_name, .. }) => {
            assert_eq!(property_name, "age");
        }
        err => panic!("Expected SchemaError::MissingRequiredProperty, got {err:?}"),
    }
}

#[test]
fn test_strict_mode_undeclared_property() {
    let mut graph = TraversalGraph::new();
    graph.set_schema(build_strict_test_schema()).unwrap();

    let mut props = PropertyMap::default();
    props.insert("name", ElementProperty::String("Alice".into()));
    props.insert("age", ElementProperty::Number(30.into()));
    props.insert("extra", ElementProperty::String("undeclared".into()));

    let res = graph.add_vertex(Some("1".into()), vec!["Person".into()], props);
    match res.unwrap_err() {
        GraphError::Schema(SchemaError::UndeclaredProperty { property_name, .. }) => {
            assert_eq!(property_name, "extra");
        }
        err => panic!("Expected SchemaError::UndeclaredProperty, got {err:?}"),
    }
}

#[test]
fn test_strict_mode_type_mismatch() {
    let mut graph = TraversalGraph::new();
    graph.set_schema(build_strict_test_schema()).unwrap();

    let mut props = PropertyMap::default();
    props.insert("name", ElementProperty::String("Alice".into()));
    props.insert("age", ElementProperty::String("thirty".into()));

    let res = graph.add_vertex(Some("1".into()), vec!["Person".into()], props);
    match res.unwrap_err() {
        GraphError::Schema(SchemaError::TypeMismatch { property_name, .. }) => {
            assert_eq!(property_name, "age");
        }
        err => panic!("Expected SchemaError::TypeMismatch, got {err:?}"),
    }
}

#[test]
fn test_strict_mode_constraint_violation() {
    let mut graph = TraversalGraph::new();
    graph.set_schema(build_strict_test_schema()).unwrap();

    // 1. Age > 150
    let mut props = PropertyMap::default();
    props.insert("name", ElementProperty::String("Methuselah".into()));
    props.insert("age", ElementProperty::Number(200.into()));
    let res = graph.add_vertex(Some("1".into()), vec!["Person".into()], props);
    match res.unwrap_err() {
        GraphError::Schema(SchemaError::ConstraintViolation {
            property_name,
            constraint_rule,
            ..
        }) => {
            assert_eq!(property_name, "age");
            assert_eq!(constraint_rule, "value <= 150");
        }
        err => panic!("Expected SchemaError::ConstraintViolation, got {err:?}"),
    }

    // 2. Name length < 2
    let mut props2 = PropertyMap::default();
    props2.insert("name", ElementProperty::String("X".into()));
    props2.insert("age", ElementProperty::Number(25.into()));
    let res2 = graph.add_vertex(Some("2".into()), vec!["Person".into()], props2);
    match res2.unwrap_err() {
        GraphError::Schema(SchemaError::ConstraintViolation { property_name, .. }) => {
            assert_eq!(property_name, "name");
        }
        err => panic!("Expected SchemaError::ConstraintViolation, got {err:?}"),
    }
}

#[test]
fn test_strict_mode_invalid_edge_topology() {
    let mut graph = TraversalGraph::new();
    graph.set_schema(build_strict_test_schema()).unwrap();

    let mut p1_props = PropertyMap::default();
    p1_props.insert("name", ElementProperty::String("Alice".into()));
    p1_props.insert("age", ElementProperty::Number(30.into()));
    let v_p1 = graph
        .add_vertex(Some("1".into()), vec!["Person".into()], p1_props)
        .unwrap();

    let mut p2_props = PropertyMap::default();
    p2_props.insert("name", ElementProperty::String("Bob".into()));
    p2_props.insert("age", ElementProperty::Number(35.into()));
    let v_p2 = graph
        .add_vertex(Some("2".into()), vec!["Person".into()], p2_props)
        .unwrap();

    let mut s_props = PropertyMap::default();
    s_props.insert("name", ElementProperty::String("Graphersal".into()));
    s_props.insert("lang", ElementProperty::String("Rust".into()));
    let v_s = graph
        .add_vertex(Some("3".into()), vec!["Software".into()], s_props)
        .unwrap();

    let mut e_props = PropertyMap::default();
    e_props.insert("weight", ElementProperty::Number(0.8.into()));
    assert!(
        graph
            .add_edge(
                Some("e1".into()),
                Some("created".into()),
                v_p1,
                v_s,
                e_props.clone()
            )
            .is_ok()
    );

    let invalid_e = graph.add_edge(
        Some("e2".into()),
        Some("created".into()),
        v_p1,
        v_p2,
        e_props,
    );
    match invalid_e.unwrap_err() {
        GraphError::Schema(SchemaError::InvalidEdgeTopology {
            source_label,
            edge_label,
            target_label,
            allowed,
            ..
        }) => {
            assert_eq!(source_label, "Person");
            assert_eq!(edge_label, "created");
            assert_eq!(target_label, "Person");
            assert_eq!(
                allowed,
                vec!["(:Person)-[:created]->(:Software)".to_string()]
            );
        }
        err => panic!("Expected SchemaError::InvalidEdgeTopology, got {err:?}"),
    }
}

#[test]
fn test_open_mode_permits_open_properties_with_type_safety() {
    let schema = build_strict_test_schema().with_mode(SchemaMode::Open);
    let mut graph = TraversalGraph::new();
    graph.set_schema(schema).unwrap();

    // Undeclared vertex label is allowed in Open mode
    let mut alien_props = PropertyMap::default();
    alien_props.insert("planet", ElementProperty::String("Mars".into()));
    assert!(
        graph
            .add_vertex(Some("1".into()), vec!["Alien".into()], alien_props)
            .is_ok()
    );

    // Undeclared properties on a declared label are allowed in Open mode
    let mut person_props = PropertyMap::default();
    person_props.insert("name", ElementProperty::String("Alice".into()));
    person_props.insert("age", ElementProperty::from(30));
    person_props.insert("nickname", ElementProperty::String("Ali".into()));
    assert!(
        graph
            .add_vertex(Some("2".into()), vec!["Person".into()], person_props)
            .is_ok()
    );

    // But declared properties MUST still follow type & constraints
    let mut bad_person_props = PropertyMap::default();
    bad_person_props.insert("name", ElementProperty::String("Bob".into()));
    bad_person_props.insert("age", ElementProperty::String("invalid_age".into()));
    let bad_person = graph.add_vertex(Some("3".into()), vec!["Person".into()], bad_person_props);
    assert!(matches!(
        bad_person.unwrap_err(),
        GraphError::Schema(SchemaError::TypeMismatch { .. })
    ));
}

/// `additionalProperties` on the label's root overrides the mode's default, both ways.
#[test]
fn test_explicit_additional_properties_overrides_the_mode() {
    let closed_but_open_label = schema(
        r#"{"mode": "closed", "vertices": {"note": {"schema": {"type": "object",
            "properties": {"text": {"type": "string"}}, "additionalProperties": true}}}}"#,
    );
    let mut graph = TraversalGraph::new();
    graph.set_schema(closed_but_open_label).unwrap();
    let mut props = PropertyMap::default();
    props.insert("text", ElementProperty::from("hi"));
    props.insert("anything", ElementProperty::from(1));
    assert!(graph.add_vertex(None, vec!["note".into()], props).is_ok());

    let open_but_strict_label = schema(
        r#"{"mode": "open", "vertices": {"note": {"schema": {"type": "object",
            "properties": {"text": {"type": "string"}}, "additionalProperties": false}}}}"#,
    );
    let mut graph = TraversalGraph::new();
    graph.set_schema(open_but_strict_label).unwrap();
    let mut props = PropertyMap::default();
    props.insert("anything", ElementProperty::from(1));
    assert!(matches!(
        graph.add_vertex(None, vec!["note".into()], props),
        Err(GraphError::Schema(SchemaError::UndeclaredProperty { .. }))
    ));
}

#[test]
fn test_pre_validation_against_schema() {
    let graph = TraversalGraph::tinkerpop_modern();
    let g = graph.traversal();

    // The inferred schema in closed mode fits the data it came from.
    let inferred = g.infer_schema().unwrap().with_mode(SchemaMode::Closed);
    let validation = g.validate_schema(&inferred).unwrap();
    assert!(validation.is_valid(), "{}", validation.violations);

    // A stricter schema: age <= 30 (marko 29, vadas 27, josh 32, peter 35).
    let mut strict_schema = inferred;
    let age = strict_schema
        .vertices
        .get_mut("person")
        .and_then(|person| person.schema.properties.get_mut("age"))
        .expect("age is inferred");
    *age = age
        .clone()
        .with_constraint(|c| c.maximum = Some(Number::from_i64(30)));

    let validation = g.validate_schema(&strict_schema).unwrap();
    let report = &validation.violations;
    // One group: person.age maximum, on josh (32) and peter (35).
    assert_eq!(report.groups.len(), 1, "{report}");
    assert_eq!(report.groups[0].kind, ViolationKind::ConstraintViolation);
    assert_eq!(report.groups[0].field.as_deref(), Some("age"));
    assert_eq!(report.groups[0].count, 2);
    // The diff names the tightened bound.
    assert!(!validation.compatible());
}

#[test]
fn test_property_step_schema_validation() {
    let mut graph = TraversalGraph::new();
    graph.set_schema(build_strict_test_schema()).unwrap();

    let mut p_props = PropertyMap::default();
    p_props.insert("name", ElementProperty::String("Alice".into()));
    p_props.insert("age", ElementProperty::Number(25.into()));
    graph
        .add_vertex(Some("1".into()), vec!["Person".into()], p_props)
        .unwrap();

    assert!(
        graph
            .traversal_mut()
            .v("1")
            .property("bio", "Software engineer")
            .next()
            .is_ok()
    );

    let err = graph
        .traversal_mut()
        .v("1")
        .property("unregistered", "val")
        .next()
        .unwrap_err();
    match err.root_cause() {
        TraverserError::Graph(GraphError::Schema(SchemaError::UndeclaredProperty {
            property_name,
            ..
        })) => assert_eq!(property_name, "unregistered"),
        other => panic!("Expected UndeclaredProperty, got {other:?}"),
    }
}
