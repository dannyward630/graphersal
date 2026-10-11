//! Integration tests for where a `by()` modulator may go. Like TinkerPop, which accepts `by()`
//! only after a `ByModulating` step, a `by()` after a step that takes none (`out().by("name")`,
//! `count().by("x")`, `select(Column::Keys).by("x")`) or after no step at all fails with
//! `TraverserError::InvalidModulator` when the traversal executes, instead of being dropped.

use graphersal::prelude::AnonymousTraversal;
use graphersal::prelude::*;

/// Asserts that `err` is rooted in `InvalidModulator` for `step` at `position`, with `reason`.
fn assert_rejected(err: &TraverserError, step: &str, position: usize, reason: &str) {
    match err.root_cause() {
        TraverserError::InvalidModulator {
            step: actual_step,
            position: actual_position,
            reason: actual_reason,
            ..
        } => {
            assert_eq!(actual_step, step, "wrong step in: {err}");
            assert_eq!(*actual_position, position, "wrong position in: {err}");
            assert_eq!(actual_reason, reason, "wrong reason in: {err}");
        }
        other => panic!("expected InvalidModulator, got: {other}"),
    }
}

#[test]
fn a_by_after_a_step_that_takes_none_fails() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .v(None)
        .out(None)
        .by("name")
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take by() modulators");

    let err = lock
        .traversal()
        .v(None)
        .count()
        .by("x")
        .to_list()
        .expect_err("count");
    assert_rejected(&err, "count", 1, "count() does not take by() modulators");

    let err = lock
        .traversal()
        .v(None)
        .group_count()
        .by("name")
        .select(Column::Keys)
        .by("x")
        .to_list()
        .expect_err("select(Column)");
    assert_rejected(
        &err,
        "select",
        1,
        "select(Column.keys) does not take by() modulators",
    );

    let err = lock
        .traversal()
        .v(None)
        .values("name")
        .by("x")
        .to_list()
        .expect_err("values");
    assert_rejected(&err, "values", 1, "values() does not take by() modulators");

    let err = lock
        .traversal()
        .v(None)
        .values("age")
        .sum()
        .by("x")
        .to_list()
        .expect_err("sum");
    assert_rejected(&err, "sum", 1, "sum() does not take by() modulators");

    let err = lock.traversal().v(None).by("x").to_list().expect_err("v");
    assert_rejected(&err, "v", 1, "v() does not take by() modulators");
}

#[test]
fn a_folded_filter_still_rejects_its_by() {
    // `has_label()` is folded into `v()` by the optimizer; the rejected `by()` survives that.
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .has_label("person")
        .by("x")
        .to_list()
        .expect_err("has_label");
    assert_rejected(
        &err,
        "has_label",
        1,
        "has_label() does not take by() modulators",
    );
}

#[test]
fn the_first_of_several_rejected_by_is_reported() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .out(None)
        .by("name")
        .by("age")
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take by() modulators");
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
        .by("name")
        .to_list()
        .expect_err("no traverser reaches out(), the query is still invalid");
    assert_rejected(&err, "out", 1, "out() does not take by() modulators");
}

#[test]
fn a_rejected_by_fails_before_its_step_mutates_the_graph() {
    let graph = GraphSource::tinkerpop_modern();
    let err = graph
        .write()
        .traversal_mut()
        .add_v("thing")
        .by("name")
        .to_list()
        .expect_err("add_v");
    assert_rejected(&err, "add_v", 1, "add_v() does not take by() modulators");
    let lock = graph.read();
    let things = lock
        .traversal()
        .v(None)
        .has_label("thing")
        .count()
        .next()
        .expect("count");
    assert_eq!(things, Some(ElementProperty::Number(0.into())));
}

#[test]
fn a_by_in_a_child_traversal_is_checked_too() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .where_t(__::out(None).by("name"))
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take by() modulators");

    let err = lock
        .traversal()
        .v(None)
        .order()
        .by(__::out(None).by("name").count())
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take by() modulators");
}

#[test]
fn a_by_without_a_step_fails() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let no_step = "by() modulates the step before it, and there is none";

    let err = lock.traversal().by("x").to_list().expect_err("g.by()");
    assert_rejected(&err, "g", 1, no_step);

    let err = lock
        .traversal()
        .v(None)
        .where_t(AnonymousTraversal::new().by("x"))
        .to_list()
        .expect_err("__.by()");
    assert_rejected(&err, "__", 1, no_step);
}

