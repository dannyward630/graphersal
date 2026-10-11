//! Integration tests for where a `from()`/`to()` modulator may go. Like TinkerPop, which accepts
//! `from()`/`to()` only after a `FromToModulating` step (`addE()`, and `path()`/`simplePath()`/
//! `cyclicPath()`, which this crate does not implement — see the ROADMAP), a `from()`/`to()`
//! after a step that takes none (`out().from("a")`), or after no step at all, fails with
//! `TraverserError::InvalidModulator` when the traversal executes, instead of being dropped —
//! and, before that, a plan-time validation pass finds the same rejection even inside a branch
//! that never runs (TinkerPop rejects such queries at build time).

use graphersal::prelude::AnonymousTraversal;
use graphersal::prelude::*;

/// Asserts that `err` is rooted in `InvalidModulator` for `step` at `position`, with `reason`,
/// and that the modulator kind shown is `"from()/to()"`.
fn assert_rejected(err: &TraverserError, step: &str, position: usize, reason: &str) {
    match err.root_cause() {
        TraverserError::InvalidModulator {
            step: actual_step,
            position: actual_position,
            modulator,
            reason: actual_reason,
            ..
        } => {
            assert_eq!(actual_step, step, "wrong step in: {err}");
            assert_eq!(*actual_position, position, "wrong position in: {err}");
            assert_eq!(*modulator, "from()/to()", "wrong modulator kind in: {err}");
            assert_eq!(actual_reason, reason, "wrong reason in: {err}");
        }
        other => panic!("expected InvalidModulator, got: {other}"),
    }
}

#[test]
fn a_from_after_a_step_that_takes_none_fails() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .v(None)
        .out(None)
        .from("1")
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take from()/to() modulators");

    let err = lock
        .traversal()
        .v(None)
        .count()
        .to("1")
        .to_list()
        .expect_err("count");
    assert_rejected(
        &err,
        "count",
        1,
        "count() does not take from()/to() modulators",
    );

    let err = lock.traversal().v(None).from("1").to_list().expect_err("v");
    assert_rejected(&err, "v", 1, "v() does not take from()/to() modulators");
}

#[test]
fn a_to_after_a_step_that_takes_none_fails() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .v(None)
        .values("name")
        .to("1")
        .to_list()
        .expect_err("values");
    assert_rejected(
        &err,
        "values",
        1,
        "values() does not take from()/to() modulators",
    );
}

#[test]
fn a_folded_filter_still_rejects_its_from() {
    // `has_label()` is folded into `v()` by the optimizer; the rejected `from()` survives that.
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .has_label("person")
        .from("1")
        .to_list()
        .expect_err("has_label");
    assert_rejected(
        &err,
        "has_label",
        1,
        "has_label() does not take from()/to() modulators",
    );
}

#[test]
fn an_empty_stream_does_not_hide_the_rejection() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .has("name", "nobody")
        .out(None)
        .from("1")
        .to_list()
        .expect_err("no traverser reaches out(), the query is still invalid");
    assert_rejected(&err, "out", 1, "out() does not take from()/to() modulators");
}

/// A rejected `from()`/`to()` inside a child traversal that never executes still fails, like
/// TinkerPop, which rejects at build time (design spec task 8's second bullet).
#[test]
fn a_from_in_a_never_executed_child_traversal_still_fails() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .has("name", "nobody")
        .where_t(__::out(None).from("1"))
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take from()/to() modulators");
}

#[test]
fn a_from_without_a_step_fails() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let no_step = "from()/to() modulates the step before it, and there is none";

    let err = lock.traversal().from("1").to_list().expect_err("g.from()");
    assert_rejected(&err, "g", 1, no_step);

    let err = lock
        .traversal()
        .v(None)
        .where_t(AnonymousTraversal::new().to("1"))
        .to_list()
        .expect_err("__.to()");
    assert_rejected(&err, "__", 1, no_step);
}

#[test]
fn add_e_still_takes_from_and_to() {
    let mut graph = TraversalGraph::new();
    let n1 = graph
        .traversal_mut()
        .add_v("thing")
        .next()
        .unwrap()
        .unwrap();
    let n2 = graph
        .traversal_mut()
        .add_v("thing")
        .next()
        .unwrap()
        .unwrap();
    let result = graph
        .traversal_mut()
        .add_e("knows")
        .from(n1)
        .to(n2)
        .next()
        .expect("add_e().from().to() works");
    assert!(result.is_some());
}

#[cfg(feature = "script")]
mod rhai {
    use std::sync::Arc;

    use super::assert_rejected;
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

