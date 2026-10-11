use graphersal::prelude::*;

#[test]
fn test_flat_traversal_profile_standard_width() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .values("name")
        .profile()
        .unwrap();

    assert!(!stats.metrics().iter().any(|m| !m.nested().is_empty()));
    assert_eq!(stats.metrics().len(), 2);
    assert_eq!(stats.metrics()[0].name(), "v()");
    assert_eq!(stats.metrics()[0].calls(), 1);
    assert_eq!(stats.metrics()[0].counts().traverser_count(), 6);

    let display_str = stats.to_string();
    // Flat table should not contain hierarchy markers
    assert!(!display_str.contains(r"\>"));
    // First separator line should be 101 '=' chars (60 + 1 + 4 + 1 + 7 + 1 + 7 + 1 + 10 + 1 + 8)
    assert!(display_str.contains(&"=".repeat(101)));
}

#[test]
fn test_single_nested_traversal_profiling() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>))
        .profile()
        .unwrap();

    assert!(stats.metrics().iter().any(|m| !m.nested().is_empty()));
    assert_eq!(stats.metrics().len(), 2);

    let v_node = &stats.metrics()[0];
    assert_eq!(v_node.name(), "v()");
    assert_eq!(v_node.calls(), 1);
    assert_eq!(v_node.counts().traverser_count(), 6);

    let where_node = &stats.metrics()[1];
    assert!(where_node.name().starts_with("where("));
    assert_eq!(where_node.calls(), 1);
    assert_eq!(where_node.count_in(), 6);
    assert_eq!(where_node.nested().len(), 1);

    let child_out = &where_node.nested()[0];
    assert!(child_out.name().starts_with("out"));
    assert_eq!(child_out.calls(), 6);
    assert_eq!(child_out.count_in(), 6);
    assert!(child_out.timing().is_some());
    let timing = child_out.timing().unwrap();
    assert!(timing.min_duration() <= timing.max_duration());

    let display_str = stats.to_string();
    assert!(display_str.contains(r"  \> out"));
    assert!(display_str.contains("[min:"));
    assert!(display_str.contains("avg:"));
    assert!(display_str.contains("max:"));
    // Expanded 70-column width separator should be 111 '=' chars (70 + 41)
    assert!(display_str.contains(&"=".repeat(111)));
}

#[test]
fn test_deep_nested_traversal_profiling() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::out(None::<()>).has_label("person")))
        .profile()
        .unwrap();

    assert!(stats.metrics().iter().any(|m| !m.nested().is_empty()));
    assert_eq!(stats.metrics().len(), 2);

    let outer_where = &stats.metrics()[1];
    assert_eq!(outer_where.nested().len(), 2);

    let child_out = &outer_where.nested()[0];
    assert!(child_out.name().starts_with("out"));
    assert_eq!(child_out.calls(), 6);

    let inner_where = &outer_where.nested()[1];
    assert!(inner_where.name().starts_with("where("));
    assert_eq!(inner_where.nested().len(), 2);

    let child_inner_out = &inner_where.nested()[0];
    assert!(child_inner_out.name().starts_with("out"));

    let child_has_label = &inner_where.nested()[1];
    assert!(child_has_label.name().starts_with("has_label"));
    assert!(child_has_label.calls() > 0);

    let display_str = stats.to_string();
    assert!(display_str.contains(r"  \> out"));
    assert!(display_str.contains(r"  \> where("));
    assert!(display_str.contains(r"        \> has_label"));
}

#[test]
fn test_unnested_where_profiling_transparency() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::has_label("person")))
        .profile()
        .unwrap();

    assert!(stats.metrics().iter().any(|m| !m.nested().is_empty()));
    assert_eq!(stats.metrics().len(), 2);

    let outer_where = &stats.metrics()[1];
    assert_eq!(outer_where.nested().len(), 2);

    let child_out = &outer_where.nested()[0];
    assert!(child_out.name().starts_with("out"));

    let child_has_label = &outer_where.nested()[1];
    assert!(child_has_label.name().starts_with("has_label"));

    let display_str = stats.to_string();
    assert!(display_str.contains(r"  \> out"));
    assert!(display_str.contains(r"  \> has_label"));
}