#[test]
fn as_does_not_hide_the_step_it_labels() {
    // `as()` labels the step before it, so a following `by()` modulates that step, like
    // TinkerPop's `order().as("a").by("name")`.
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v(None)
        .has_label("person")
        .order()
        .as_("a")
        .by("name")
        .values("name")
        .to_list()
        .expect("order().as_().by()");
    let names: Vec<String> = names
        .iter()
        .map(|n| n.as_string().unwrap_or_default().to_string())
        .collect();
    assert_eq!(names, vec!["josh", "marko", "peter", "vadas"]);

    let err = lock
        .traversal()
        .v(None)
        .out(None)
        .as_("a")
        .by("name")
        .to_list()
        .expect_err("out");
    assert_rejected(&err, "out", 1, "out() does not take by() modulators");
}

#[test]
fn every_by_modulating_step_still_takes_by() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let g = || lock.traversal();
    let results = [
        g().v(None).order().by("name").to_list(),
        g().v(None).group().by(By::Label).by(By::Count).to_list(),
        g().v(None).group_count().by("name").to_list(),
        g().v(None).dedup().by(By::Label).to_list(),
        g().v(None)
            .as_("a")
            .out(None)
            .as_("b")
            .select(vec!["a", "b"])
            .by("name")
            .to_list(),
        g().v(None).project(vec!["n"]).by("name").to_list(),
        g().v("1").out(None).path().by("name").to_list(),
        g().v("1").out(None).tree(None::<()>).by("name").to_list(),
        g().v("1")
            .as_("a")
            .out("knows")
            .as_("b")
            .math("a + b")
            .by("age")
            .to_list(),
        g().v(None).value_map(None).by(__::unfold()).to_list(),
        g().v(None).aggregate("x").by("name").to_list(),
        g().v(None).store("x").by("name").to_list(),
    ];
    for result in results {
        let values = result.expect("a by()-modulating step takes by()");
        assert!(!values.is_empty());
    }
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

    #[test]
    fn a_by_after_a_step_that_takes_none_fails_in_both_spellings() {
        let cases = [
            (r#"g.V().out().by("name")"#, "out", "out()"),
            (r#"g.v().out().by("name")"#, "out", "out()"),
            (r#"g.V().count().by("x")"#, "count", "count()"),
            (r#"g.V().outE().by("weight")"#, "out_e", "out_e()"),
            (r#"g.v().out_e().by("weight")"#, "out_e", "out_e()"),
            (
                r#"g.V().hasLabel("person").by("x")"#,
                "has_label",
                "has_label()",
            ),
            (
                r#"g.v().has_label("person").by("x")"#,
                "has_label",
                "has_label()",
            ),
            (r#"g.V().values("name").by("x")"#, "values", "values()"),
            (r#"g.V().out().by("name", Order.desc)"#, "out", "out()"),
            (
                r#"g.V().groupCount().by("name").select(Column.keys).by("x")"#,
                "select",
                "select(Column.keys)",
            ),
            (r#"g.V().where(__.out().by("name"))"#, "out", "out()"),
        ];
        for (script, step, shown) in cases {
            let err = run(script).expect_err(script);
            assert_rejected(
                &err,
                step,
                1,
                &format!("{shown} does not take by() modulators"),
            );
        }
    }

    #[test]
    fn by_modulating_steps_still_take_by_in_both_spellings() {
        for script in [
            r#"g.V().order().by("name", Order.desc)"#,
            r#"g.V().groupCount().by("name")"#,
            r#"g.v().group_count().by("name")"#,
            r#"g.V().valueMap().by(__.unfold())"#,
            r#"g.v().value_map().by(__.unfold())"#,
            r#"g.V().as("a").out().as("b").select("a", "b").by("name")"#,
            r#"g.V().hasLabel("person").order().as("a").by("age").values("name")"#,
        ] {
            let values = run(script).unwrap_or_else(|e| panic!("`{script}`: {e}"));
            assert!(!values.is_empty(), "`{script}` returned nothing");
        }
    }
}
