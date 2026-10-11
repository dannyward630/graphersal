use graphersal::GraphSource;

#[test]
fn test_out_e_has_label_folding_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    // v("1").out_e(None).has_label("knows") vs v("1").out_e("knows")
    let folded_edges = g_lock
        .traversal()
        .v("1")
        .out_e(None)
        .has_label("knows")
        .to_list()
        .unwrap();
    let direct_edges = g_lock.traversal().v("1").out_e("knows").to_list().unwrap();

    assert_eq!(folded_edges.len(), 2);
    assert_eq!(folded_edges, direct_edges);
}

#[test]
fn test_in_e_and_both_e_has_label_folding() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let in_e_folded = g_lock
        .traversal()
        .v("3")
        .in_e(None)
        .has_label("created")
        .to_list()
        .unwrap();
    let in_e_direct = g_lock.traversal().v("3").in_e("created").to_list().unwrap();
    assert_eq!(in_e_folded.len(), 3);
    assert_eq!(in_e_folded, in_e_direct);

    let both_e_folded = g_lock
        .traversal()
        .v("4")
        .both_e(None)
        .has_label("created")
        .to_list()
        .unwrap();
    let both_e_direct = g_lock
        .traversal()
        .v("4")
        .both_e("created")
        .to_list()
        .unwrap();
    assert_eq!(both_e_folded.len(), 2);
    assert_eq!(both_e_folded, both_e_direct);
}

#[test]
fn test_degree_count_optimization_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    // out().count()
    let out_count = g_lock
        .traversal()
        .v("1")
        .out(None)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(out_count.as_i64().unwrap(), 3);

    // out("knows").count()
    let out_knows_count = g_lock
        .traversal()
        .v("1")
        .out("knows")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(out_knows_count.as_i64().unwrap(), 2);

    // in().count()
    let in_count = g_lock
        .traversal()
        .v("3")
        .in_(None)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(in_count.as_i64().unwrap(), 3);

    // in("knows").count() (vertex 3 only has "created" in-edges)
    let in_knows_count = g_lock
        .traversal()
        .v("3")
        .in_("knows")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(in_knows_count.as_i64().unwrap(), 0);

    // both().count()
    let both_count = g_lock
        .traversal()
        .v("4")
        .both(None)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(both_count.as_i64().unwrap(), 3); // 1 in (knows from 1), 2 out (created to 3, 5)

    // out_e().count()
    let out_e_count = g_lock
        .traversal()
        .v("1")
        .out_e(None)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(out_e_count.as_i64().unwrap(), 3);

    // in_e().count()
    let in_e_count = g_lock
        .traversal()
        .v("3")
        .in_e(None)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(in_e_count.as_i64().unwrap(), 3);

    // both_e().count()
    let both_e_count = g_lock
        .traversal()
        .v("4")
        .both_e(None)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(both_e_count.as_i64().unwrap(), 3);
}

#[test]
fn test_edge_label_folding_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    // When folded, has_label should not appear as a separate step
    let stats = g_lock
        .traversal()
        .v("1")
        .out_e(None)
        .has_label("knows")
        .profile()
        .unwrap();
    let profile_str = stats.to_string();

    assert!(
        profile_str.contains("out_e("),
        "Profile should contain out_e step: {}",
        profile_str
    );
    assert!(
        !profile_str.contains("has_label("),
        "Profile should not contain has_label step when folded: {}",
        profile_str
    );
}

#[cfg(feature = "script")]
#[test]
fn test_rhai_dsl_edge_folding_and_degree_count_parity() {
    use std::sync::Arc;

    let graph = Arc::new(GraphSource::tinkerpop_modern());

    let res_folded = crate::script_host::eval(
        graph.clone(),
        r#"g.V("1").outE().hasLabel("knows").count().toList()"#,
    )
    .unwrap();
    assert!(
        res_folded.contains("[2]") || res_folded.contains("2"),
        "Got: {}",
        res_folded
    );

    let res_out_count =
        crate::script_host::eval(graph.clone(), r#"g.V("1").out().count().toList()"#).unwrap();
    assert!(
        res_out_count.contains("[3]") || res_out_count.contains("3"),
        "Got: {}",
        res_out_count
    );

    let res_in_count =
        crate::script_host::eval(graph.clone(), r#"g.V("3").in().count().toList()"#).unwrap();
    assert!(
        res_in_count.contains("[3]") || res_in_count.contains("3"),
        "Got: {}",
        res_in_count
    );
}
