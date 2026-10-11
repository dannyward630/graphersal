//! Integration tests for the `union()` traversal step.

use graphersal::prelude::*;

#[test]
fn test_union_out_knows_and_created() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let names = g
        .v("1")
        .union(vec![__::out("knows"), __::out("created")])
        .values("name")
        .to_list()
        .unwrap();

    let name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();

    assert_eq!(name_strs.len(), 3);
    assert_eq!(name_strs, vec!["josh", "vadas", "lop"]);
}

#[test]
fn test_union_identity_and_out() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let names = g
        .v("1")
        .union(vec![__::identity(), __::out("knows")])
        .values("name")
        .to_list()
        .unwrap();

    let name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();

    assert_eq!(name_strs.len(), 3);
    assert_eq!(name_strs, vec!["marko", "josh", "vadas"]);
}

#[test]
fn test_union_empty_branches() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let results = g.v("1").union(vec![]).values("name").to_list().unwrap();

    assert!(results.is_empty());
}

#[test]
fn test_anonymous_union_where() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Query vertices that have either outgoing 'knows' or 'created' with name 'lop'
    let res = g
        .v(None::<()>)
        .where_t(__::union(vec![__::out("knows"), __::out("created")]).has("name", "lop"))
        .values("name")
        .to_list()
        .unwrap();

    let mut names: Vec<String> = res
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();
    names.sort();

    // Vertices 1 (marko), 4 (josh), 6 (peter) created 'lop'
    assert_eq!(names, vec!["josh", "marko", "peter"]);
}

#[test]
fn test_union_profile_transparency() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let stats = g
        .v("1")
        .union(vec![__::out("knows"), __::out("created")])
        .values("name")
        .profile()
        .unwrap();

    let report = stats.to_string();
    assert!(
        report.contains("union("),
        "Profile should show union step: {}",
        report
    );
    assert!(stats.metrics().iter().any(|m| !m.nested().is_empty()));
}

#[cfg(feature = "script")]
#[test]
fn test_union_rhai_script() {
    use crate::script_host::eval;
    use std::sync::Arc;

    let graph = Arc::new(GraphSource::tinkerpop_modern());

    let res1 = eval(
        graph.clone(),
        r#"g.V("1").union([__.out("knows"), __.out("created")]).values("name").toList()"#,
    );
    assert!(res1.is_ok(), "union query failed: {:?}", res1.err());
    let out1 = res1.unwrap();
    assert!(
        out1.contains("josh") && out1.contains("vadas") && out1.contains("lop"),
        "out1: {}",
        out1
    );

    let res2 = eval(
        graph.clone(),
        r#"g.v("1").union([__.identity(), __.out("knows")]).to_list()"#,
    );
    assert!(res2.is_ok(), "identity union failed: {:?}", res2.err());

    let res3 = eval(graph, r#"g.v("1").union([]).to_list()"#);
    assert!(res3.is_ok(), "empty union failed: {:?}", res3.err());
}
