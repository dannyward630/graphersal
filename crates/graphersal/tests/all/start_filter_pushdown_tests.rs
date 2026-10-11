use graphersal::GraphSource;
use graphersal::prelude::ElementProperty;

#[test]
fn test_vertex_has_label_pushdown_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // With pushdown: g.v(None).has_label("person")
    let persons = g.v(None).has_label("person").id().to_list().unwrap();
    assert_eq!(persons.len(), 4);
    let ids: Vec<String> = persons
        .into_iter()
        .map(|p| p.as_string().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec!["1", "2", "4", "6"]);

    // With multiple labels: person and software
    let g = g_lock.traversal();
    let count = g
        .v(None)
        .has_label(vec!["person", "software"])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count.as_i64().unwrap(), 6);
}

#[test]
fn test_edge_has_label_pushdown_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let knows = g.e(None).has_label("knows").id().to_list().unwrap();
    assert_eq!(knows.len(), 2);

    let g = g_lock.traversal();
    let total_edges = g
        .e(None)
        .has_label(vec!["knows", "created"])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(total_edges.as_i64().unwrap(), 6);
}

#[test]
fn test_vertex_has_id_pushdown_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let single = g.v(None).has_id("1").id().to_list().unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0], ElementProperty::String("1".into()));

    let g = g_lock.traversal();
    let multiple = g.v(None).has_id(vec!["1", "4"]).id().to_list().unwrap();
    assert_eq!(multiple.len(), 2);
    let ids: Vec<String> = multiple
        .into_iter()
        .map(|p| p.as_string().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec!["1", "4"]);
}

#[test]
fn test_pushdown_profile_folding() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // In profile, the has_label step should be folded directly into v(labels: ...),
    // meaning no separate has_label step exists in execution statistics.
    let stats = g.v(None).has_label("person").count().profile().unwrap();
    let profile_str = stats.to_string();

    assert!(
        !profile_str.contains("has_label("),
        "Profile should not contain has_label when folded: {}",
        profile_str
    );
    assert!(
        profile_str.contains("v("),
        "Profile should contain v step: {}",
        profile_str
    );
}

#[test]
fn test_pushdown_with_ids_and_labels() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Query specific ID and match label
    let res = g.v("1").has_label("person").id().to_list().unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0], ElementProperty::String("1".into()));

    // Query specific ID but non-matching label
    let g = g_lock.traversal();
    let res2 = g.v("1").has_label("software").id().to_list().unwrap();
    assert_eq!(res2.len(), 0);
}

#[test]
fn test_pushdown_multiple_chained_labels() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Two disjoint labels chained should intersect to empty
    let res = g
        .v(None)
        .has_label("person")
        .has_label("software")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(res.len(), 0);
}
