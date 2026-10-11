//! `label()` and `hasLabel()` over property elements (`properties()`): the label of a vertex
//! property is its key. `id()` over a property element stays an error (no property ids).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run(script: &str) -> String {
    let value = eval_value(modern(), script).unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

fn err(script: &str) -> String {
    eval_value(modern(), script).expect_err(script).to_string()
}

#[test]
fn label_of_a_vertex_property_is_its_key() {
    assert_eq!(
        run(r#"g.V("1").properties().label().toList()"#),
        r#"["age", "name"]"#.replace("\"age\", \"name\"", "\"name\", \"age\"")
    );
    assert_eq!(
        run(r#"g.v("1").properties().label().to_list()"#),
        run(r#"g.V("1").properties().label().toList()"#)
    );
    assert_eq!(
        run(r#"g.V().properties().label().dedup().count().next()"#),
        "3"
    );
    assert_eq!(
        run(r#"g.V("1").properties("age").label().toList()"#),
        r#"["age"]"#
    );
}

#[test]
fn has_label_filters_property_elements_by_key() {
    assert_eq!(
        run(r#"g.V().properties().hasLabel("name").count().next()"#),
        "6"
    );
    assert_eq!(
        run(r#"g.v().properties().has_label("age").count().next()"#),
        "4"
    );
    assert_eq!(
        run(r#"g.V().properties("name").hasLabel("name").count().next()"#),
        "6"
    );
    assert_eq!(
        run(r#"g.V().properties("name").hasLabel("age").count().next()"#),
        "0"
    );
    assert_eq!(
        run(r#"g.V().properties().hasLabel("name", "lang").count().next()"#),
        "8"
    );
    assert_eq!(
        run(r#"g.V().properties().hasLabel("name").value().toList().len()"#),
        "6"
    );
}

#[test]
fn has_label_null_matches_nothing() {
    assert_eq!(
        run(r#"g.V().properties().hasLabel(()).count().next()"#),
        "0"
    );
    assert_eq!(
        run(r#"g.V().properties().hasLabel(()).toList().len()"#),
        "0"
    );
}

#[test]
fn label_readers_work_inside_children() {
    assert_eq!(
        run(r#"g.V().where(__.properties().hasLabel("lang")).count().next()"#),
        "2"
    );
    assert_eq!(
        run(r#"g.V().properties().where(__.hasLabel("age")).count().next()"#),
        "4"
    );
    assert_eq!(
        run(r#"g.V("1").properties().local(__.label()).toList().len()"#),
        "2"
    );
}

#[test]
fn values_and_edge_properties_are_not_elements() {
    for script in [
        r#"g.V().values("name").hasLabel("name").toList()"#,
        r#"g.V().values("name").label().toList()"#,
        r#"g.E().properties().hasLabel("weight").toList()"#,
        r#"g.E().properties().label().toList()"#,
    ] {
        let e = err(script);
        assert!(e.contains("Cast exception"), "{script}: {e}");
        assert!(e.contains("vertex property"), "{script}: {e}");
    }
}

#[test]
fn id_of_a_property_element_is_an_error_with_help() {
    for script in [
        r#"g.V().properties().id().toList()"#,
        r#"g.v().properties("name").id().to_list()"#,
    ] {
        let e = err(script);
        assert!(
            e.contains("property elements have no id in Graphersal"),
            "{script}: {e}"
        );
        assert!(e.contains("g.v().properties().key()"), "{script}: {e}");
    }
    // The documented alternatives run.
    assert_eq!(
        run(r#"g.V("1").properties("name").key().toList()"#),
        r#"["name"]"#
    );
    assert_eq!(
        run(r#"g.V("1").properties("name").element().id().toList()"#),
        r#"["1"]"#
    );
    // id() of a vertex and an edge is unchanged.
    assert_eq!(run(r#"g.V("1").id().toList()"#), r#"["1"]"#);
}
