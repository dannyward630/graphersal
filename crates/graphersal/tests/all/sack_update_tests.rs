//! `sack(Operator).by(..)` and `barrier(Barrier.normSack)` (sack series, step 3): the update
//! operator, its `by()` operand rules, the child-input rule of the sack, and sack normalisation.

use graphersal::prelude::*;

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

type Source<'b> = GraphTraversalSource<'b, &'b TraversalGraph>;

fn int(v: i64) -> ElementProperty {
    ElementProperty::Number(Number::from(v))
}

fn float(v: f64) -> ElementProperty {
    ElementProperty::Number(Number::from(v))
}

fn run<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    query(lock.traversal()).to_list().expect("query runs")
}

fn fails<F>(query: F) -> String
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    let err = query(lock.traversal())
        .to_list()
        .expect_err("query must fail");
    format_diagnostic(&err)
}

/// Runs `query` with merging off and on and checks both give the same multiset.
fn assert_merge_invisible<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    let mut unmerged = query(lock.traversal().with("bulk.merge", false))
        .to_list()
        .expect("unmerged runs");
    let merged = query(lock.traversal().with("bulk.merge", true))
        .to_list()
        .expect("merged runs");
    assert_eq!(unmerged.len(), merged.len());
    for item in &merged {
        let at = unmerged
            .iter()
            .position(|c| c == item)
            .unwrap_or_else(|| panic!("{item:?} missing"));
        unmerged.swap_remove(at);
    }
    merged
}

fn sorted(mut values: Vec<ElementProperty>) -> Vec<String> {
    let mut texts: Vec<String> = values.drain(..).map(|v| format!("{v:?}")).collect();
    texts.sort();
    texts
}

// ---- widening, overflow and the numeric scenarios ----

#[test]
fn integer_widths_widen_to_int64() {
    assert_eq!(
        run(|g| g
            .with_sack(127)
            .inject(vec![1])
            .sack_with(Operator::Sum)
            .sack()),
        vec![int(128)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(32767)
            .inject(vec![1])
            .sack_with(Operator::Sum)
            .sack()),
        vec![int(32768)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(2147483647)
            .inject(vec![1])
            .sack_with(Operator::Sum)
            .sack()),
        vec![int(2147483648)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(-128)
            .inject(vec![1])
            .sack_with(Operator::Minus)
            .sack()),
        vec![int(-129)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(127)
            .inject(vec![2])
            .sack_with(Operator::Mult)
            .sack()),
        vec![int(254)]
    );
}

#[test]
fn division_follows_java() {
    assert_eq!(
        run(|g| g
            .with_sack(7)
            .inject(vec![2])
            .sack_with(Operator::Div)
            .sack()),
        vec![int(3)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(7)
            .inject(vec![2.0])
            .sack_with(Operator::Div)
            .sack()),
        vec![float(3.5)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(127)
            .inject(vec![0.5])
            .sack_with(Operator::Div)
            .sack()),
        vec![float(254.0)]
    );
    assert_eq!(
        run(|g| g
            .with_sack(-128)
            .inject(vec![-1])
            .sack_with(Operator::Div)
            .sack()),
        vec![int(128)]
    );
    assert!(
        fails(|g| g.with_sack(1).inject(vec![0]).sack_with(Operator::Div))
            .contains("division by zero")
    );
}

#[test]
fn float_overflow_is_infinity_and_int_overflow_is_an_error() {
    let max = f64::MAX;
    assert_eq!(
        run(move |g| g
            .with_sack(max)
            .inject(vec![max])
            .sack_with(Operator::Sum)
            .sack()),
        vec![float(f64::INFINITY)]
    );
    let report = fails(|g| {
        g.with_sack(i64::MAX)
            .inject(vec![1])
            .sack_with(Operator::Sum)
    });
    assert!(
        report.contains("Operator 'sum'") && report.contains("overflow"),
        "{report}"
    );
    assert!(
        report.contains("sack(Operator.sum)"),
        "the failing step is named: {report}"
    );
}

#[test]
fn null_sack_rules() {
    // `null + x = null` (NumberHelper), so only the four vertices with an age produce a value,
    // and each of them keeps the null sack.
    let nulls = run(|g| g.v(None).sack_with(Operator::Sum).by("age").sack());
    assert_eq!(nulls, vec![ElementProperty::Null; 4]);
    // assign works on a null sack
    assert_eq!(
        sorted(run(|g| g
            .v(None)
            .sack_with(Operator::Assign)
            .by("age")
            .sack()))
        .len(),
        4
    );
}

