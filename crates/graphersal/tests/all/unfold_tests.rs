//! `unfold()` of a path (its objects) and of a property that holds a list or a map.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn graph_with_list() -> Arc<Graph> {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let _ = eval_value(
        graph.clone(),
        r#"g.V("1").property("list", [1, 2, 3]).property("map", #{a: 1, b: 2}).next();"#,
    )
    .expect("seed");
    graph
}

fn int(graph: &Arc<Graph>, script: &str) -> i64 {
    eval_value(graph.clone(), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .as_int()
        .unwrap_or_else(|_| panic!("{script}: not an int"))
}

#[test]
fn a_path_unfolds_into_its_objects() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    assert_eq!(
        int(&graph, r#"g.V("1").out().path().unfold().count().next()"#),
        6
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V("1").outE().inV().path().unfold().count().next()"#
        ),
        9
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V("1").out().path().by("name").unfold().count().next()"#
        ),
        6
    );
    let names = eval_value(
        graph.clone(),
        r#"g.V("1").out("created").path().by("name").unfold().toList()"#,
    )
    .unwrap();
    assert_eq!(format!("{names:?}"), r#"["marko", "lop"]"#);
}

#[test]
fn a_list_property_unfolds() {
    let graph = graph_with_list();
    assert_eq!(
        int(&graph, r#"g.V().values("list").unfold().count().next()"#),
        3
    );
    assert_eq!(
        int(&graph, r#"g.V().values("list").unfold().sum().next()"#),
        6
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V().properties("list").value().unfold().count().next()"#
        ),
        3
    );
}

#[test]
fn a_map_property_unfolds_into_entries() {
    let graph = graph_with_list();
    assert_eq!(
        int(&graph, r#"g.V().values("map").unfold().count().next()"#),
        2
    );
}

#[test]
fn a_scalar_property_passes_through() {
    let graph = graph_with_list();
    assert_eq!(
        int(&graph, r#"g.V().values("name").unfold().count().next()"#),
        6
    );
    assert_eq!(
        int(&graph, r#"g.V().values("age").unfold().sum().next()"#),
        123
    );
}

#[test]
fn profile_shows_unfold_out_greater_than_in() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let lock = graph.read();
    let profile = lock
        .traversal()
        .v("1")
        .out(None)
        .path()
        .unfold()
        .profile()
        .unwrap()
        .to_string();
    let line = profile
        .lines()
        .find(|l| l.contains("unfold("))
        .expect("unfold row");
    assert!(line.contains('3') && line.contains('6'), "{line}");
}
