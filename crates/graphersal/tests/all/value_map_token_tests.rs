//! `valueMap(true, keys..)`: the `id`/`label` tokens, the null key, and the argument rules.
//!
//! Sources: the TinkerPop 3.8.2 feature files (`ValueMap.feature`), the Gherkin harness maps
//! `t[id]`/`t[label]` to the strings `id`/`label`, exactly as for `elementMap()`.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run_on(graph: Arc<Graph>, script: &str) -> String {
    let value = eval_value(graph, script).unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

fn run(script: &str) -> String {
    run_on(modern(), script)
}

fn list(value: impl Into<ElementProperty>) -> ElementProperty {
    ElementProperty::Array(vec![value.into()])
}

fn object(res: &ElementProperty) -> &graphersal::prelude::PropertyMap {
    match res {
        ElementProperty::Object(obj) => obj,
        other => panic!("expected an object, got {other:?}"),
    }
}

#[test]
fn value_map_true_adds_id_and_label_in_both_spellings() {
    let expected = r#"[#{"age": [29], "id": "1", "label": "person", "name": ["marko"]}]"#;
    assert_eq!(run(r#"g.V("1").valueMap(true).toList()"#), expected);
    assert_eq!(run(r#"g.v("1").value_map(true).to_list()"#), expected);
}

#[test]
fn value_map_true_with_keys_keeps_the_tokens() {
    let expected = r#"[#{"id": "1", "label": "person", "name": ["marko"]}]"#;
    assert_eq!(run(r#"g.V("1").valueMap(true, "name").toList()"#), expected);
    assert_eq!(
        run(r#"g.v("1").value_map(true, "name").to_list()"#),
        expected
    );
    // The TinkerPop scenario: valueMap(true, "name", "age") on the modern graph.
    assert_eq!(
        run(r#"g.V("1").valueMap(true, "name", "age").toList()"#),
        r#"[#{"age": [29], "id": "1", "label": "person", "name": ["marko"]}]"#
    );
}

#[test]
fn the_null_key_matches_nothing() {
    // TinkerPop `valueMap("name", "age", null)`.
    let expected = r#"[#{"age": [29], "name": ["marko"]}]"#;
    assert_eq!(
        run(r#"g.V("1").valueMap("name", "age", ()).toList()"#),
        expected
    );
    assert_eq!(
        run(r#"g.v("1").value_map("name", "age", ()).to_list()"#),
        expected
    );
    // `valueMap(null)` alone equals valueMap() (documented deviation).
    assert_eq!(
        run(r#"g.V("1").valueMap(()).toList()"#),
        run(r#"g.V("1").valueMap().toList()"#)
    );
}

#[test]
fn false_is_the_plain_value_map() {
    assert_eq!(
        run(r#"g.V("1").valueMap(false, "name").toList()"#),
        r#"[#{"name": ["marko"]}]"#
    );
}

#[test]
fn every_vertex_gets_the_tokens() {
    let rows = run(r#"g.V().valueMap(true).toList().len()"#);
    assert_eq!(rows, "6");
    assert_eq!(run(r#"g.V().valueMap(true).count().next()"#), "6");
    assert_eq!(
        run(
            r#"g.V().valueMap(true).toList().filter(|m| m.contains("id") && m.contains("label")).len()"#
        ),
        "6"
    );
}

#[test]
fn edges_get_id_and_label() {
    assert_eq!(
        run(r#"g.E("0").valueMap(true).toList()"#),
        r#"[#{"id": "0", "label": "knows", "weight": [0.5]}]"#
    );
}

#[test]
fn an_unlabeled_vertex_has_no_label_entry_like_element_map() {
    let graph = Arc::new(GraphSource::empty());
    run_on(graph.clone(), r#"g.addV().property("a", 1).next()"#);
    let with_tokens = run_on(graph.clone(), r#"g.V().valueMap(true).toList()"#);
    assert_eq!(with_tokens, r#"[#{"a": [1], "id": "1"}]"#);
    assert!(!with_tokens.contains("label"));
    let element_map = run_on(graph, r#"g.V().elementMap().toList()"#);
    assert!(!element_map.contains("label"));
}

#[test]
fn by_modulates_the_properties_but_not_the_tokens() {
    assert_eq!(
        run(r#"g.V("1").valueMap(true).by(__.unfold()).toList()"#),
        r#"[#{"age": 29, "id": "1", "label": "person", "name": "marko"}]"#
    );
    assert_eq!(
        run(r#"g.v("1").value_map(true, "name").by(__.unfold()).to_list()"#),
        r#"[#{"id": "1", "label": "person", "name": "marko"}]"#
    );
}

#[test]
fn anonymous_value_map_with_tokens_inside_local() {
    assert_eq!(
        run(r#"g.V("1").local(__.valueMap(true, "name")).toList()"#),
        r#"[#{"id": "1", "label": "person", "name": ["marko"]}]"#
    );
    assert_eq!(
        run(r#"g.V("1").local(__.valueMap(true)).toList().len()"#),
        "1"
    );
}

#[test]
fn rust_api_value_map_with_tokens() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let res = lock
        .traversal()
        .v("1")
        .value_map_with_tokens(vec!["name"])
        .to_list()
        .unwrap();
    let obj = object(&res[0]);
    assert_eq!(obj.get("name"), Some(&list("marko")));
    assert_eq!(obj.get("label"), Some(&ElementProperty::from("person")));
    assert_eq!(obj.get("id"), Some(&ElementProperty::from("1")));
    assert!(obj.get("age").is_none());

    // All keys, anonymous, and unchanged `value_map`.
    let res = lock
        .traversal()
        .v("1")
        .local(__::value_map_with_tokens(None::<()>))
        .to_list()
        .unwrap();
    assert_eq!(object(&res[0]).len(), 4);
    let res = lock
        .traversal()
        .v("1")
        .value_map(None::<()>)
        .to_list()
        .unwrap();
    assert_eq!(object(&res[0]).len(), 2);
}

#[test]
fn profile_shows_the_token_flag() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let text = lock
        .traversal()
        .v("1")
        .value_map_with_tokens(vec!["name"])
        .profile()
        .unwrap()
        .to_string();
    assert!(text.contains(r#"value_map(true, "name")"#), "{text}");
    let text = lock
        .traversal()
        .v("1")
        .value_map_with_tokens(None::<()>)
        .profile()
        .unwrap()
        .to_string();
    assert!(text.contains("value_map(true)"), "{text}");
}

#[test]
fn bad_arguments_fail_with_the_new_help() {
    for script in [
        r#"g.V().valueMap("a", true)"#,
        r#"g.V().value_map(1)"#,
        r#"g.V().valueMap(true, true)"#,
    ] {
        let err = eval_value(modern(), script).expect_err(script).to_string();
        assert!(
            err.contains("a leading `true` adds id and label"),
            "{script}: {err}"
        );
        assert!(
            err.contains("g.v().element_map(\\\"name\\\")") || err.contains("element_map("),
            "{script}: {err}"
        );
    }
}
