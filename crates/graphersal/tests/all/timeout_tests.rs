//! Integration tests for the per-execution `evaluationTimeout` option: cooperative
//! cancellation, nested traversals, option validation, mutation semantics and the script hosts.
//!
//! Timing-dependent assertions check the error variant, never how long the query ran.

use graphersal::prelude::*;

/// A fan-out that runs for tens of milliseconds on the `large` graph.
fn out3_count(
    graph: &Graph,
    timeout: Option<ElementProperty>,
) -> TraverserResult<Option<ElementProperty>> {
    let lock = graph.read();
    let mut source = lock.traversal();
    if let Some(timeout) = timeout {
        source = source.with("evaluationTimeout", timeout);
    }
    source.v(None).out(None).out(None).out(None).count().next()
}

// T1
#[test]
fn a_fan_out_past_its_budget_times_out() {
    let graph = GraphSource::large_110k();
    let err =
        out3_count(&graph, Some(1.into())).expect_err("1 ms is not enough for 330k expansions");
    assert!(
        matches!(
            err.root_cause(),
            TraverserError::Timeout { limit_ms: 1, .. }
        ),
        "expected a timeout, got {err:?}"
    );
    // The failure names the step that was running when the budget ran out.
    assert!(
        matches!(*err, TraverserError::StepFailed { .. }),
        "got {err:?}"
    );
    let report = format_diagnostic(&err);
    assert!(
        report.contains("Traversal exceeded evaluationTimeout of 1 ms"),
        "report: {report}"
    );
}

// T2
#[test]
fn no_timeout_or_zero_runs_to_completion() {
    let graph = GraphSource::tinkerpop_modern();
    let expected = out3_count(&graph, None).unwrap();
    assert_eq!(out3_count(&graph, Some(0.into())).unwrap(), expected);
    assert_eq!(out3_count(&graph, Some(60_000.into())).unwrap(), expected);
}

#[test]
fn a_generous_budget_does_not_change_the_result() {
    let graph = GraphSource::large_110k();
    let expected = out3_count(&graph, None).unwrap();
    assert_eq!(out3_count(&graph, Some(600_000.into())).unwrap(), expected);
}

// T3
#[test]
fn a_timeout_inside_a_nested_traversal_surfaces_as_timeout() {
    let graph = GraphSource::large_110k();
    let lock = graph.read();
    let err = lock
        .traversal()
        .with("evaluationTimeout", 1)
        .v(None)
        .where_t(__::out(None).out(None).out(None))
        .count()
        .next()
        .expect_err("the where() scan exceeds 1 ms");
    assert!(
        matches!(err.root_cause(), TraverserError::Timeout { .. }),
        "expected a timeout, got {err:?}"
    );
}

#[test]
fn a_nested_timeout_is_wrapped_with_the_enclosing_where_step() {
    let graph = GraphSource::large_110k();
    let lock = graph.read();
    // A single start vertex whose where() explores ~5M paths (hundreds of milliseconds): the
    // budget can only run out inside the nested traversal. `lazy_barrier` is off: merged, this
    // exploration would finish within the budget.
    let err = lock
        .traversal()
        .with("evaluationTimeout", 50)
        .with("optimizer.disabled", vec!["lazy_barrier"])
        .v("V_0_0")
        .where_t(
            __::both(None)
                .both(None)
                .both(None)
                .both(None)
                .both(None)
                .both(None)
                .both(None)
                .has("level", "none"),
        )
        .count()
        .next()
        .expect_err("the nested traversal exceeds 50 ms");
    assert!(
        matches!(err.root_cause(), TraverserError::Timeout { .. }),
        "expected a timeout, got {err:?}"
    );
    match &*err {
        TraverserError::StepFailed {
            step_name,
            step_index,
            ..
        } => {
            assert!(step_name.starts_with("where"), "outer step: {step_name}");
            assert_eq!(*step_index, Some(1));
        }
        other => panic!("expected a step failure, got {other:?}"),
    }
    let report = format_diagnostic(&err);
    assert!(report.contains("Step #1."), "report: {report}");
}

// T4
#[test]
fn invalid_timeout_values_fail_at_execution() {
    let graph = GraphSource::tinkerpop_modern();
    for value in [
        ElementProperty::from(-1),
        ElementProperty::String("abc".into()),
        ElementProperty::Number(graphersal::prelude::Number::from_f64(1.5)),
    ] {
        let lock = graph.read();
        // with() itself cannot fail: the source is built without error.
        let source = lock
            .traversal()
            .with("evaluationTimeout", value.clone())
            .v(None)
            .count();
        let err = source
            .next()
            .expect_err("an invalid value is rejected when executed");
        match &*err {
            TraverserError::InvalidOption { key, .. } => assert_eq!(key, "evaluationTimeout"),
            other => panic!("expected InvalidOption for {value:?}, got {other:?}"),
        }
    }
}

#[test]
fn unknown_options_are_ignored() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let count = lock
        .traversal()
        .with("someUnknownOption", "x")
        .v(None)
        .count()
        .next()
        .unwrap();
    assert_eq!(count, Some(6.into()));
}

