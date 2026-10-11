//! Integration tests for the query-language surface of the type system: the `GType` tokens, the
//! `typeOf()` predicate and the coercion steps.

use graphersal::prelude::*;
use graphersal::schema::ValueType;

/// The modern graph with `josh`'s age stored as the string `"999"`, which is the mixed-type
/// field the type system exists to locate and repair.
fn graph_with_string_age() -> Graph {
    let graph = GraphSource::tinkerpop_modern();
    {
        let mut writable = graph.write();
        let _ = writable
            .traversal_mut()
            .v("4")
            .property("age", "999")
            .next();
    }
    graph
}

#[test]
fn type_of_locates_the_values_of_a_mixed_field() {
    let graph = graph_with_string_age();
    let locked = graph.read();

    let offending = locked
        .traversal()
        .v(None)
        .has_label("person")
        .values("age")
        .is(P::type_of(GType::STRING))
        .to_list()
        .unwrap();
    assert_eq!(offending, vec![ElementProperty::from("999")]);

    let sound = locked
        .traversal()
        .v(None)
        .has_label("person")
        .values("age")
        .is(P::type_of(GType::LONG))
        .count()
        .next()
        .unwrap();
    assert_eq!(sound, Some(ElementProperty::from(3i64)));
}

#[test]
fn type_of_accepts_a_type_as_well_as_a_token() {
    let graph = graph_with_string_age();
    let locked = graph.read();

    let by_token = locked
        .traversal()
        .v(None)
        .values("age")
        .is(P::type_of(GType::STRING))
        .count()
        .next()
        .unwrap();
    let by_type = locked
        .traversal()
        .v(None)
        .values("age")
        .is(P::type_of(ValueType::String))
        .count()
        .next()
        .unwrap();
    assert_eq!(by_token, by_type);
}

#[test]
fn the_number_token_matches_both_numeric_types() {
    let graph = graph_with_string_age();
    let locked = graph.read();

    let numeric = locked
        .traversal()
        .v(None)
        .values("age")
        .is(P::type_of(GType::NUMBER))
        .count()
        .next()
        .unwrap();
    assert_eq!(numeric, Some(ElementProperty::from(3i64)));

    let weights = locked
        .traversal()
        .e(None)
        .values("weight")
        .is(P::type_of(GType::NUMBER))
        .count()
        .next()
        .unwrap();
    assert_eq!(weights, Some(ElementProperty::from(6i64)));
}

#[test]
fn element_tokens_separate_a_mixed_stream() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();

    let vertices = locked
        .traversal()
        .v(None)
        .is(P::type_of(GType::VERTEX))
        .count()
        .next()
        .unwrap();
    assert_eq!(vertices, Some(ElementProperty::from(6i64)));

    let edges = locked
        .traversal()
        .e(None)
        .is(P::type_of(GType::EDGE))
        .count()
        .next()
        .unwrap();
    assert_eq!(edges, Some(ElementProperty::from(6i64)));

    // A vertex answers only to its own token: its properties form a map, but it is not one.
    let as_map = locked
        .traversal()
        .v(None)
        .is(P::type_of(GType::MAP))
        .count()
        .next()
        .unwrap();
    assert_eq!(as_map, Some(ElementProperty::from(0i64)));

    // ... and an edge is not a vertex.
    let edges_as_vertices = locked
        .traversal()
        .e(None)
        .is(P::type_of(GType::VERTEX))
        .count()
        .next()
        .unwrap();
    assert_eq!(edges_as_vertices, Some(ElementProperty::from(0i64)));
}

#[test]
fn the_list_token_matches_a_folded_stream() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();

    let folded = locked
        .traversal()
        .v(None)
        .values("name")
        .fold()
        .is(P::type_of(GType::LIST))
        .count()
        .next()
        .unwrap();
    assert_eq!(folded, Some(ElementProperty::from(1i64)));
}

#[test]
fn coercion_repairs_the_mixed_field_for_an_aggregation() {
    let graph = graph_with_string_age();
    let locked = graph.read();

    // Without the conversion the aggregation fails on the one string value.
    assert!(
        locked
            .traversal()
            .v(None)
            .has_label("person")
            .values("age")
            .max()
            .next()
            .is_err()
    );

    let max = locked
        .traversal()
        .v(None)
        .has_label("person")
        .values("age")
        .as_number(GType::LONG)
        .max()
        .next()
        .unwrap();
    assert_eq!(max, Some(ElementProperty::from(999i64)));
}

#[test]
fn type_of_is_ordered_before_an_expensive_filter() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();

    // `is(typeOf(..))` reads a type rather than a value, so the optimizer must run it before a
    // sub-traversal filter, exactly as it does for has_label / has_id.
    let plan = locked
        .traversal()
        .v(None)
        .where_t(__::out(None))
        .is(P::type_of(GType::VERTEX))
        .profile()
        .unwrap()
        .to_string();

    let type_check = plan
        .find("is(P.typeOf(GType.VERTEX))")
        .unwrap_or_else(|| panic!("missing typeOf in: {plan}"));
    let sub_traversal = plan
        .find("where(")
        .unwrap_or_else(|| panic!("missing where in: {plan}"));
    assert!(
        type_check < sub_traversal,
        "the cheap type check must be planned first in: {plan}"
    );
}

/// The `GType` token spellings are observable in query text, so they are a stable contract
/// (design decision D14) and are pinned here like the `ValueType` renderings.
#[test]
fn gtype_token_renderings_are_pinned() {
    assert_eq!(GType::NULL.to_string(), "null");
    assert_eq!(GType::BOOLEAN.to_string(), "boolean");
    assert_eq!(GType::INT.to_string(), "int64");
    assert_eq!(GType::LONG.to_string(), "int64");
    assert_eq!(GType::FLOAT.to_string(), "float64");
    assert_eq!(GType::DOUBLE.to_string(), "float64");
    assert_eq!(GType::NUMBER.to_string(), "number");
    assert_eq!(GType::STRING.to_string(), "string");
    assert_eq!(GType::UUID.to_string(), "uuid");
    assert_eq!(GType::LIST.to_string(), "list");
    assert_eq!(GType::MAP.to_string(), "map");
    assert_eq!(GType::VERTEX.to_string(), "vertex");
    assert_eq!(GType::EDGE.to_string(), "edge");
    assert_eq!(GType::PATH.to_string(), "path");
}
