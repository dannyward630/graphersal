//! Regressions for `TraversalGraph` contract violations, reproduced here through traversals, in
//! Rust and in Rhai. The first two were found by the storage conformance suite
//! (`crates/graphersal-storage-tests`):
//!
//! 1. `add_edge` to a dropped vertex panicked inside petgraph instead of returning an error.
//! 2. `drop_vertex` left the edge-id (and label) index entries of its incident edges. Because
//!    `StableGraph` reuses freed slots, a later `add_edge` made the stale id resolve to a
//!    DIFFERENT edge: silent wrong results.
//! 3. Element handles were plain slot indices, so a handle held across a drop (step label, side
//!    effect, path) resolved to the element that reused the slot later in the same query.
//!    Handles are generational now (`roadmap/transactions.md`, section 1a): a stale handle reads
//!    as removed (no properties, no edges), and `id()`/`label()`/materialization fail with
//!    `GraphError::ElementRemoved`.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::PropertyMap;
use graphersal::prelude::*;
use graphersal::storage::GraphNodeType;
use std::sync::Arc;

fn run(graph: &Arc<Graph>, script: &str) -> Result<rhai::Dynamic, String> {
    eval_value(graph.clone(), script).map_err(|err| err.to_string())
}

fn int(graph: &Arc<Graph>, script: &str) -> i64 {
    run(graph, script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .as_int()
        .expect("an integer")
}

/// a -e1-> b, a -e2-> c, all with explicit ids.
fn triangle() -> Arc<Graph> {
    let graph = Arc::new(GraphSource::empty());
    let _ = run(
        &graph,
        r#"
        g.addV("n").property(T.id, "a").toList();
        g.addV("n").property(T.id, "b").toList();
        g.addV("n").property(T.id, "c").toList();
        g.V("a").addE("l").property(T.id, "e1").to(__.V("b")).toList();
        g.V("a").addE("l").property(T.id, "e2").to(__.V("c")).toList();
        "#,
    )
    .expect("fixture");
    graph
}

#[test]
fn edge_id_lookup_after_dropping_an_endpoint_finds_nothing_in_rhai() {
    let graph = triangle();
    let _ = run(&graph, r#"g.V("b").drop().toList()"#).expect("drop");
    assert_eq!(
        int(&graph, r#"g.E("e1").count().next()"#),
        0,
        "e1 died with b"
    );
    assert_eq!(int(&graph, r#"g.E().count().next()"#), 1);
    assert_eq!(
        int(&graph, r#"g.E().hasLabel("l").count().next()"#),
        1,
        "label index entry removed"
    );
}

#[test]
fn a_reused_slot_does_not_resurrect_a_dropped_edge_id_in_rhai() {
    let graph = triangle();
    let _ = run(&graph, r#"g.V("b").drop().toList()"#).expect("drop");
    // The freed edge slot is reused by this new edge.
    let _ = run(
        &graph,
        r#"g.V("c").addE("m").property(T.id, "fresh").to(__.V("a")).toList()"#,
    )
    .expect("add");
    assert_eq!(
        int(&graph, r#"g.E("e1").count().next()"#),
        0,
        "the stale id e1 must not find the new edge"
    );
    assert_eq!(int(&graph, r#"g.E("fresh").count().next()"#), 1);
    assert_eq!(
        int(&graph, r#"g.E("fresh").hasLabel("m").count().next()"#),
        1
    );
    assert_eq!(
        int(&graph, r#"g.E().hasLabel("l").count().next()"#),
        1,
        "only e2 is left with label l"
    );
}

#[test]
fn the_same_through_the_rust_api() {
    let mut graph = TraversalGraph::new();
    let mk = |g: &mut TraversalGraph, id: &str| {
        g.add_vertex(Some(id.into()), vec!["n".into()], PropertyMap::default())
            .unwrap()
    };
    let (a, b, c) = (
        mk(&mut graph, "a"),
        mk(&mut graph, "b"),
        mk(&mut graph, "c"),
    );
    graph
        .add_edge(
            Some("e1".into()),
            Some("l".into()),
            a,
            b,
            PropertyMap::default(),
        )
        .unwrap();
    graph.traversal_mut().v("b").drop().to_list().unwrap();
    assert_eq!(
        graph
            .traversal()
            .e("e1")
            .count()
            .next()
            .unwrap()
            .and_then(|c| c.as_i64()),
        Some(0)
    );
    graph
        .add_edge(
            Some("fresh".into()),
            Some("m".into()),
            c,
            a,
            PropertyMap::default(),
        )
        .unwrap();
    assert_eq!(
        graph
            .traversal()
            .e("e1")
            .count()
            .next()
            .unwrap()
            .and_then(|c| c.as_i64()),
        Some(0)
    );
    assert_eq!(
        graph
            .traversal()
            .e("fresh")
            .count()
            .next()
            .unwrap()
            .and_then(|c| c.as_i64()),
        Some(1)
    );
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn add_edge_to_a_vertex_dropped_earlier_in_the_traversal_fails_cleanly_in_rhai() {
    let graph = triangle();
    // `b` is dropped by a side effect, then the same traversal adds an edge to its stale handle.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(
            &graph,
            r#"g.V("a").as("x").V("b").as("y").sideEffect(__.select("y").drop()).addE("l").from("x").to("y").toList()"#,
        )
    }));
    let outcome = outcome.expect("add_edge to a dropped vertex must not panic");
    let report = outcome.expect_err("the engine reports the removed endpoint");
    assert!(report.contains("removed earlier in this query"), "{report}");
    // The failing traversal is rolled back: `b` and its edge `e1` are back.
    assert_eq!(int(&graph, r#"g.V("b").count().next()"#), 1);
    assert_eq!(int(&graph, r#"g.E("e1").count().next()"#), 1);
}

#[test]
fn add_edge_to_a_stale_handle_is_an_error_in_the_rust_api() {
    let mut graph = TraversalGraph::new();
    let a = graph
        .add_vertex(Some("a".into()), vec![], PropertyMap::default())
        .unwrap();
    let b = graph
        .add_vertex(Some("b".into()), vec![], PropertyMap::default())
        .unwrap();
    graph.drop_vertex(b).unwrap();
    for (from, to) in [(a, b), (b, a), (b, b)] {
        let err = graph
            .add_edge(None, Some("l".into()), from, to, PropertyMap::default())
            .expect_err("a dropped endpoint must be rejected");
        assert!(
            matches!(
                err,
                GraphError::ElementRemoved {
                    kind: GraphNodeType::Vertex,
                    ..
                }
            ),
            "{err:?}"
        );
    }
    assert_eq!(graph.edge_count(), 0);
    // The graph is still usable.
    let c = graph
        .add_vertex(Some("c".into()), vec![], PropertyMap::default())
        .unwrap();
    graph
        .add_edge(None, Some("l".into()), a, c, PropertyMap::default())
        .unwrap();
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn the_error_report_names_the_vertex_and_helps() {
    let mut graph = TraversalGraph::new();
    let a = graph
        .add_vertex(Some("a".into()), vec![], PropertyMap::default())
        .unwrap();
    let b = graph
        .add_vertex(Some("b".into()), vec![], PropertyMap::default())
        .unwrap();
    graph.drop_vertex(b).unwrap();
    let err = graph
        .add_edge(None, None, a, b, PropertyMap::default())
        .expect_err("rejected");
    let text = format_diagnostic(&err);
    assert!(
        text.contains("The vertex was removed earlier in this query"),
        "{text}"
    );
    assert!(
        text.contains("dropped"),
        "the help explains stale handles: {text}"
    );
}

// ---------------------------------------------------------------------------------------------
// Generational handles: a handle held across a drop never reaches the slot's next occupant.
// ---------------------------------------------------------------------------------------------

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

/// Runs `script` against a fresh modern graph and returns the rendered error report.
fn rhai_error(script: &str) -> String {
    match run(&modern(), script) {
        Ok(value) => panic!("{script}: expected an error, got {value:?}"),
        Err(report) => report,
    }
}

/// Runs `script` against a fresh modern graph and returns the result list.
fn rhai_list(script: &str) -> rhai::Array {
    run(&modern(), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .into_array()
        .unwrap_or_else(|t| panic!("{script}: not a list but {t}"))
}

/// `true` iff `err` is (or wraps) `GraphError::ElementRemoved` of `kind`.
fn is_element_removed(err: &TraverserError, kind: GraphNodeType) -> bool {
    let text = format!("{err:?}");
    text.contains("ElementRemoved") && text.contains(&format!("{kind:?}"))
}

#[test]
fn a_labelled_vertex_dropped_earlier_does_not_read_the_vertex_reusing_its_slot() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let names = graph
        .traversal_mut()
        .v("1")
        .as_("a")
        .side_effect_t(__::drop())
        .add_v("ghost")
        .property("name", "GHOST")
        .select("a")
        .values("name")
        .to_list()
        .expect("runs");
    assert!(
        names.is_empty(),
        "a removed vertex has no properties: {names:?}"
    );
    // The new vertex exists and kept its own data.
    assert_eq!(
        graph
            .traversal()
            .v(None)
            .has_label("ghost")
            .values("name")
            .to_list()
            .unwrap(),
        vec![ElementProperty::String("GHOST".into())]
    );
    for script in [
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").property("name","GHOST").select("a").values("name").toList()"#,
        r#"g.v("1").as_("a").side_effect(__.drop()).add_v("ghost").property("name","GHOST").select("a").values("name").to_list()"#,
    ] {
        assert!(rhai_list(script).is_empty(), "{script}");
    }
}

#[test]
fn an_aggregated_vertex_dropped_earlier_does_not_read_the_vertex_reusing_its_slot() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let names = graph
        .traversal_mut()
        .v("1")
        .aggregate("x")
        .side_effect_t(__::drop())
        .add_v("ghost")
        .property("name", "GHOST")
        .cap("x")
        .unfold()
        .values("name")
        .to_list()
        .expect("runs");
    assert!(names.is_empty(), "{names:?}");
    for script in [
        r#"g.V("1").aggregate("x").sideEffect(__.drop()).addV("ghost").property("name","GHOST").cap("x").unfold().values("name").toList()"#,
        r#"g.v("1").aggregate("x").side_effect(__.drop()).add_v("ghost").property("name","GHOST").cap("x").unfold().values("name").to_list()"#,
    ] {
        assert!(rhai_list(script).is_empty(), "{script}");
    }
    // Materializing the removed vertex itself fails instead of returning another vertex.
    let report = rhai_error(
        r#"g.V("1").aggregate("x").sideEffect(__.drop()).addV("ghost").cap("x").unfold().toList()"#,
    );
    assert!(
        report.contains("The vertex was removed earlier"),
        "{report}"
    );
}

#[test]
fn the_label_of_a_dropped_edge_is_an_error_not_the_label_of_the_edge_reusing_its_slot() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let err = graph
        .traversal_mut()
        .v("1")
        .out_e(None)
        .as_("e")
        .side_effect_t(__::drop())
        .v("2")
        .add_e("x")
        .to_by(__::v("3"))
        .select("e")
        .label()
        .to_list()
        .expect_err("label() of a removed edge");
    assert!(is_element_removed(&err, GraphNodeType::Edge), "{err:?}");
    for script in [
        r#"g.V("1").outE().as("e").sideEffect(__.drop()).V("2").addE("x").to(__.V("3")).select("e").label().toList()"#,
        r#"g.v("1").out_e().as_("e").side_effect(__.drop()).v("2").add_e("x").to(__.v("3")).select("e").label().to_list()"#,
    ] {
        let report = rhai_error(script);
        assert!(report.contains("The edge was removed earlier"), "{report}");
    }
}

#[test]
fn edge_properties_of_a_dropped_edge_are_absent() {
    for script in [
        r#"g.V("1").outE().as("e").sideEffect(__.drop()).V("2").addE("x").property("weight", 9.5).to(__.V("3")).select("e").values("weight").toList()"#,
        r#"g.v("1").out_e().as_("e").side_effect(__.drop()).v("2").add_e("x").property("weight", 9.5).to(__.v("3")).select("e").values("weight").to_list()"#,
        r#"g.V("1").outE().as("e").sideEffect(__.drop()).V("2").addE("x").property("weight", 9.5).to(__.V("3")).select("e").properties().toList()"#,
        r#"g.V("1").outE().as("e").sideEffect(__.drop()).V("2").addE("x").property("weight", 9.5).to(__.V("3")).select("e").inV().toList()"#,
    ] {
        assert!(rhai_list(script).is_empty(), "{script}");
    }
    // The three new edges exist, with their own weight.
    let graph = modern();
    let _ = run(
        &graph,
        r#"g.V("1").outE().as("e").sideEffect(__.drop()).V("2").addE("x").property("weight", 9.5).to(__.V("3")).toList()"#,
    )
    .expect("runs");
    assert_eq!(
        int(
            &graph,
            r#"g.E().hasLabel("x").has("weight", 9.5).count().next()"#
        ),
        3
    );
    assert_eq!(int(&graph, r#"g.V("1").outE().count().next()"#), 0);
}

#[test]
fn a_sack_never_reads_the_vertex_reusing_a_dropped_slot() {
    // Before generational handles the sack summed the ghost's age (1 + 99).
    for script in [
        r#"g.withSack(1).V("1").as("a").sideEffect(__.drop()).addV("ghost").property("age", 99).select("a").sack(Operator.sum).by("age").sack().toList()"#,
        r#"g.with_sack(1).v("1").as_("a").side_effect(__.drop()).add_v("ghost").property("age", 99).select("a").sack(Operator.sum).by("age").sack().to_list()"#,
    ] {
        let sacks = rhai_list(script);
        assert!(
            sacks.iter().all(|s| s.as_int().ok() != Some(100)),
            "{script}: {sacks:?}"
        );
    }
}

#[test]
fn writes_and_ids_through_a_stale_handle_fail_and_touch_nothing() {
    for script in [
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").property("name", "X").toList()"#,
        r#"g.v("1").as_("a").side_effect(__.drop()).add_v("ghost").select("a").id().to_list()"#,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").label().toList()"#,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").elementMap().toList()"#,
    ] {
        let report = rhai_error(script);
        assert!(
            report.contains("The vertex was removed earlier"),
            "{script}: {report}"
        );
    }
    // The failed write did not reach the ghost that reused the slot.
    let graph = modern();
    let _ = run(
        &graph,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").property("name", "X").toList()"#,
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V().hasLabel("ghost").has("name").count().next()"#
        ),
        0
    );
    // Filters drop a removed element; a dropped vertex has no edges.
    for script in [
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").hasLabel("ghost").toList()"#,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").hasId("1").toList()"#,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").has(T.id, "1").toList()"#,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").out().toList()"#,
        r#"g.V("1").as("a").sideEffect(__.drop()).addV("ghost").select("a").bothE().toList()"#,
    ] {
        assert!(rhai_list(script).is_empty(), "{script}");
    }
}