    /// Like [`run`], but against a fresh, empty, mutable graph (for `add_e()`/`add_v()`).
    fn run_mut(script: &str) -> TraverserResult<Vec<ElementProperty>> {
        let graph = Arc::new(GraphSource::empty());
        let engine = crate::script_host::engine();
        let mut scope = crate::script_host::graph_scope(graph.clone());
        let gts = engine
            .eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(&mut scope, script)
            .unwrap_or_else(|e| panic!("`{script}` failed to evaluate: {e}"));
        let mut lock = graph.write();
        gts.ast_convert_to_gts(&mut *lock).to_list()
    }

    #[test]
    fn a_from_to_after_a_step_that_takes_none_fails_in_both_spellings() {
        let cases = [
            (r#"g.V().out().from("1")"#, "out", "out()"),
            (r#"g.v().out().to("1")"#, "out", "out()"),
            (r#"g.V().count().from("1")"#, "count", "count()"),
            (r#"g.V().where(__.out().from("1"))"#, "out", "out()"),
        ];
        for (script, step, shown) in cases {
            let err = run(script).expect_err(script);
            assert_rejected(
                &err,
                step,
                1,
                &format!("{shown} does not take from()/to() modulators"),
            );
        }
    }

    #[test]
    fn add_e_still_takes_from_to_in_both_spellings() {
        for script in [
            r#"g.addV("person").as("a").addV("person").as("b").addE("knows").from("a").to("b")"#,
            r#"g.add_v("person").as_("a").add_v("person").as_("b").add_e("knows").from("a").to("b")"#,
        ] {
            let values = run_mut(script).unwrap_or_else(|e| panic!("`{script}`: {e}"));
            assert!(!values.is_empty(), "`{script}` returned nothing");
        }
    }
}

/// `from()`/`to()` inside a child traversal (`__.addE(..).from(..)`), with step labels of the
/// parent path, vertex ids and child traversals, and their rendering in `.profile()`.
mod anonymous {
    use super::*;

    fn edges(graph: &TraversalGraph, label: &str) -> Vec<(String, String)> {
        let lock = graph.traversal();
        let mut pairs: Vec<(String, String)> = lock
            .e(None::<()>)
            .has_label(label)
            .project(vec!["f", "t"])
            .by(__::out_v().id())
            .by(__::in_v().id())
            .to_list()
            .unwrap()
            .into_iter()
            .map(|m| {
                let m = m.as_object().unwrap().clone();
                (
                    m.get("f").unwrap().as_str().unwrap().to_string(),
                    m.get("t").unwrap().as_str().unwrap().to_string(),
                )
            })
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn labels_and_ids_from_the_parent_path() {
        let mut graph = TraversalGraph::tinkerpop_modern();
        graph
            .traversal_mut()
            .v("1")
            .as_("a")
            .out("knows")
            .side_effect_t(__::add_e("likes").from("a"))
            .to_list()
            .unwrap();
        assert_eq!(
            edges(&graph, "likes"),
            vec![("1".into(), "2".into()), ("1".into(), "4".into())]
        );
        graph
            .traversal_mut()
            .v("6")
            .side_effect_t(__::add_e("hates").to("3"))
            .to_list()
            .unwrap();
        assert_eq!(edges(&graph, "hates"), vec![("6".into(), "3".into())]);
    }

    #[test]
    fn child_traversals_pick_the_endpoints() {
        let mut graph = TraversalGraph::tinkerpop_modern();
        graph
            .traversal_mut()
            .v("4")
            .side_effect_t(
                __::add_e("rates")
                    .from_by(__::v("5"))
                    .to_by(__::out("created").has("name", "lop")),
            )
            .to_list()
            .unwrap();
        assert_eq!(edges(&graph, "rates"), vec![("5".into(), "3".into())]);
        // the traversal form at the top level, with to() taking the incoming vertex's neighbour
        graph
            .traversal_mut()
            .v("2")
            .add_e("mentors")
            .from_by(__::v("6"))
            .to_by(__::identity())
            .to_list()
            .unwrap();
        assert_eq!(edges(&graph, "mentors"), vec![("6".into(), "2".into())]);
    }

    #[test]
    fn a_from_traversal_without_a_result_fails() {
        let mut graph = TraversalGraph::tinkerpop_modern();
        let err = graph
            .traversal_mut()
            .v("1")
            .add_e("x")
            .from_by(__::out("nope"))
            .to("2")
            .to_list()
            .expect_err("no endpoint");
        assert!(
            format_diagnostic(&*err).contains("from"),
            "{}",
            format_diagnostic(&*err)
        );
        assert_eq!(graph.edge_count(), 6, "nothing added");
    }

    #[test]
    fn profile_shows_the_endpoints() {
        let graph = GraphSource::tinkerpop_modern();
        let mut lock = graph.write();
        let text = lock
            .traversal_mut()
            .v("1")
            .as_("a")
            .add_e("x")
            .from("a")
            .to_by(__::v("2"))
            .profile()
            .unwrap()
            .to_string();
        assert!(text.contains("add_e"), "{text}");
    }
}
