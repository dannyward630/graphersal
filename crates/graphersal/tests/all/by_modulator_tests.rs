//! Integration tests for the `by([..])` vertex-label-list modulator (`By::Labels`): no existing
//! `by()` consumer can use it, so every one of them fails with
//! `TraverserError::InvalidModulator` when it executes, in the Rust API and in Rhai alike.

use graphersal::prelude::Order;
use graphersal::prelude::*;

/// Asserts that `err` is rooted in `InvalidModulator` for `step` at `position`.
fn assert_invalid_modulator(err: &TraverserError, step: &str, position: usize) {
    match err.root_cause() {
        TraverserError::InvalidModulator {
            step: actual_step,
            position: actual_position,
            ..
        } => {
            assert_eq!(actual_step, step, "wrong step in: {err}");
            assert_eq!(*actual_position, position, "wrong position in: {err}");
        }
        other => panic!("expected InvalidModulator, got: {other}"),
    }
}

#[test]
fn order_rejects_a_label_list() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .order()
        .by(["a"])
        .to_list()
        .expect_err("order() cannot sort by a label list");
    assert_invalid_modulator(&err, "order", 1);
}

#[test]
fn order_reports_the_position_of_the_offending_by() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .order()
        .by("name")
        .by(Order::Desc)
        .by(vec!["a", "b"])
        .to_list()
        .expect_err("order() cannot sort by a label list");
    assert_invalid_modulator(&err, "order", 3);
}

#[test]
fn group_rejects_a_label_list() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .group()
        .by(["a"])
        .to_list()
        .expect_err("group() cannot key by a label list");
    assert_invalid_modulator(&err, "group", 1);
}

#[test]
fn group_rejects_a_label_list_as_its_value_projection() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .group()
        .by("name")
        .by(vec!["a".to_string()])
        .to_list()
        .expect_err("group() cannot project values by a label list");
    assert_invalid_modulator(&err, "group", 2);
}

#[test]
fn group_count_by_label_does_not_bypass_the_rejection() {
    // `groupCount().by(label)` is normally computed by the `group_count_pushdown` rule; a
    // rejected `by()` must keep it from firing.
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .group_count()
        .by(By::Label)
        .by(["a"])
        .to_list()
        .expect_err("group_count() cannot use a label list");
    assert_invalid_modulator(&err, "group_count", 2);
}

#[test]
fn path_rejects_a_label_list() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .out(None)
        .path()
        .by(["a"])
        .to_list()
        .expect_err("path() cannot project by a label list");
    assert_invalid_modulator(&err, "path", 1);
}

#[test]
fn project_rejects_a_label_list() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .project("x")
        .by(["a"])
        .to_list()
        .expect_err("project() cannot project by a label list");
    assert_invalid_modulator(&err, "project", 1);
}

#[test]
fn the_remaining_by_consumers_reject_a_label_list() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .v(None)
        .as_("x")
        .select("x")
        .by(["a"])
        .to_list()
        .expect_err("select");
    assert_invalid_modulator(&err, "select", 1);

    let err = lock
        .traversal()
        .v(None)
        .store("x")
        .by(["a"])
        .to_list()
        .expect_err("store");
    assert_invalid_modulator(&err, "store", 1);

    let err = lock
        .traversal()
        .v(None)
        .aggregate("x")
        .by(["a"])
        .to_list()
        .expect_err("aggregate");
    assert_invalid_modulator(&err, "aggregate", 1);

    let err = lock
        .traversal()
        .v(None)
        .tree(None::<()>)
        .by(["a"])
        .to_list()
        .expect_err("tree");
    assert_invalid_modulator(&err, "tree", 1);

    let err = lock
        .traversal()
        .v(None)
        .dedup()
        .by(["a"])
        .to_list()
        .expect_err("dedup");
    assert_invalid_modulator(&err, "dedup", 1);

    let err = lock
        .traversal()
        .v(None)
        .value_map(None)
        .by(["a"])
        .to_list()
        .expect_err("value_map");
    assert_invalid_modulator(&err, "value_map", 1);
}

#[cfg(feature = "script")]
mod rhai {
    use std::sync::Arc;

    use super::assert_invalid_modulator;
    use graphersal::prelude::*;

    /// Runs a Rhai query that ends on a traversal and returns its materialized results.
    fn run(script: &str) -> TraverserResult<Vec<ElementProperty>> {
        let graph = Arc::new(GraphSource::tinkerpop_modern());
        let engine = crate::script_host::engine();
        let mut scope = crate::script_host::graph_scope(graph.clone());
        let gts = engine
            .eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(&mut scope, script)
            .unwrap_or_else(|e| panic!("`{script}` failed to evaluate: {e}"));
        let lock = graph.read();
        gts.ast_convert_to_gts(&*lock).to_list()
    }

    #[test]
    fn every_by_consumer_rejects_a_label_list() {
        let cases = [
            (r#"g.V().order().by(["a"])"#, "order", 1),
            (
                r#"g.v().order().by("name", Order.desc).by(["a"])"#,
                "order",
                3,
            ),
            (r#"g.V().group().by(["a"])"#, "group", 1),
            (r#"g.v().group().by("name").by(["a", "b"])"#, "group", 2),
            (r#"g.V().groupCount().by(["a"])"#, "group_count", 1),
            (r#"g.V().out().path().by(["a"])"#, "path", 1),
            (r#"g.V().project("x").by(["a"])"#, "project", 1),
            (r#"g.V().as("x").select("x").by(["a"])"#, "select", 1),
            (r#"g.V().store("x").by(["a"])"#, "store", 1),
            (r#"g.V().aggregate("x").by(["a"])"#, "aggregate", 1),
            (r#"g.V().tree().by(["a"])"#, "tree", 1),
            (r#"g.V().dedup().by(["a"])"#, "dedup", 1),
            (r#"g.V().valueMap().by(["a"])"#, "value_map", 1),
            (
                r#"g.v().value_map().by(__.unfold()).by(["a"])"#,
                "value_map",
                2,
            ),
        ];
        for (script, step, position) in cases {
            let err = run(script).expect_err(script);
            assert_invalid_modulator(&err, step, position);
        }
    }

    #[test]
    fn an_empty_label_list_is_accepted_by_the_binding() {
        // Emptiness is for the consuming step to judge; the binding only converts.
        let err = run(r#"g.V().order().by([])"#).expect_err("order() still rejects it");
        assert_invalid_modulator(&err, "order", 1);
    }

    #[test]
    fn a_non_string_label_is_an_argument_mismatch() {
        let graph = Arc::new(GraphSource::tinkerpop_modern());
        let err = crate::script_host::eval_value(graph, r#"g.V().order().by(["a", 1]).toList()"#)
            .expect_err("a label list holds strings only")
            .to_string();
        assert!(
            err.contains(
                "Argument mismatch for step 'by': expected a list of vertex-label strings, \
                 got a list with an element of type int64"
            ),
            "unexpected error: {err}"
        );
    }
}