#[test]
fn the_last_value_of_a_key_wins() {
    let graph = GraphSource::large_110k();
    let lock = graph.read();
    let result = lock
        .traversal()
        .with("evaluationTimeout", 1)
        .with("evaluationTimeout", 0)
        .v(None)
        .out(None)
        .out(None)
        .out(None)
        .count()
        .next();
    assert!(result.is_ok(), "{result:?}");
}

// T5
#[test]
fn a_mutating_step_is_applied_completely_or_not_at_all() {
    let graph = GraphSource::large_110k();
    let total = {
        let lock = graph.read();
        lock.traversal().v(None).count().next().unwrap()
    };
    let err = {
        let mut lock = graph.write();
        lock.traversal_mut()
            .with("evaluationTimeout", 1)
            .v(None)
            .property("x", 1)
            .out(None)
            .out(None)
            .out(None)
            .count()
            .next()
            .expect_err("the traversal exceeds 1 ms")
    };
    assert!(
        matches!(err.root_cause(), TraverserError::Timeout { .. }),
        "expected a timeout, got {err:?}"
    );
    let marked = {
        let lock = graph.read();
        lock.traversal()
            .v(None)
            .has_key("x")
            .count()
            .next()
            .unwrap()
    };
    assert!(
        marked == Some(0.into()) || marked == total,
        "property() was interrupted half-way: {marked:?} of {total:?} vertices marked"
    );
}

#[test]
fn mutations_before_the_timed_out_step_are_rolled_back() {
    let graph = GraphSource::large_110k();
    // The start set is a single id lookup, so the budget cannot run out before property(); the
    // where() step behind it explores ~5M paths, which takes far longer than the budget.
    let err = {
        let mut lock = graph.write();
        lock.traversal_mut()
            .with("evaluationTimeout", 50)
            .v("V_0_0")
            .property("x", 1)
            .where_t(
                __::both(None)
                    .both(None)
                    .both(None)
                    .both(None)
                    .both(None)
                    .both(None)
                    .both(None)
                    .has("level", "none"),
            )
            .count()
            .next()
            .expect_err("the where() step exceeds 50 ms")
    };
    assert!(
        matches!(err.root_cause(), TraverserError::Timeout { .. }),
        "{err:?}"
    );
    // Implicit auto-commit: the timeout rolls back the property() write before it.
    let lock = graph.read();
    let marked = lock.traversal().v("V_0_0").values("x").next().unwrap();
    assert_eq!(marked, None);
}

// D7
#[test]
fn profile_honours_the_timeout() {
    let graph = GraphSource::large_110k();
    let lock = graph.read();
    let result = lock
        .traversal()
        .with("evaluationTimeout", 1)
        .v(None)
        .out(None)
        .out(None)
        .out(None)
        .count()
        .profile();
    let Err(err) = result else {
        panic!("profiling executes the traversal and must time out");
    };
    assert!(
        matches!(err.root_cause(), TraverserError::Timeout { .. }),
        "{err:?}"
    );
}

// T6
#[cfg(feature = "script")]
mod script {
    use std::sync::Arc;

    use crate::script_host::rhai::Dynamic;
    use graphersal::prelude::*;

    #[test]
    fn a_rhai_query_raises_the_timeout_diagnostic() {
        let graph = Arc::new(GraphSource::large_110k());
        let err = crate::script_host::eval_value(
            graph,
            r#"g.with("evaluationTimeout", 1).V().out().out().out().count().next()"#,
        )
        .expect_err("the query exceeds 1 ms");
        let rendered = err.to_string();
        assert!(
            rendered.contains("Traversal exceeded evaluationTimeout of 1 ms"),
            "rendered: {rendered}"
        );
        assert!(
            rendered.contains("Or raise the budget: g.with(\"evaluationTimeout\", 2)."),
            "rendered: {rendered}"
        );
    }

    #[test]
    fn an_invalid_value_in_rhai_is_reported_at_execution() {
        let graph = Arc::new(GraphSource::tinkerpop_modern());
        for value in ["-1", "\"abc\"", "1.5"] {
            let script = format!(r#"g.with("evaluationTimeout", {value}).V().count().next()"#);
            let err = crate::script_host::eval_value(graph.clone(), &script)
                .expect_err("an invalid value is rejected");
            let rendered = err.to_string();
            assert!(
                rendered.contains("Invalid value for option 'evaluationTimeout'"),
                "rendered: {rendered}"
            );
        }
    }

    #[test]
    fn scope_defaults_apply_and_a_query_overrides_them() {
        let graph = Arc::new(GraphSource::large_110k());
        let engine = crate::script_host::engine();
        let options = [("evaluationTimeout", ElementProperty::from(1))];

        let mut scope = crate::script_host::graph_scope_with_options(graph.clone(), &options);
        let err = engine
            .eval_with_scope::<Dynamic>(&mut scope, "g.V().out().out().out().count().next()")
            .expect_err("the default budget of 1 ms applies");
        assert!(err.to_string().contains("evaluationTimeout"), "{err}");

        let mut scope = crate::script_host::graph_scope_with_options(graph, &options);
        let count = engine
            .eval_with_scope::<Dynamic>(
                &mut scope,
                r#"g.with("evaluationTimeout", 0).V().out().out().out().count().next()"#,
            )
            .expect("the query's own value overrides the default");
        assert_eq!(count.as_int().unwrap(), 110_000);
    }
}
