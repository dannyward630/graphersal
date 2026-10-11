use graphersal::ElementProperty;
use graphersal::prelude::{By::Count, By::Label};
use graphersal::{GraphSource, prelude::__};

#[test]
fn test_v_group_count_by_label_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let res = g.v(None).group_count().by(Label).to_list().unwrap();
    assert_eq!(res.len(), 1);
    match &res[0] {
        ElementProperty::Object(map) => {
            assert_eq!(map.get("person").and_then(|v| v.as_i64()), Some(4));
            assert_eq!(map.get("software").and_then(|v| v.as_i64()), Some(2));
        }
        _ => panic!("Expected Object property, got: {:?}", res[0]),
    }
}

#[test]
fn test_v_group_by_label_by_count_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    // 1. group().by(Label).by(Count)
    let g1 = g_lock.traversal();
    let res1 = g1.v(None).group().by(Label).by(Count).to_list().unwrap();
    assert_eq!(res1.len(), 1);
    match &res1[0] {
        ElementProperty::Object(map) => {
            assert_eq!(map.get("person").and_then(|v| v.as_i64()), Some(4));
            assert_eq!(map.get("software").and_then(|v| v.as_i64()), Some(2));
        }
        _ => panic!("Expected Object property"),
    }

    // 2. group().by(Label).by(__::count())
    let g2 = g_lock.traversal();
    let res2 = g2
        .v(None)
        .group()
        .by(Label)
        .by(__::count())
        .to_list()
        .unwrap();
    assert_eq!(res1, res2);
}

#[test]
fn test_e_group_count_by_label_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let res = g.e(None).group_count().by(Label).to_list().unwrap();
    assert_eq!(res.len(), 1);
    match &res[0] {
        ElementProperty::Object(map) => {
            assert_eq!(map.get("knows").and_then(|v| v.as_i64()), Some(2));
            assert_eq!(map.get("created").and_then(|v| v.as_i64()), Some(4));
        }
        _ => panic!("Expected Object property, got: {:?}", res[0]),
    }
}

#[test]
fn test_e_group_by_label_by_count_exact_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let res = g.e(None).group().by(Label).by(Count).to_list().unwrap();
    assert_eq!(res.len(), 1);
    match &res[0] {
        ElementProperty::Object(map) => {
            assert_eq!(map.get("knows").and_then(|v| v.as_i64()), Some(2));
            assert_eq!(map.get("created").and_then(|v| v.as_i64()), Some(4));
        }
        _ => panic!("Expected Object property"),
    }
}

#[test]
fn test_v_has_label_group_count_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g = g_lock.traversal();
    let res = g
        .v(None)
        .has_label("person")
        .group_count()
        .by(Label)
        .to_list()
        .unwrap();
    assert_eq!(res.len(), 1);
    match &res[0] {
        ElementProperty::Object(map) => {
            assert_eq!(map.get("person").and_then(|v| v.as_i64()), Some(4));
            assert_eq!(map.get("software"), None);
        }
        _ => panic!("Expected Object property"),
    }
}

#[test]
fn test_v_ids_group_count_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let g = g_lock.traversal();
    let res = g
        .v(vec!["1", "2", "3"])
        .group_count()
        .by(Label)
        .to_list()
        .unwrap();
    assert_eq!(res.len(), 1);
    match &res[0] {
        ElementProperty::Object(map) => {
            assert_eq!(map.get("person").and_then(|v| v.as_i64()), Some(2));
            assert_eq!(map.get("software").and_then(|v| v.as_i64()), Some(1));
        }
        _ => panic!("Expected Object property"),
    }
}

#[test]
fn test_group_count_followed_by_count() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // The grouped map is 1 object, so counting the stream produces 1
    let count = g
        .v(None)
        .group_count()
        .by(Label)
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count.as_i64().unwrap(), 1);
}

#[test]
fn test_group_count_pushdown_profile_transparency() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let stats = g.v(None).group_count().by(Label).profile().unwrap();
    let profile_str = stats.to_string();

    assert!(
        profile_str.contains("v().group_count().by(T.label)"),
        "Profile should show folded group_count: {}",
        profile_str
    );
    assert!(
        !profile_str.contains("groupCount()"),
        "Profile should not show standalone unoptimized groupCount step: {}",
        profile_str
    );
}

#[cfg(feature = "script")]
#[test]
fn test_rhai_dsl_group_count_pushdown_parity() {
    use std::sync::Arc;

    let graph = Arc::new(GraphSource::tinkerpop_modern());

    let res_v_gc =
        crate::script_host::eval(graph.clone(), "g.V().groupCount().by(By::Label).toList()")
            .unwrap();
    assert!(
        res_v_gc.contains("person") && res_v_gc.contains("software"),
        "Got: {}",
        res_v_gc
    );

    let res_v_group = crate::script_host::eval(
        graph.clone(),
        "g.V().group().by(By::Label).by(By::Count).toList()",
    )
    .unwrap();
    assert!(
        res_v_group.contains("person") && res_v_group.contains("software"),
        "Got: {}",
        res_v_group
    );

    let res_e_gc =
        crate::script_host::eval(graph.clone(), "g.E().groupCount().by(By::Label).toList()")
            .unwrap();
    assert!(
        res_e_gc.contains("knows") && res_e_gc.contains("created"),
        "Got: {}",
        res_e_gc
    );

    let res_e_group = crate::script_host::eval(
        graph.clone(),
        "g.E().group().by(By::Label).by(By::Count).toList()",
    )
    .unwrap();
    assert!(
        res_e_group.contains("knows") && res_e_group.contains("created"),
        "Got: {}",
        res_e_group
    );
}

/// The fused `group_count_pushdown` path (`v(labels: "nope").group_count().by(Label)` folded into
/// one `Init` step) and the unfused path (`optimizer.disabled` forces a real `Barrier` step after
/// `v()`) must agree on an empty result: both `{}`.
#[test]
fn test_group_count_pushdown_empty_result_matches_unfused() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let fused = g_lock
        .traversal()
        .v(None)
        .has_label("nope")
        .group_count()
        .by(Label)
        .to_list()
        .unwrap();

    let unfused = g_lock
        .traversal()
        .with("optimizer.disabled", vec!["group_count_pushdown"])
        .v(None)
        .has_label("nope")
        .group_count()
        .by(Label)
        .to_list()
        .unwrap();

    assert_eq!(fused, unfused);
    match &fused[0] {
        ElementProperty::Object(map) => assert!(map.is_empty()),
        other => panic!("expected an empty map, got {other:?}"),
    }
}