// ---- Sack.feature scenarios, written out ----

#[test]
fn assign_by_label_then_read() {
    let labels = run(|g| {
        g.with_sack("hello")
            .v(None)
            .out_e(None)
            .sack_with(Operator::Assign)
            .by(By::Label)
            .in_v()
            .sack()
    });
    assert_eq!(
        sorted(labels),
        sorted(vec![
            "knows".into(),
            "knows".into(),
            "created".into(),
            "created".into(),
            "created".into(),
            "created".into()
        ])
    );
}

#[test]
fn sum_by_weight_accumulates_along_the_walk() {
    let sacks = run(|g| {
        g.with_sack(0)
            .v(None)
            .out_e(None)
            .sack_with(Operator::Sum)
            .by("weight")
            .in_v()
            .sack()
    });
    assert_eq!(
        sorted(sacks),
        sorted(vec![
            float(0.5),
            float(1.0),
            float(0.4),
            float(1.0),
            float(0.4),
            float(0.2)
        ])
    );
    let total = run(|g| {
        g.with_sack(0)
            .v(None)
            .out_e(None)
            .sack_with(Operator::Sum)
            .by("weight")
            .in_v()
            .sack()
            .sum()
    });
    assert_eq!(total.len(), 1);
    assert!((total[0].as_f64().unwrap() - 3.5).abs() < 1e-9, "{total:?}");
}

#[test]
fn assign_by_age_drops_unproductive_traversers() {
    let ages = run(|g| g.v(None).sack_with(Operator::Assign).by("age").sack());
    assert_eq!(
        sorted(ages),
        sorted(vec![int(29), int(27), int(32), int(35)])
    );
}

#[test]
fn div_by_a_constant_traversal() {
    let values = run(|g| {
        g.with_sack(2)
            .v(None)
            .sack_with(Operator::Div)
            .by(__::constant(4.0))
            .sack()
    });
    assert_eq!(values, vec![float(0.5); 6]);
}

#[test]
fn a_second_by_is_an_invalid_modulator() {
    let report = fails(|g| g.v(None).sack_with(Operator::Assign).by("age").by("name"));
    assert!(
        report.contains("Sack step can only have one by modulator"),
        "{report}"
    );
}

#[test]
fn local_child_carries_the_update_out() {
    let sacks = run(|g| {
        g.with_sack(0)
            .v(None)
            .in_(None)
            .barrier()
            .local(__::sack_with(Operator::Sum).by("age"))
            .sack()
    });
    assert_eq!(
        sorted(sacks),
        sorted(vec![int(29), int(29), int(29), int(32), int(32), int(35)])
    );
}

#[test]
fn where_child_does_not_change_the_outer_sack() {
    let sacks = run(|g| {
        g.with_sack(1)
            .v(None)
            .where_t(__::sack_with(Operator::Sum).by(__::constant(5)))
            .sack()
    });
    assert_eq!(sacks, vec![int(1); 6]);
}

// ---- by() resolution ----

#[test]
fn by_takes_the_first_result_of_a_traversal() {
    let first = run(|g| {
        g.with_sack(0)
            .v(None)
            .has("name", "marko")
            .sack_with(Operator::Sum)
            .by(__::out_e(None).values("weight"))
            .sack()
    });
    let expected = run(|g| {
        g.v(None)
            .has("name", "marko")
            .out_e(None)
            .values("weight")
            .limit(1)
    });
    assert_eq!(first, expected);
    assert_eq!(first.len(), 1);
}

#[test]
fn an_empty_by_traversal_drops_the_traverser() {
    let none = run(|g| {
        g.with_sack(0)
            .v(None)
            .sack_with(Operator::Sum)
            .by(__::out(None).limit(0))
            .sack()
    });
    assert!(none.is_empty());
}

#[test]
fn without_by_the_operand_is_the_traversers_own_value() {
    assert_eq!(
        run(|g| g
            .with_sack(1)
            .inject(vec![2])
            .sack_with(Operator::Sum)
            .sack()),
        vec![int(3)]
    );
}

#[test]
fn a_vertex_operand_is_refused() {
    let report = fails(|g| g.with_sack(1).v(None).sack_with(Operator::Assign));
    assert!(report.contains("vertex"), "{report}");
}

// ---- sack child-input rule ----

