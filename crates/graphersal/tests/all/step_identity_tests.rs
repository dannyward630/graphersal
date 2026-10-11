//! Integration tests for the `identity()` traversal step.

use graphersal::prelude::*;
use graphersal::{GraphSource, prelude::__};

#[test]
fn test_identity_vertex_count() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let count_with_identity = g.v(None).identity().count().to_list().unwrap()[0]
        .as_i64()
        .unwrap();

    let g_direct = g_lock.traversal();
    let count_direct = g_direct.v(None).count().to_list().unwrap()[0]
        .as_i64()
        .unwrap();

    assert_eq!(count_with_identity, count_direct);
    assert_eq!(count_with_identity, 6);
}

#[test]
fn test_identity_preserves_values() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g1 = g_lock.traversal();
    let mut names_identity = g1
        .v(None)
        .has_label("person")
        .identity()
        .values("name")
        .to_list()
        .unwrap();

    let g2 = g_lock.traversal();
    let mut names_direct = g2
        .v(None)
        .has_label("person")
        .values("name")
        .to_list()
        .unwrap();

    names_identity.sort_by(|a, b| a.as_string().cmp(&b.as_string()));
    names_direct.sort_by(|a, b| a.as_string().cmp(&b.as_string()));

    assert_eq!(names_identity, names_direct);
    assert_eq!(
        names_identity,
        vec![
            ElementProperty::String("josh".into()),
            ElementProperty::String("marko".into()),
            ElementProperty::String("peter".into()),
            ElementProperty::String("vadas".into()),
        ]
    );
}

#[test]
fn test_identity_edge_traversal() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let edge_ids = g.e(None::<()>).identity().id().to_list().unwrap();
    assert_eq!(edge_ids.len(), 6);
}

#[test]
fn test_anonymous_identity() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Anonymous traversal with __::identity()
    let res = g
        .v(None::<()>)
        .where_t(__::identity().has_label("person"))
        .values("name")
        .to_list()
        .unwrap();

    assert_eq!(res.len(), 4);
}

#[test]
fn test_identity_profile_transparency() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let profile = g.v(None::<()>).identity().profile().unwrap();
    let report = profile.to_string();

    assert!(
        report.contains("identity()"),
        "Profile should show identity() step: {}",
        report
    );
    assert_eq!(profile.metrics().len(), 2);
    assert_eq!(profile.metrics()[1].name(), "identity()");
    assert_eq!(profile.metrics()[1].counts().traverser_count(), 6);
}

#[cfg(feature = "script")]
#[test]
fn test_identity_rhai_script() {
    use crate::script_host::eval;
    use std::sync::Arc;

    let graph = Arc::new(GraphSource::tinkerpop_modern());

    let res1 = eval(graph.clone(), r#"g.V().identity().toList()"#);
    assert!(
        res1.is_ok(),
        "g.V().identity().toList() failed: {:?}",
        res1.err()
    );

    let res2 = eval(graph, r#"g.v().identity().to_list()"#);
    assert!(
        res2.is_ok(),
        "g.v().identity().to_list() failed: {:?}",
        res2.err()
    );
}
