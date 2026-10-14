//! A vertex or an edge returned as a value (a terminal's result) keeps its id, label(s),
//! endpoints and properties apart: `VertexValue`/`EdgeValue` in Rust, `Vertex`/`Edge` in Rhai,
//! untyped GraphSON 3 in JSON (issue #2). A property named `id` or `label` never takes the place
//! of the element's own.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> rhai::Dynamic {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

/// The final list of `script` with every item as its text.
fn list(script: &str) -> Vec<String> {
    run(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|item| item.to_string())
        .collect()
}

fn text(script: &str) -> String {
    run(script).to_string()
}

#[test]
fn a_vertex_is_a_vertex_value_in_the_rust_api() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let result = lock.traversal().v("1").to_list().unwrap();
    let ElementProperty::Vertex(vertex) = &result[0] else {
        panic!("a vertex: {result:?}");
    };
    assert_eq!(vertex.id(), "1");
    assert_eq!(vertex.label(), Some("person"));
    assert_eq!(vertex.labels(), ["person"]);
    assert_eq!(vertex.properties().len(), 2);
    assert_eq!(vertex.properties()["name"], ElementProperty::from("marko"));
    assert!(!vertex.properties().contains_key("id"));
    assert!(!vertex.properties().contains_key("label"));

    let result = lock.traversal().e("0").to_list().unwrap();
    let ElementProperty::Edge(edge) = &result[0] else {
        panic!("an edge: {result:?}");
    };
    assert_eq!((edge.id(), edge.label()), ("0", Some("knows")));
    assert_eq!(
        (edge.out_v().id(), edge.out_v().label()),
        ("1", Some("person"))
    );
    assert_eq!(edge.in_v().id(), "2");
    assert_eq!(edge.properties()["weight"], ElementProperty::from(0.5));
}