#[test]
fn test_multi_branch_and_step_profiling() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .and(vec![
            __::values("age").is(P::Lt(30)),
            __::values("name").is("marko"),
        ])
        .profile()
        .unwrap();

    assert!(stats.metrics().iter().any(|m| !m.nested().is_empty()));
    let and_node = &stats.metrics()[1];
    assert_eq!(and_node.calls(), 1);
    assert_eq!(and_node.count_in(), 6);
    // 2 branches of 2 steps each = 4 children
    assert_eq!(and_node.nested().len(), 4);
    assert_eq!(and_node.nested()[0].name(), "values(\"age\")");
    assert_eq!(and_node.nested()[0].calls(), 6);
    assert!(and_node.nested()[1].name().starts_with("is("));
    assert_eq!(and_node.nested()[2].name(), "values(\"name\")");
    assert!(and_node.nested()[3].name().starts_with("is("));

    let display_str = stats.to_string();
    assert!(display_str.contains(r"  \> values"));
}

#[test]
fn test_multi_branch_or_step_profiling() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .or(vec![__::values("name").is("lop"), __::has_label("person")])
        .profile()
        .unwrap();

    assert!(stats.metrics().iter().any(|m| !m.nested().is_empty()));
    let or_node = &stats.metrics()[1];
    assert_eq!(or_node.calls(), 1);
    assert_eq!(or_node.count_in(), 6);
    assert!(or_node.nested().len() >= 2);
}

#[test]
fn test_coalesce_and_optional_profiling() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let stats_opt = g_lock
        .traversal()
        .v("6")
        .optional(__::out(None::<()>))
        .profile()
        .unwrap();
    assert!(stats_opt.metrics().iter().any(|m| !m.nested().is_empty()));
    let opt_node = &stats_opt.metrics()[1];
    assert_eq!(opt_node.nested().len(), 1);
    assert!(opt_node.nested()[0].name().starts_with("out"));

    let stats_coalesce = g_lock
        .traversal()
        .v("1")
        .coalesce(vec![__::out_e("knows"), __::out_e("created")])
        .profile()
        .unwrap();
    assert!(
        stats_coalesce
            .metrics()
            .iter()
            .any(|m| !m.nested().is_empty())
    );
    let coalesce_node = &stats_coalesce.metrics()[1];
    assert!(!coalesce_node.nested().is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn test_metrics_json_shape() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>))
        .profile()
        .unwrap();

    let json: serde_json::Value = serde_json::from_str(&stats.to_json()).unwrap();
    assert!(json["duration_ns"].is_u64());
    let metrics = json["metrics"].as_array().unwrap();
    assert_eq!(metrics.len(), stats.metrics().len());
    let where_step = &metrics[1];
    assert_eq!(where_step["id"], "1");
    assert_eq!(where_step["nested"][0]["id"], "1.0.0");
    assert_eq!(
        where_step["nested"][0]["calls"].as_u64(),
        Some(stats.metrics()[1].nested()[0].calls())
    );
    assert_eq!(where_step["counts"]["traverser_count"].as_u64(), Some(3));
}

#[test]
fn test_nested_where_t_execution_without_step_start() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let res = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::has_label("person")))
        .id()
        .to_list()
        .unwrap();

    assert_eq!(res.len(), 1);
    assert_eq!(res[0], ElementProperty::String("1".into()));
}

#[test]
fn test_nested_where_t_large_110k_profile() {
    let graph = GraphSource::large_110k();
    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::has_label("person")))
        .project(vec!["name"])
        .by("name")
        .profile()
        .unwrap();

    println!("Large 110k Profile:\n{}", stats);
}

#[cfg(feature = "script")]
#[test]
fn test_rhai_script_nested_profiling() {
    use std::sync::Arc;
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let script = r#"g.V().where(__.out()).profile()"#;
    let res = crate::script_host::eval(graph, script).unwrap();
    assert!(res.contains(r"\> out"));
    assert!(res.contains("Traversal Metrics"));
}