#[test]
fn sack_update_then_path_by_sack_uses_the_initial_sack() {
    // `path().by(..)` projects bare objects: they get the initial sack (TinkerPop `generate()`),
    // so the update is invisible there.
    let initial = run(|g| {
        g.with_sack(1)
            .v(None)
            .sack_with(Operator::Sum)
            .by(__::constant(1))
            .path()
            .by(__::sack())
            .unfold()
    });
    assert_eq!(initial, vec![int(1); 6]);
}

#[test]
fn sack_update_then_real_traverser_children_see_the_updated_sack() {
    let updated = run(|g| {
        g.with_sack(1)
            .v(None)
            .sack_with(Operator::Sum)
            .by(__::constant(1))
            .project(vec!["s"])
            .by(__::sack())
            .select("s")
    });
    assert_eq!(updated, vec![int(2); 6]);
    let order = run(|g| {
        g.with_sack(1)
            .v(None)
            .sack_with(Operator::Sum)
            .by(__::constant(1))
            .order()
            .by(__::sack())
            .sack()
    });
    assert_eq!(order, vec![int(2); 6]);
}

#[test]
fn the_update_does_not_extend_the_path_but_the_read_does() {
    let lens = |query: for<'b> fn(Source<'b>) -> Source<'b>| {
        run(query)
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        lens(|g| g
            .v(None)
            .sack_with(Operator::Assign)
            .by("age")
            .path()
            .count_scoped(Scope::Local)),
        vec![1; 4]
    );
    assert_eq!(
        lens(|g| g
            .v(None)
            .sack_with(Operator::Assign)
            .by("age")
            .sack()
            .path()
            .count_scoped(Scope::Local)),
        vec![2; 4]
    );
}

// ---- bulk ----

#[test]
fn merging_is_invisible_to_a_sack_update() {
    let merged = assert_merge_invisible(|g| {
        g.with_sack(1)
            .v(None)
            .out(None)
            .barrier()
            .sack_with(Operator::Sum)
            .by(__::constant(1))
            .sack()
    });
    assert_eq!(merged, vec![int(2); 6]);
    assert_merge_invisible(|g| {
        g.with_sack(0)
            .v(None)
            .repeat(__::out_e(None).sack_with(Operator::Sum).by("weight").in_v())
            .times(2)
            .sack()
    });
}

// ---- normSack ----

#[test]
fn norm_sack_scales_to_one() {
    let sacks = run(|g| {
        g.with_sack(1.0)
            .v(None)
            .out(None)
            .barrier_with(Barrier::NormSack)
            .sack()
    });
    assert_eq!(sacks.len(), 6);
    assert!(
        sacks.iter().all(|s| s.as_f64().unwrap() == 1.0 / 6.0),
        "{sacks:?}"
    );
    let off = assert_merge_invisible(|g| {
        g.with_sack(1.0)
            .v(None)
            .out(None)
            .barrier_with(Barrier::NormSack)
            .sack()
    });
    let total: f64 = off.iter().map(|s| s.as_f64().unwrap()).sum();
    assert!((total - 1.0).abs() < 1e-12);
}

#[test]
fn norm_sack_runs_on_a_single_traverser_and_with_merging_off() {
    assert_eq!(
        run(|g| g
            .with_sack(5)
            .inject(vec![1])
            .barrier_with(Barrier::NormSack)
            .sack()),
        vec![float(1.0)]
    );
    let graph = modern();
    let lock = graph.read();
    let off = lock
        .traversal()
        .with("bulk.merge", false)
        .with_sack(2)
        .v(None)
        .barrier_with(Barrier::NormSack)
        .sack()
        .to_list()
        .unwrap();
    assert!(
        off.iter().all(|s| s.as_f64().unwrap() == 1.0 / 6.0),
        "{off:?}"
    );
}

#[test]
fn norm_sack_keeps_null_and_rejects_text() {
    assert_eq!(
        run(|g| g.v(None).barrier_with(Barrier::NormSack).sack()),
        vec![ElementProperty::Null; 6]
    );
    let report = fails(|g| g.with_sack("a").v(None).barrier_with(Barrier::NormSack));
    assert!(report.contains("normSack"), "{report}");
}