#[test]
fn rhai_getters_read_the_element_and_the_indexer_its_properties() {
    assert_eq!(text(r#"type_of(g.V("1").next())"#), "Vertex");
    assert_eq!(text(r#"type_of(g.E("0").next())"#), "Edge");
    assert_eq!(text(r#"g.V("1").next().id"#), "1");
    assert_eq!(text(r#"g.V("1").next().label"#), "person");
    assert_eq!(text(r#"g.V("1").next().labels"#), r#"["person"]"#);
    assert_eq!(text(r#"g.V("1").next()["name"]"#), "marko");
    // A name that is no getter falls back to the indexer.
    assert_eq!(text(r#"g.V("1").next().name"#), "marko");
    assert_eq!(text(r#"g.V("1").next().properties.age"#), "29");
    assert_eq!(text(r#"g.V("1").next()["missing"]"#), "");
    assert_eq!(text(r#"g.V("1").next().to_string()"#), "v[1]");
    assert_eq!(text(r#"g.E("0").next().to_string()"#), "e[0][1-knows->2]");
    assert_eq!(text(r#"g.E("0").next().out_v.id"#), "1");
    assert_eq!(text(r#"g.E("0").next().outV.label"#), "person");
    assert_eq!(text(r#"g.E("0").next().in_v.to_string()"#), "v[2]");
    assert_eq!(text(r#"g.E("0").next().inV.id"#), "2");
    assert_eq!(text(r#"g.E("0").next()["weight"]"#), "0.5");
}

#[test]
fn a_property_named_id_or_label_never_hides_the_element_own() {
    let vertex =
        r#"let v = g.addV("person").property("label", "CUSTOM").property("id", "X").next();"#;
    assert_eq!(text(&format!("{vertex} v.label")), "person");
    assert_eq!(text(&format!("{vertex} v[\"label\"]")), "CUSTOM");
    assert_eq!(text(&format!("{vertex} v[\"id\"]")), "X");
    assert_ne!(text(&format!("{vertex} v.id")), "X");
    let edge =
        r#"let e = g.V("1").addE("knows").to(__.V("2")).property("label", "CUSTOM").next();"#;
    assert_eq!(text(&format!("{edge} e.label")), "knows");
    assert_eq!(text(&format!("{edge} e[\"label\"]")), "CUSTOM");
}

#[test]
fn elements_pass_back_into_a_query_and_compare_by_id() {
    assert_eq!(
        list(r#"let v = g.V("1").next(); g.V(v).values("name").toList()"#),
        ["marko"]
    );
    assert_eq!(
        list(r#"let v = g.V("1").next(); g.V().hasId(v).values("name").toList()"#),
        ["marko"]
    );
    assert_eq!(
        text(r#"let v = g.V("1").next(); g.inject(v).out().count().next()"#),
        "3"
    );
    assert_eq!(
        list(r#"let e = g.E("0").next(); g.inject(e).inV().values("name").toList()"#),
        ["vadas"]
    );
    assert_eq!(
        list(r#"let e = g.E("0").next(); g.V(e.out_v).values("name").toList()"#),
        ["marko"]
    );
    assert_eq!(
        text(r#"g.V("1").next() == g.V().has("name", "marko").next()"#),
        "true"
    );
    assert_eq!(text(r#"g.V("1").next() == g.V("2").next()"#), "false");
    assert_eq!(text(r#"g.E("0").next().out_v == g.V("1").next()"#), "true");
}

/// The JSON of the first result of `build` on the modern graph (the serde form, which every JSON
/// front end writes).
#[cfg(feature = "serde")]
fn json_of(
    build: impl for<'a> FnOnce(
        GraphTraversalSource<'a, &'a mut TraversalGraph>,
    ) -> GraphTraversalSource<'a, &'a mut TraversalGraph>,
) -> serde_json::Value {
    let graph = GraphSource::tinkerpop_modern();
    let mut lock = graph.write();
    let result = build(GraphTraversalSource::new(&mut *lock))
        .to_list()
        .unwrap();
    serde_json::to_value(&result[0]).unwrap()
}

#[test]
#[cfg(feature = "serde")]
fn json_is_untyped_graphson() {
    assert_eq!(
        json_of(|g| g.v("1")),
        serde_json::json!({"id": "1", "label": "person", "properties": {"name": "marko", "age": 29}})
    );
    assert_eq!(
        json_of(|g| g.e("0")),
        serde_json::json!({
            "id": "0", "label": "knows", "inVLabel": "person", "outVLabel": "person",
            "inV": "2", "outV": "1", "properties": {"weight": 0.5}
        })
    );
    // A property named like a top-level field stays inside "properties".
    let value = json_of(|g| g.add_v("person").property("label", "CUSTOM"));
    assert_eq!(value["label"], "person");
    assert_eq!(value["properties"]["label"], "CUSTOM");
}

#[test]
#[cfg(feature = "serde")]
fn a_multi_label_vertex_lists_its_labels() {
    let value = json_of(|g| g.add_v(vec!["Person", "Admin"]));
    assert_eq!(value["label"], "Person");
    assert_eq!(value["labels"], serde_json::json!(["Person", "Admin"]));
    assert_eq!(
        text(r#"g.addV(["Person", "Admin"]).next().labels"#),
        r#"["Person", "Admin"]"#
    );
    // A single-label vertex has no "labels" field.
    assert!(json_of(|g| g.v("1")).get("labels").is_none());
}

#[test]
fn edge_properties_may_hold_nested_objects() {
    let script = r#"g.V("1").addE("knows").to(__.V("2")).property("meta", #{since: 2020, tags: ["a"]}).next()"#;
    assert_eq!(text(&format!("{script}[\"meta\"].since")), "2020");
    assert_eq!(text(&format!("{script}[\"meta\"].tags")), r#"["a"]"#);
}

#[test]
#[cfg(feature = "display")]
fn tables_keep_the_element_columns_and_mark_same_named_properties() {
    let graph = GraphSource::tinkerpop_modern();
    let mut lock = graph.write();
    let table = GraphTraversalSource::new(&mut *lock)
        .add_v("person")
        .property("name", "Bill")
        .property("label", "CUSTOM")
        .property("id", "X")
        .to_table()
        .unwrap();
    let header = table.lines().nth(1).unwrap_or_default().to_string();
    for column in ["id", "label", "id (property)", "label (property)", "name"] {
        assert!(header.contains(column), "{column}: {table}");
    }
    let table = lock.traversal().e("0").to_table().unwrap();
    let header = table.lines().nth(1).unwrap_or_default().to_string();
    for column in ["id", "label", "outV", "inV", "weight"] {
        assert!(header.contains(column), "{column}: {table}");
    }
}

#[test]
fn paths_and_lists_hold_element_values() {
    assert_eq!(
        list(
            r#"g.V("1").outE("knows").inV().has("name", "vadas").path().next().map(|x| x.to_string())"#
        ),
        ["v[1]", "e[0][1-knows->2]", "v[2]"]
    );
    assert_eq!(
        list(r#"g.V("1").out("knows").toList().map(|v| v.name)"#),
        ["josh", "vadas"]
    );
}
