//! Regression tests for engine correctness fixes: optimizer rules that changed a query's result
//! (multi-label `groupCount().by(T.label)`, filters reordered across side effects, duplicate ids
//! and labels folded into a source step, a fused source count mid-traversal) and mutating steps
//! that treated an empty upstream as a start step. The rule-by-rule differential check over a
//! larger corpus lives in `optimizer_differential_tests.rs`.
#![cfg(feature = "script")]

use std::sync::Arc;

use crate::script_host::eval_value;
use graphersal::prelude::*;

fn int(value: i64) -> Option<ElementProperty> {
    Some(ElementProperty::from(value))
}

/// The modern graph plus one vertex labeled `Person` + `Admin` and one labeled `Person`.
fn multi_label_graph() -> TraversalGraph {
    let mut graph = TraversalGraph::new();
    graph.traversal_mut().add_v("Person").next().unwrap();
    graph
        .traversal_mut()
        .add_v(vec!["Person", "Admin"])
        .next()
        .unwrap();
    graph
}

fn script(graph: TraversalGraph, script: &str) -> rhai::Dynamic {
    let graph: Arc<Graph> = Arc::new(Graph::new(graph));
    eval_value(graph, script).unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn modern_script(script: &str) -> rhai::Dynamic {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

// Bug 1: group_count_pushdown counted a multi-label vertex under every label.

#[test]
fn group_count_by_label_counts_a_multi_label_vertex_once_under_its_primary_label() {
    let graph = multi_label_graph();
    let optimized = graph
        .traversal()
        .v(None::<()>)
        .group_count()
        .by(By::Label)
        .to_list()
        .unwrap();
    let unoptimized = graph
        .traversal()
        .with("optimizer.disabled", vec!["group_count_pushdown"])
        .v(None::<()>)
        .group_count()
        .by(By::Label)
        .to_list()
        .unwrap();
    assert_eq!(optimized, unoptimized);
    let text = format!("{optimized:?}");
    assert!(text.contains("Person") && !text.contains("Admin"), "{text}");

    // The pushdown actually fired: this is the fused plan, not a fallback.
    let profile = graph
        .traversal()
        .v(None::<()>)
        .group_count()
        .by(By::Label)
        .profile()
        .unwrap()
        .to_string();
    assert!(profile.contains("group_count_pushdown"), "{profile}");
}

#[test]
fn group_count_by_label_rhai_both_spellings() {
    for q in [
        r#"g.V().groupCount().by(T.label).next()"#,
        r#"g.v().group_count().by(T.label).next()"#,
        r#"g.V().group().by(T.label).by(__.count()).next()"#,
    ] {
        let value = script(multi_label_graph(), q);
        let map = value.cast::<rhai::Map>();
        assert_eq!(map.len(), 1, "{q}: {map:?}");
        assert_eq!(map["Person"].as_int().unwrap(), 2, "{q}: {map:?}");
    }
}

#[test]
fn has_label_count_and_group_count_agree_with_the_unoptimized_plan() {
    // has_label matches ANY label of a vertex, and each vertex once.
    for q in [
        r#"g.V().hasLabel("Admin").count().next()"#,
        r#"g.V().has_label("Admin", "Person").count().next()"#,
        r#"g.V().hasLabel("Person", "Person").count().next()"#,
    ] {
        let optimized = script(multi_label_graph(), q).as_int().unwrap();
        let unoptimized = script(
            multi_label_graph(),
            &q.replacen("g.", r#"g.with("optimizer.disabled", ["all"])."#, 1),
        )
        .as_int()
        .unwrap();
        assert_eq!(optimized, unoptimized, "{q}");
    }
    let value = script(
        multi_label_graph(),
        r#"g.V().hasLabel("Admin").groupCount().by(T.label).next()"#,
    );
    let map = value.cast::<rhai::Map>();
    assert_eq!(map.len(), 1, "{map:?}");
    assert_eq!(map["Person"].as_int().unwrap(), 1, "{map:?}");
}

// Bug 2: filter_reorder moved a filter across a filter whose child writes a side effect.

#[test]
fn filters_are_not_reordered_across_a_side_effect_child() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let count = lock
        .traversal()
        .v(None::<()>)
        .filter_t(__::aggregate("x"))
        .has_label("software")
        .cap("x")
        .unfold()
        .count()
        .next()
        .unwrap();
    assert_eq!(count, int(6));
}

#[test]
fn filters_are_not_reordered_across_a_side_effect_child_rhai() {
    for q in [
        r#"g.V().filter(__.aggregate("x")).hasLabel("software").cap("x").unfold().count().next()"#,
        r#"g.v().filter(__.aggregate("x")).has_label("software").cap("x").unfold().count().next()"#,
        r#"g.V().where(__.sideEffect(__.store("x"))).hasId("1").cap("x").unfold().count().next()"#,
    ] {
        assert_eq!(modern_script(q).as_int().unwrap(), 6, "{q}");
    }
}

#[test]
fn filters_are_not_reordered_across_a_mutation() {
    // drop() is a filter: moving has_id before it would drop one vertex instead of two.
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let q = r#"g.V().hasLabel("software").drop().hasId("3").to_list(); g.V().count().next()"#;
    assert_eq!(eval_value(graph, q).unwrap().as_int().unwrap(), 4);
    // A mutating child runs for every traverser that reaches it.
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let q = r#"g.V().filter(__.property("seen", true)).hasId("1").to_list(); g.V().has("seen").count().next()"#;
    assert_eq!(eval_value(graph, q).unwrap().as_int().unwrap(), 6);
}

// Folding a filter's id/label list (a set) into a source step's list (a sequence).

#[test]
fn duplicate_has_id_and_has_label_arguments_pass_an_element_once() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let count = lock
        .traversal()
        .v(None::<()>)
        .has_id(vec!["1", "1"])
        .count()
        .next()
        .unwrap();
    assert_eq!(count, int(1));
    let count = lock
        .traversal()
        .e(None::<()>)
        .has_label(vec!["knows", "knows"])
        .count()
        .next()
        .unwrap();
    assert_eq!(count, int(2));
    for (q, expected) in [
        (r#"g.V().hasId("1", "1").count().next()"#, 1),
        (r#"g.v().has_id("1", "1").count().next()"#, 1),
        (r#"g.E().hasLabel("knows", "knows").count().next()"#, 2),
        // A source step's own id list is a sequence: V("1", "1") yields vertex 1 twice.
        (r#"g.V("1", "1").hasId("1").count().next()"#, 2),
    ] {
        assert_eq!(modern_script(q).as_int().unwrap(), expected, "{q}");
    }
}

// A fused source count mid-traversal counts one pass per incoming traverser.

#[test]
fn a_fused_count_in_a_batch_child_counts_every_incoming_traverser() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let counts = lock
        .traversal()
        .v(None::<()>)
        .has_label("software")
        .union(vec![__::v(None::<()>).count()])
        .to_list()
        .unwrap();
    assert_eq!(counts, vec![ElementProperty::from(12)]);
    let q = r#"g.V().hasLabel("software").union(__.V().hasLabel("person").count()).next()"#;
    assert_eq!(modern_script(q).as_int().unwrap(), 8);
}

// Bug 3: a mutating step after an upstream that yields nothing is not a start step.

#[test]
fn add_e_and_add_v_over_an_empty_upstream_yield_nothing() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let edges = graph
        .traversal_mut()
        .v(None::<()>)
        .has("name", "nobody")
        .as_("a")
        .add_e("x")
        .from("a")
        .to_list()
        .unwrap();
    assert!(edges.is_empty());
    let edges = graph
        .traversal_mut()
        .v(None::<()>)
        .has("name", "nobody")
        .add_e("x")
        .to_by(__::v("2"))
        .to_list()
        .unwrap();
    assert!(edges.is_empty());
    let vertices = graph
        .traversal_mut()
        .v(None::<()>)
        .has("name", "nobody")
        .add_v("ghost")
        .to_list()
        .unwrap();
    assert!(vertices.is_empty());
    assert_eq!(
        graph.traversal().v(None::<()>).count().next().unwrap(),
        int(6)
    );
    assert_eq!(
        graph.traversal().e(None::<()>).count().next().unwrap(),
        int(6)
    );
}

#[test]
fn add_e_and_add_v_over_an_empty_upstream_yield_nothing_rhai() {
    for q in [
        r#"g.V().has("name", "nobody").as("a").addE("x").from("a").to_list()"#,
        r#"g.v().has("name", "nobody").as_("a").add_e("x").from("a").to_list()"#,
        r#"g.V().has("name", "nobody").addE("x").to(__.V("2")).to_list()"#,
        r#"g.V().has("name", "nobody").addV("ghost").to_list()"#,
        r#"g.v().has("name", "nobody").add_v("ghost").to_list()"#,
        r#"g.inject().addV("ghost").to_list()"#,
        r#"g.V().has("name", "nobody").mergeV(#{name: "ghost"}).to_list()"#,
        r#"g.V().has("name", "nobody").property("x", 1).to_list()"#,
    ] {
        let graph = Arc::new(GraphSource::tinkerpop_modern());
        let result = eval_value(graph.clone(), q).unwrap_or_else(|e| panic!("{q}: {e}"));
        assert!(result.into_array().unwrap().is_empty(), "{q}");
        let state = eval_value(
            graph,
            "[g.V().count().next(), g.E().count().next(), g.V().properties().count().next()]",
        )
        .unwrap();
        assert_eq!(format!("{state:?}"), "[6, 6, 12]", "{q}");
    }
}

#[test]
fn add_v_and_add_e_as_start_steps_still_run_once() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let q = r#"g.addV("solo").to_list(); g.addE("x").from(__.V("1")).to(__.V("2")).to_list(); [g.V().count().next(), g.E().count().next()]"#;
    let state = eval_value(graph, q).unwrap();
    assert_eq!(format!("{state:?}"), "[7, 7]");
}

#[test]
fn start_step_creation_works_on_an_empty_graph() {
    // Rust API.
    let mut graph = TraversalGraph::new();
    let added = graph.traversal_mut().add_v("a").to_list().unwrap();
    assert_eq!(added.len(), 1);
    graph.traversal_mut().add_v("b").next().unwrap();
    let ids: Vec<String> = graph
        .traversal()
        .v(None::<()>)
        .id()
        .to_list()
        .unwrap()
        .into_iter()
        .map(|id| id.as_string().map(|s| s.to_string()).unwrap_or_default())
        .collect();
    let edge = graph
        .traversal_mut()
        .add_e("x")
        .from_by(__::v(ids[0].as_str()))
        .to_by(__::v(ids[1].as_str()))
        .to_list()
        .unwrap();
    assert_eq!(edge.len(), 1);
    let injected = graph
        .traversal_mut()
        .inject(vec![ElementProperty::from(1)])
        .add_v("c")
        .to_list()
        .unwrap();
    assert_eq!(injected.len(), 1);
    assert_eq!(
        graph.traversal().v(None::<()>).count().next().unwrap(),
        int(3)
    );
    assert_eq!(
        graph.traversal().e(None::<()>).count().next().unwrap(),
        int(1)
    );

    // Rhai, multi-statement scripts, both spellings.
    for script_text in [
        r#"g.addV("a").property("name", "x").to_list();
           g.addV("b").property("name", "y").to_list();
           g.addE("e").from(__.V().has("name", "x")).to(__.V().has("name", "y")).to_list();
           g.inject(1).addV("c").to_list();
           [g.V().count().next(), g.E().count().next()]"#,
        r#"g.add_v("a").property("name", "x").to_list();
           g.add_v("b").property("name", "y").to_list();
           g.add_e("e").from(__.v().has("name", "x")).to(__.v().has("name", "y")).to_list();
           g.inject(1).add_v("c").to_list();
           [g.v().count().next(), g.e().count().next()]"#,
    ] {
        let state = script(TraversalGraph::new(), script_text);
        assert_eq!(format!("{state:?}"), "[3, 1]", "{script_text}");
    }
}