#[test]
fn norm_sack_weights_unequal_sacks() {
    let sacks = run(|g| {
        g.with_sack(0)
            .v(None)
            .out_e(None)
            .sack_with(Operator::Sum)
            .by("weight")
            .barrier_with(Barrier::NormSack)
            .sack()
    });
    let total: f64 = sacks.iter().map(|s| s.as_f64().unwrap()).sum();
    assert!((total - 1.0).abs() < 1e-12, "{sacks:?}");
}

// ---- optimizer and profile ----

#[test]
fn profile_names_the_steps_and_no_rule_fuses_them() {
    let graph = modern();
    let lock = graph.read();
    let text = lock
        .traversal()
        .with_sack(0)
        .v(None)
        .out_e(None)
        .sack_with(Operator::Sum)
        .by("weight")
        .in_v()
        .barrier_with(Barrier::NormSack)
        .sack()
        .profile()
        .unwrap()
        .to_string();
    assert!(
        text.contains(r#"sack(Operator.sum).by("weight")"#),
        "{text}"
    );
    assert!(text.contains("barrier(Barrier.normSack)"), "{text}");
    assert!(text.contains("sack()"), "{text}");
}

#[test]
fn disabling_every_optimizer_rule_changes_nothing() {
    let graph = modern();
    let lock = graph.read();
    fn query<'b>(g: Source<'b>) -> Source<'b> {
        g.with_sack(0)
            .v(None)
            .out_e(None)
            .sack_with(Operator::Sum)
            .by("weight")
            .in_v()
            .barrier_with(Barrier::NormSack)
            .sack()
    }
    let default = query(lock.traversal()).to_list().unwrap();
    let plain = query(lock.traversal().with("optimizer.disabled", vec!["all"]))
        .to_list()
        .unwrap();
    assert_eq!(sorted(default), sorted(plain));
}

// ---- Rhai ----

#[cfg(feature = "script")]
mod dsl {
    use super::*;
    use std::sync::Arc;

    fn eval(script: &str) -> Result<Vec<ElementProperty>, String> {
        let graph = Arc::new(modern());
        let engine = crate::script_host::engine();
        let mut scope = crate::script_host::graph_scope(graph.clone());
        let gts = engine
            .eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(&mut scope, script)
            .map_err(|e| format!("{script}: {e}"))?;
        let lock = graph.read();
        gts.ast_convert_to_gts(&*lock)
            .to_list()
            .map_err(|e| format_diagnostic(&e).to_string())
    }

