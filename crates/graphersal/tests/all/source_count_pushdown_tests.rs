use graphersal::GraphSource;
use graphersal::storage::Transactional;

#[test]
fn test_v_count_pushdown_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Direct g.v(None).count()
    let count = g.v(None).count().next().unwrap().unwrap();
    assert_eq!(count.as_i64().unwrap(), 6);
}

#[test]
fn test_e_count_pushdown_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Direct g.e(None).count()
    let count = g.e(None).count().next().unwrap().unwrap();
    assert_eq!(count.as_i64().unwrap(), 6);
}

#[test]
fn test_v_has_label_count_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g = g_lock.traversal();
    let person_count = g
        .v(None)
        .has_label("person")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(person_count.as_i64().unwrap(), 4);

    let g = g_lock.traversal();
    let software_count = g
        .v(None)
        .has_label("software")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(software_count.as_i64().unwrap(), 2);

    let g = g_lock.traversal();
    let multi_count = g
        .v(None)
        .has_label(vec!["person", "software"])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(multi_count.as_i64().unwrap(), 6);
}

#[test]
fn test_e_has_label_count_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g = g_lock.traversal();
    let knows_count = g
        .e(None)
        .has_label("knows")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(knows_count.as_i64().unwrap(), 2);

    let g = g_lock.traversal();
    let created_count = g
        .e(None)
        .has_label("created")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(created_count.as_i64().unwrap(), 4);
}

#[test]
fn test_v_has_id_count_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g = g_lock.traversal();
    let count_single = g.v(None).has_id("1").count().next().unwrap().unwrap();
    assert_eq!(count_single.as_i64().unwrap(), 1);

    let g = g_lock.traversal();
    let count_multi = g
        .v(None)
        .has_id(vec!["1", "2", "nonexistent"])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count_multi.as_i64().unwrap(), 2);
}

#[test]
fn test_count_pushdown_with_ids_and_labels() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    // ID 1 is a person
    let g = g_lock.traversal();
    let count_match = g
        .v("1")
        .has_label("person")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count_match.as_i64().unwrap(), 1);

    // ID 1 is not software
    let g = g_lock.traversal();
    let count_mismatch = g
        .v("1")
        .has_label("software")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count_mismatch.as_i64().unwrap(), 0);
}

#[test]
fn test_count_pushdown_profile_folding() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // In profile, count should be pushed down directly into v().count() or v(labels: ...).count(),
    // meaning no separate standalone count() step exists in execution statistics.
    let stats = g.v(None).has_label("person").count().profile().unwrap();
    let profile_str = stats.to_string();

    assert!(
        profile_str.contains(".count()"),
        "Profile should contain folded v step with .count(): {}",
        profile_str
    );
    assert!(
        !profile_str.contains("has_label("),
        "Profile should not contain has_label when folded: {}",
        profile_str
    );
}

#[cfg(feature = "script")]
#[test]
fn test_rhai_dsl_count_pushdown_parity() {
    use std::sync::Arc;

    let graph = Arc::new(GraphSource::tinkerpop_modern());

    let res_v_count = crate::script_host::eval(graph.clone(), "g.V().count().toList()").unwrap();
    assert!(
        res_v_count.contains("[6]") || res_v_count.contains("6"),
        "Got: {}",
        res_v_count
    );

    let res_v_label_count = crate::script_host::eval(
        graph.clone(),
        r#"g.V().hasLabel("person").count().toList()"#,
    )
    .unwrap();
    assert!(
        res_v_label_count.contains("[4]") || res_v_label_count.contains("4"),
        "Got: {}",
        res_v_label_count
    );

    let res_e_count = crate::script_host::eval(graph.clone(), "g.E().count().toList()").unwrap();
    assert!(
        res_e_count.contains("[6]") || res_e_count.contains("6"),
        "Got: {}",
        res_e_count
    );

    let res_e_label_count =
        crate::script_host::eval(graph.clone(), r#"g.E().hasLabel("knows").count().toList()"#)
            .unwrap();
    assert!(
        res_e_label_count.contains("[2]") || res_e_label_count.contains("2"),
        "Got: {}",
        res_e_label_count
    );
}

/// The fused `count_pushdown` path (`v(labels: "nope").count()` folded into one `Init` step) and
/// the unfused path (`optimizer.disabled` forces a real `Count` step after `v()`) must agree on
/// an empty result: both `0`, never one returning nothing while the other returns `0`.
#[test]
fn test_count_pushdown_empty_result_matches_unfused() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let fused = g_lock
        .traversal()
        .v(None)
        .has_label("nope")
        .count()
        .to_list()
        .unwrap();

    let unfused = g_lock
        .traversal()
        .with("optimizer.disabled", vec!["count_pushdown"])
        .v(None)
        .has_label("nope")
        .count()
        .to_list()
        .unwrap();

    assert_eq!(fused, unfused);
    assert_eq!(fused[0].as_i64(), Some(0));
}

/// The fused label count reads the statistics; it must agree with the unfused plan for repeated,
/// several and unknown labels, on vertices and edges, also after a mutation and a rollback.
#[test]
fn label_counts_from_the_statistics_match_the_unfused_plan() {
    let graph = GraphSource::tinkerpop_modern();
    let mut g_lock = graph.write();
    g_lock
        .traversal_mut()
        .add_v("person")
        .next()
        .unwrap()
        .unwrap();
    struct Abort;
    impl From<graphersal::error::GraphError> for Abort {
        fn from(_: graphersal::error::GraphError) -> Self {
            Abort
        }
    }
    let rolled_back = g_lock.transaction(|g| -> Result<(), Abort> {
        g.traversal_mut()
            .add_v("person")
            .next()
            .map_err(|_| Abort)?;
        Err(Abort)
    });
    assert!(rolled_back.is_err());
    let vertex_cases: [&[&str]; 5] = [
        &["person"],
        &["person", "person"],
        &["person", "software"],
        &["nope"],
        &["person", "nope"],
    ];
    for labels in vertex_cases {
        let fused = g_lock
            .traversal()
            .v(None)
            .has_label(labels.to_vec())
            .count()
            .to_list();
        let unfused = g_lock
            .traversal()
            .with("optimizer.disabled", vec!["count_pushdown"])
            .v(None)
            .has_label(labels.to_vec())
            .count()
            .to_list();
        assert_eq!(fused.unwrap(), unfused.unwrap(), "v {labels:?}");
    }
    let edge_cases: [&[&str]; 4] = [
        &["knows"],
        &["knows", "knows"],
        &["knows", "created"],
        &["knows", "nope", "knows"],
    ];
    for labels in edge_cases {
        let fused = g_lock
            .traversal()
            .e(None)
            .has_label(labels.to_vec())
            .count()
            .to_list();
        let unfused = g_lock
            .traversal()
            .with("optimizer.disabled", vec!["count_pushdown"])
            .e(None)
            .has_label(labels.to_vec())
            .count()
            .to_list();
        assert_eq!(fused.unwrap(), unfused.unwrap(), "e {labels:?}");
    }
    let persons = g_lock
        .traversal()
        .v(None)
        .has_label("person")
        .count()
        .next()
        .unwrap();
    assert_eq!(
        persons.unwrap().as_i64(),
        Some(5),
        "the added vertex counts, the rolled-back one not"
    );
}
