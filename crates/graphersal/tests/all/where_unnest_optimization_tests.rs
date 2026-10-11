use graphersal::prelude::AnonymousTraversal;
use graphersal::prelude::*;

#[test]
fn test_where_has_label_root_level_fuses_with_source_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // 1. Result equivalence
    let expected = g
        .clone()
        .v(None::<()>)
        .has_label("person")
        .to_list()
        .unwrap();
    let actual = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .to_list()
        .unwrap();
    assert_eq!(actual.len(), 4);
    assert_eq!(actual, expected);

    // 2. Physical plan verification via profile()
    let stats = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .profile()
        .unwrap();

    // After WhereUnnestRule + SourceFilterPushdownRule:
    // logical `v().where_t([has_label("person")])` -> physical `v(labels: ["person"])`
    assert_eq!(stats.metrics().len(), 1);
    assert!(stats.metrics()[0].name().starts_with("v("));
    assert!(stats.metrics()[0].name().contains("person"));
    assert!(!stats.metrics().iter().any(|m| !m.nested().is_empty()));
}

#[test]
fn test_where_has_id_and_has_label_fuses_with_source() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let expected = g
        .clone()
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .to_list()
        .unwrap();
    let actual = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_label("person").has_id("1"))
        .to_list()
        .unwrap();

    assert_eq!(actual.len(), 1);
    assert_eq!(actual, expected);

    let stats = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_label("person").has_id("1"))
        .profile()
        .unwrap();

    // Folds into v(id: 1, labels: ["person"])
    assert_eq!(stats.metrics().len(), 1);
    assert!(stats.metrics()[0].name().starts_with("v("));
    assert!(stats.metrics()[0].name().contains("person"));
}

#[test]
fn test_where_with_count_pushdown() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let count_res = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(count_res[0].as_i64(), Some(4));

    let stats = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .count()
        .profile()
        .unwrap();

    // count() folds into v(labels: ["person"]) count_only
    assert_eq!(stats.metrics().len(), 1);
    assert!(stats.metrics()[0].name().starts_with("v("));
}

#[test]
fn test_nested_where_inside_navigation_where() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let results = g
        .clone()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::has_label("person")))
        .id()
        .to_list()
        .unwrap();

    // Modern graph: marko (1) -> josh (4), vadas (2) which are persons
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], ElementProperty::String("1".into()));

    let stats = g
        .clone()
        .v(None::<()>)
        .where_t(__::out(None::<()>).where_t(__::has_label("person")))
        .profile()
        .unwrap();

    // Outer where_t has 2 direct children: out() and has_label("person")
    assert_eq!(stats.metrics().len(), 2);
    let outer_where = &stats.metrics()[1];
    assert!(outer_where.name().starts_with("where("));
    assert_eq!(outer_where.nested().len(), 2);
    assert!(outer_where.nested()[0].name().starts_with("out"));
    assert!(outer_where.nested()[1].name().starts_with("has_label"));
}

#[test]
fn test_where_unnest_with_has_p_and_is_and_has_key() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // has_p inside where_t
    let res_has_p = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_p("age", P::Gt(30)))
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(res_has_p.len(), 2); // peter (35), josh (32)

    // has_key inside where_t
    let res_has_key = g
        .clone()
        .v(None::<()>)
        .where_t(__::has_key("age"))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(res_has_key[0].as_i64(), Some(4));

    // is inside where_t
    let res_is = g
        .clone()
        .v(None::<()>)
        .values("age")
        .where_t(__::is(P::Gt(30)))
        .to_list()
        .unwrap();
    assert_eq!(res_is.len(), 2);
}

#[test]
fn test_where_empty_traversal_noop() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let count_res = g
        .clone()
        .v(None::<()>)
        .where_t(AnonymousTraversal::new())
        .count()
        .to_list()
        .unwrap();
    assert_eq!(count_res[0].as_i64(), Some(6));
}

#[cfg(feature = "script")]
#[test]
fn test_rhai_script_where_unnest_parity() {
    use crate::script_host::eval;
    use std::sync::Arc;

    let graph = Arc::new(GraphSource::tinkerpop_modern());

    // Root-level where_t with hasLabel unnesting & counting
    let script_res = eval(
        graph.clone(),
        "g.V().where(__.hasLabel(\"person\")).count().toList()",
    )
    .unwrap();
    assert!(
        script_res.contains("4") || script_res.contains("[4]"),
        "Unexpected output: {}",
        script_res
    );

    // Nested where inside where
    let script_res_nested = eval(
        graph.clone(),
        "g.V().where(__.out().where(__.hasLabel(\"person\"))).id().toList()",
    )
    .unwrap();
    assert!(
        script_res_nested.contains("1") || script_res_nested.contains("[\"1\"]"),
        "Unexpected output: {}",
        script_res_nested
    );

    // Profile output via script showing unnested physical plan
    let profile_stats = eval(graph, "g.V().where(__.hasLabel(\"person\")).profile()").unwrap();
    assert!(profile_stats.contains("v("));
    assert!(profile_stats.contains("person"));
    assert!(!profile_stats.contains("where_t"));
}