    fn ok(script: &str) -> Vec<ElementProperty> {
        eval(script).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn the_numeric_scenarios() {
        assert_eq!(
            ok("g.withSack(127).inject(1).sack(Operator.sum).sack()"),
            vec![int(128)]
        );
        assert_eq!(
            ok("g.withSack(2147483647).inject(1).sack(Operator.sum).sack()"),
            vec![int(2147483648)]
        );
        assert_eq!(
            ok(
                "g.withSack(1.7976931348623157e308).inject(1.7976931348623157e308).sack(Operator.sum).sack()"
            ),
            vec![float(f64::INFINITY)]
        );
        assert_eq!(
            ok("g.withSack(-128).inject(1).sack(Operator.minus).sack()"),
            vec![int(-129)]
        );
        assert_eq!(
            ok("g.withSack(127).inject(2).sack(Operator.mult).sack()"),
            vec![int(254)]
        );
        assert_eq!(
            ok("g.withSack(127).inject(0.5).sack(Operator.div).sack()"),
            vec![float(254.0)]
        );
        assert_eq!(
            ok("g.withSack(-32768).inject(-1).sack(Operator.div).sack()"),
            vec![int(32768)]
        );
        assert_eq!(
            ok("g.withSack(7).inject(2).sack(Operator::div).sack()"),
            vec![int(3)]
        );
        assert!(eval("g.withSack(9223372036854775807).inject(1).sack(Operator.sum)").is_err());
        assert!(eval("g.withSack(1).inject(0).sack(Operator.div)").is_err());
    }

    #[test]
    fn the_feature_scenarios() {
        assert_eq!(
            sorted(ok(
                r#"g.withSack("hello").V().outE().sack(Operator.assign).by(T.label).inV().sack()"#
            )),
            sorted(vec![
                "knows".into(),
                "knows".into(),
                "created".into(),
                "created".into(),
                "created".into(),
                "created".into()
            ])
        );
        let sum =
            ok("g.withSack(0).V().outE().sack(Operator.sum).by(\"weight\").inV().sack().sum()");
        assert!((sum[0].as_f64().unwrap() - 3.5).abs() < 1e-9);
        let twice = ok(
            r#"g.withSack(0).V().repeat(__.outE().sack(Operator.sum).by("weight").inV()).times(2).sack()"#,
        );
        assert_eq!(twice.len(), 2);
        assert_eq!(
            ok(r#"g.V().sack(Operator.assign).by("age").sack()"#).len(),
            4
        );
        assert_eq!(
            ok("g.withSack(2).V().sack(Operator.div).by(__.constant(4.0)).sack()"),
            vec![float(0.5); 6]
        );
        assert!(
            eval(r#"g.V().sack(Operator.assign).by("age").by("name").sack()"#)
                .unwrap_err()
                .contains("Sack step can only have one by modulator")
        );
        assert_eq!(
            sorted(ok(
                r#"g.withSack(0).V().in().barrier().local(__.sack(Operator.sum).by("age")).sack()"#
            )),
            sorted(vec![int(29), int(29), int(29), int(32), int(32), int(35)])
        );
        let below = ok(
            r#"g.withSack(0).V().repeat(__.sack(Operator.sum).by("age").where(__.sack().is(P.lt(59)))).times(2)"#,
        );
        assert!(!below.is_empty());
        assert_eq!(ok("g.withSack(1).inject(1).repeat(__.sack(Operator.sum).by(__.constant(1))).times(5).emit().math(\"_ + 0\").by(__.sack())").len(), 5);
    }

    #[test]
    fn a_bool_scenario_and_the_child_input_rule() {
        let values = ok(
            r#"g.V().sack(Operator.assign).by(__.hasLabel("person").count().asBool()).sack(Operator.and).by(__.outE().count().asBool()).sack().path()"#,
        );
        assert_eq!(values.len(), 6);
        assert_eq!(
            ok(
                "g.withSack(1).V().sack(Operator.sum).by(__.constant(1)).path().by(__.sack()).unfold()"
            ),
            vec![int(1); 6]
        );
        assert_eq!(
            ok(
                r#"g.withSack(1).V().sack(Operator.sum).by(__.constant(1)).project("s").by(__.sack()).select("s")"#
            ),
            vec![int(2); 6]
        );
    }

    #[test]
    fn norm_sack_both_spellings() {
        for script in [
            "g.withSack(1.0).V().out().barrier(Barrier.normSack).sack()",
            "g.withSack(1.0).V().out().barrier(Barrier::normSack).sack()",
        ] {
            let sacks = ok(script);
            assert_eq!(sacks.len(), 6);
            assert!(
                sacks.iter().all(|s| s.as_f64() == Some(1.0 / 6.0)),
                "{script}: {sacks:?}"
            );
        }
        assert_eq!(
            ok("g.V().barrier(Barrier.normSack).sack()"),
            vec![ElementProperty::Null; 6]
        );
        assert_eq!(
            ok("g.withSack(5).inject(1).barrier(Barrier.normSack).sack()"),
            vec![float(1.0)]
        );
        assert!(eval(r#"g.withSack("a").V().barrier(Barrier.normSack)"#).is_err());
    }

    #[test]
    fn bad_arguments_keep_their_errors() {
        assert!(eval("g.V().barrier(0)").unwrap_err().contains("barrier(0)"));
        assert!(
            eval(r#"g.V().barrier("x")"#)
                .unwrap_err()
                .contains("barrier(Consumer) is not supported")
        );
        assert!(
            eval("g.V().sack(5)")
                .unwrap_err()
                .contains("sack(BiFunction) is not supported")
        );
        assert!(eval("g.V().sack(Barrier.normSack)").is_err());
        assert!(
            eval(r#"g.withSack("a").V().sack(Operator.sum).by("age")"#)
                .unwrap_err()
                .contains("OperatorFailed")
                || eval(r#"g.withSack("a").V().sack(Operator.sum).by("age")"#)
                    .unwrap_err()
                    .contains("Operator 'sum'")
        );
    }

    #[test]
    fn barrier_is_a_reserved_parameter_name() {
        let err = crate::script_host::eval_with_params(
            Arc::new(GraphSource::tinkerpop_modern()),
            "g.v().count().next()",
            std::collections::HashMap::from([(
                "Barrier".to_string(),
                ElementProperty::Boolean(true),
            )]),
        )
        .expect_err("a parameter shadowing the Barrier token must be rejected");
        assert!(
            err.to_string().contains("reserved by the script scope"),
            "{err}"
        );
    }
}
