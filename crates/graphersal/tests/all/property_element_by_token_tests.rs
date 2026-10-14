//! `by(T.label)` / `by(T.id)` over property elements (`properties()`) and over plain values.
//! The label of a vertex property is its key, so `order().by(T.label)` sorts by key (it used to
//! return an empty result, because the by() resolver only knew element labels). A by(T.label) or
//! by(T.id) on a value that cannot carry the token is an error, never a silently dropped traverser.
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
    let e = eval_value(modern(), script).expect_err(script);
    format!("{e}")
}

const BY_KEY: &str = r#"["age", "age", "age", "age", "lang", "lang", "name", "name", "name", "name", "name", "name"]"#;

#[test]
fn order_by_label_sorts_property_elements_by_key_in_both_spellings_and_bulk_modes() {
    for merge in [true, false] {
        for q in [
            r#"g.V().properties().order().by(T.label).label().toList()"#,
            r#"g.v().properties().order().by(T.label).label().to_list()"#,
            r#"g.V().properties().order().by(T::label).label().toList()"#,
            r#"g.V().properties().order().by(__.label()).label().toList()"#,
            r#"g.V().properties().order().by(__.key()).label().toList()"#,
        ] {
            let q = q.replacen("g.", &format!(r#"g.with("bulk.merge", {merge})."#), 1);
            assert_eq!(run(&q), BY_KEY, "{q}");
        }
        let desc = format!(
            r#"g.with("bulk.merge", {merge}).V().properties().order().by(T.label, Order.desc).label().dedup().toList()"#
        );
        assert_eq!(run(&desc), r#"["name", "lang", "age"]"#, "{desc}");
    }
}

#[test]
fn group_dedup_project_by_label_work_over_property_elements() {
    assert_eq!(
        run(r#"g.V().properties().dedup().by(T.label).count().next()"#),
        "3"
    );
    assert!(
        run(r#"g.V().properties().group().by(T.label).by(__.count()).next()"#)
            .contains("\"name\": 6")
    );
    assert_eq!(
        run(r#"g.V().properties().project("k").by(T.label).select("k").dedup().count().next()"#),
        "3"
    );
}

#[test]
fn rust_api_orders_property_elements_by_label() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let keys = lock
        .traversal()
        .v(None)
        .properties(None)
        .order()
        .by(By::Label)
        .label()
        .to_list()
        .unwrap();
    assert_eq!(keys.len(), 12);
    assert_eq!(keys[0], ElementProperty::String("age".into()));
    assert_eq!(keys[11], ElementProperty::String("name".into()));
}

#[test]
fn token_on_plain_values_is_an_error_not_an_empty_result() {
    for q in [
        r#"g.V().values("name").order().by(T.label)"#,
        r#"g.V().values("name").group().by(T.id)"#,
        r#"g.V().values("name").dedup().by(T.label)"#,
        r#"g.V().values("age").project("a").by(T.id)"#,
        r#"g.V().values("name").aggregate("x").by(T.label)"#,
    ] {
        let e = err(&format!("{q}.toList()"));
        assert!(
            e.contains("can only be read from a vertex, an edge or a vertex property"),
            "{q}: {e}"
        );
    }
}

#[test]
fn id_token_on_a_property_element_is_an_error() {
    for q in [
        r#"g.V().properties().order().by(T.id)"#,
        r#"g.V().properties().dedup().by(T.id)"#,
        r#"g.V().properties().group().by(T.id)"#,
    ] {
        let e = err(&format!("{q}.toList()"));
        assert!(e.contains("property elements have no id"), "{q}: {e}");
    }
}

#[test]
fn element_and_map_tokens_keep_working() {
    assert_eq!(
        run(r#"g.V().order().by(T.label, Order.desc).by(T.id).id().toList().len()"#),
        "6"
    );
    assert_eq!(
        run(r#"g.V().elementMap().order().by(T.label).limit(1).select(T.label).next()"#),
        "\"person\""
    );
}

/// The label of an element map is the `T.label` token, not the string key `"label"`: without a
/// property named `label`, `select("label")` finds nothing (TinkerPop).
#[test]
fn an_element_map_has_no_string_label_key() {
    assert_eq!(
        run(r#"g.V().elementMap().order().by(T.label).limit(1).select("label").toList()"#),
        "[]"
    );
}
