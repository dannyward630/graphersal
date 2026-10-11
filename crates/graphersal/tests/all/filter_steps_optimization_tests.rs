use graphersal::GraphSource;
use graphersal::prelude::__;
use graphersal::prelude::*;
use std::time::Instant;

#[test]
fn test_has_id_pushdown_equivalence_tinkerpop_modern() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    // 1. Compare g.v(None).has_id("1") vs g.v("1")
    let g1 = g_lock.traversal();
    let res_pushdown = g1.v(None::<()>).has_id("1").id().to_list().unwrap();

    let g2 = g_lock.traversal();
    let res_direct = g2.v("1").id().to_list().unwrap();

    assert_eq!(res_pushdown, res_direct);
    assert_eq!(res_pushdown.len(), 1);
    assert_eq!(res_pushdown[0], ElementProperty::String("1".into()));

    // 2. Compare multi-id: g.v(None).has_id(["1", "2"]) vs g.v(["1", "2"])
    let g3 = g_lock.traversal();
    let res_multi_pushdown = g3
        .v(None::<()>)
        .has_id(vec!["1", "2"])
        .id()
        .to_list()
        .unwrap();

    let g4 = g_lock.traversal();
    let res_multi_direct = g4.v(vec!["1", "2"]).id().to_list().unwrap();

    assert_eq!(res_multi_pushdown, res_multi_direct);
    assert_eq!(res_multi_pushdown.len(), 2);
}

#[test]
fn test_edge_has_id_pushdown_equivalence_tinkerpop_modern() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g1 = g_lock.traversal();
    let res_pushdown = g1.e(None::<()>).has_id("0").id().to_list().unwrap();

    let g2 = g_lock.traversal();
    let res_direct = g2.e("0").id().to_list().unwrap();

    assert_eq!(res_pushdown, res_direct);
    assert_eq!(res_pushdown.len(), 1);
    assert_eq!(res_pushdown[0], ElementProperty::String("0".into()));
}

#[test]
fn test_has_id_pushdown_profile_shows_folded_step() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // In profile, has_id should be folded into v("1")
    let stats = g.v(None::<()>).has_id("1").count().profile().unwrap();
    let profile_str = stats.to_string();

    assert!(
        !profile_str.contains("has_id("),
        "Profile should not contain separate has_id step: {}",
        profile_str
    );
    assert!(
        profile_str.contains("v("),
        "Profile should contain v step: {}",
        profile_str
    );
}

#[test]
fn test_filter_reorder_and_id_pushdown_combined() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // Traversal with expensive where_t before cheap has_id and has_label
    // Pipeline should:
    // 1. Reorder: v(None) -> has_id("1") -> has_label("person") -> where_t(...)
    // 2. Fold has_id and has_label into Init(v)
    let res = g
        .v(None::<()>)
        .where_t(__::out(None::<()>))
        .has_label("person")
        .has_id("1")
        .id()
        .to_list()
        .unwrap();

    assert_eq!(res.len(), 1);
    assert_eq!(res[0], ElementProperty::String("1".into()));
}

#[test]
fn test_filter_steps_optimization_on_large_graph() {
    let graph = GraphSource::large_110k();
    let g_lock = graph.read();

    // The vertex ID in large_110k at level 0, index 0 is "V_0_0"
    let g1 = g_lock.traversal();
    let start_time_direct = Instant::now();
    let res_direct = g1.v("V_0_0").id().to_list().unwrap();
    let elapsed_direct = start_time_direct.elapsed();

    let g2 = g_lock.traversal();
    let start_time_pushdown = Instant::now();
    let res_pushdown = g2.v(None::<()>).has_id("V_0_0").id().to_list().unwrap();
    let elapsed_pushdown = start_time_pushdown.elapsed();

    assert_eq!(res_direct, res_pushdown);
    assert_eq!(res_pushdown.len(), 1);
    assert_eq!(res_pushdown[0], ElementProperty::String("V_0_0".into()));

    // Filter reordering test on large graph:
    // expensive where_t subquery reordered after cheap has_key and has_id
    let g3 = g_lock.traversal();
    let res_reordered = g3
        .v(None::<()>)
        .where_t(__::out(None::<()>))
        .has_key("level")
        .has_id("V_0_0")
        .id()
        .to_list()
        .unwrap();

    assert_eq!(res_reordered, res_direct);

    println!(
        "large_110k: direct = {:?}, pushdown = {:?}",
        elapsed_direct, elapsed_pushdown
    );
}

#[test]
fn test_nested_anonymous_traversal_optimization() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    // Inner where_t has out(None).where_t(__::has_label("person")) which should be optimized
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::has_label("person")))
        .profile()
        .unwrap();

    assert_eq!(stats.metrics().len(), 2);
    let where_node = &stats.metrics()[1];
    assert_eq!(where_node.nested().len(), 2);
}
