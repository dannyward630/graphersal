//! Merge operator, `withBulk(false)` and the bulk-merge interaction (sack series, step 4):
//! `withSack(initial, Operator)` combines the sacks of equal traversers at an explicit
//! `barrier()`, the automatic merge points switch off, `withBulk(false)` is the option
//! `bulk.one`, and `normSack` uses TinkerPop's numerator only next to a merge operator.

use graphersal::prelude::*;

use super::bulk_tests::dense;

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

fn run_on<F>(graph: &Graph, query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let lock = graph.read();
    query(lock.traversal()).to_list().expect("query runs")
}

fn run<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    run_on(&modern(), query)
}

/// The four vertices of the first layer of the dense graph (ages 1 to 4).
fn layer0() -> Vec<&'static str> {
    vec!["l0_0", "l0_1", "l0_2", "l0_3"]
}

fn sorted_ints(values: Vec<ElementProperty>) -> Vec<i64> {
    let mut out: Vec<i64> = values
        .iter()
        .map(|v| match v {
            ElementProperty::Number(n) => n.as_i64().unwrap_or_else(|| panic!("not an int: {v:?}")),
            other => panic!("not a number: {other:?}"),
        })
        .collect();
    out.sort_unstable();
    out
}

fn profile_of<F>(graph: &Graph, query: F) -> String
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let lock = graph.read();
    query(lock.traversal()).profile().unwrap().to_string()
}

/// Debug helper for merge-operator plans: `bulk.merge=false` must change the result (the opposite
/// of `assert_bulk_invisible`, merging is visible here).
fn assert_merge_visible<F>(graph: &Graph, query: F)
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let lock = graph.read();
    let mut merged = query(lock.traversal().with("bulk.merge", true))
        .to_list()
        .unwrap();
    let mut unmerged = query(lock.traversal().with("bulk.merge", false))
        .to_list()
        .unwrap();
    let key = |v: &ElementProperty| format!("{v:?}");
    merged.sort_by_key(key);
    unmerged.sort_by_key(key);
    assert_ne!(
        merged, unmerged,
        "merging must be visible for a merge-operator plan"
    );
}

// ---- the three TinkerPop scenarios ----

#[test]
fn with_bulk_false_merges_lop_once_with_a_summed_sack() {
    // `g.withBulk(false).withSack(1, sum).V().out().barrier().sack()` is 3,1,1,1.
    let got = run(|g| {
        g.with_bulk(false)
            .with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .barrier()
            .sack()
    });
    assert_eq!(sorted_ints(got), vec![1, 1, 1, 3]);
}

#[test]
fn without_with_bulk_false_the_merged_bulk_expands() {
    let got = run(|g| {
        g.with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .barrier()
            .sack()
    });
    assert_eq!(sorted_ints(got), vec![1, 1, 1, 3, 3, 3]);
}

#[test]
fn norm_sack_scenarios_under_a_merge_operator() {
    // withBulk(false): normSack inside local, then the merge at the end: 1.0 once.
    let one = run(|g| {
        g.with_bulk(false)
            .with_sack_merge(1.0, Operator::Sum)
            .v("1")
            .local(__::out_e("knows").barrier_with(Barrier::NormSack).in_v())
            .in_("knows")
            .barrier()
            .sack()
    });
    assert_eq!(one, vec![float(1.0)]);
    // Without withBulk(false) the merged bulk of 2 expands: 1.0 twice.
    let two = run(|g| {
        g.with_sack_merge(1.0, Operator::Sum)
            .v("1")
            .local(__::out("knows").barrier_with(Barrier::NormSack))
            .in_("knows")
            .barrier()
            .sack()
    });
    assert_eq!(two, vec![float(1.0), float(1.0)]);
}

// ---- rules of the design (section 5) ----

#[test]
fn equal_sacks_merge_without_an_operator_and_the_multiset_is_unchanged() {
    let graph = modern();
    fn query<'b>(g: Source<'b>) -> Source<'b> {
        g.with_sack(0).v(None).out(None).barrier().sack()
    }
    let merged = run_on(&graph, query);
    assert_eq!(merged.len(), 6);
    assert!(merged.iter().all(|v| *v == int(0)));
    let lock = graph.read();
    let unmerged = query(lock.traversal().with("bulk.merge", false))
        .to_list()
        .unwrap();
    assert_eq!(merged, unmerged);
    // The merge happened: lop (3 paths) is one traverser of bulk 3.
    let text = profile_of(&graph, |g| {
        g.with_sack(0).v(None).out(None).barrier().sack()
    });
    assert!(text.lines().any(|l| l.starts_with("barrier()")), "{text}");
    assert!(text.contains("Bulk"), "{text}");
}

#[test]
fn with_bulk_false_and_a_sack_without_an_operator_never_merge() {
    let got = run(|g| {
        g.with_bulk(false)
            .with_sack(0)
            .v(None)
            .out(None)
            .barrier()
            .sack()
    });
    assert_eq!(got.len(), 6);
}

#[test]
fn with_bulk_false_without_a_sack_collapses_duplicates_at_barrier() {
    let got = run(|g| g.with_bulk(false).v(None).out(None).barrier());
    assert_eq!(got.len(), 4);
    let normal = run(|g| g.v(None).out(None).barrier());
    assert_eq!(normal.len(), 6);
}

#[test]
fn with_bulk_true_resets_the_option() {
    let got = run(|g| {
        g.with_bulk(false)
            .with_bulk(true)
            .v(None)
            .out(None)
            .barrier()
    });
    assert_eq!(got.len(), 6);
}

#[test]
fn operators_combine_in_encounter_order() {
    // Layer 0 starts get their age as sack (1..4), so a layer-1 vertex sees 1,2,3,4 in order,
    // a layer-2 vertex one from each of the four layer-1 starts (11..14).
    let graph = dense();
    let expect = |op: Operator, want: Vec<i64>| {
        let got = run_on(&graph, |g| {
            g.with_sack_merge(0, op)
                .v(None)
                .sack_with(Operator::Assign)
                .by("age")
                .out(None)
                .barrier()
                .sack()
                .dedup()
        });
        assert_eq!(sorted_ints(got), want, "{op:?}");
    };
    expect(Operator::Assign, vec![4, 14]);
    expect(Operator::Max, vec![4, 14]);
    expect(Operator::Min, vec![1, 11]);
    expect(Operator::Sum, vec![10, 50]);
}

#[test]
fn boolean_operators_combine_booleans() {
    let and = run(|g| {
        g.with_sack_merge(true, Operator::And)
            .inject(vec![true, false, true])
            .sack_with(Operator::Assign)
            .by(__::identity())
            .constant(1)
            .barrier()
            .sack()
    });
    assert_eq!(and, vec![ElementProperty::Boolean(false); 3]);
    let or = run(|g| {
        g.with_sack_merge(false, Operator::Or)
            .inject(vec![true, false, true])
            .sack_with(Operator::Assign)
            .by(__::identity())
            .constant(1)
            .barrier()
            .sack()
    });
    assert_eq!(or, vec![ElementProperty::Boolean(true); 3]);
}

#[test]
fn a_failing_merge_is_an_error_naming_the_barrier() {
    let graph = modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .with_sack_merge("a", Operator::Sum)
        .v(None)
        .out(None)
        .barrier()
        .sack()
        .to_list()
        .expect_err("sum over strings fails");
    let text = format_diagnostic(&err);
    assert!(text.contains("barrier"), "{text}");
}

// ---- automatic merge points ----

#[test]
fn a_merge_operator_turns_the_automatic_points_off() {
    let graph = dense();
    fn flat<'b>(g: Source<'b>) -> Source<'b> {
        g.with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .out(None)
            .sack()
    }
    let got = run_on(&graph, flat);
    assert_eq!(got.len(), 4 * 4 * 4);
    assert!(
        got.iter().all(|v| *v == int(1)),
        "no merge, no combined sacks"
    );
    let text = profile_of(&graph, flat);
    assert!(!text.contains("Bulk"), "{text}");
    // The lazy_barrier row is still there and passes through (In == Out).
    let row = text
        .lines()
        .find(|l| l.starts_with("lazy_barrier()"))
        .expect("row");
    let cells: Vec<&str> = row.split_whitespace().collect();
    assert_eq!(cells[2], cells[3], "{text}");

    fn repeated<'b>(g: Source<'b>) -> Source<'b> {
        g.with_sack_merge(1, Operator::Sum)
            .v(None)
            .has("name", "n00")
            .repeat(__::out(None))
            .times(2)
            .sack()
    }
    let got = run_on(&graph, repeated);
    assert_eq!(got.len(), 16);
    assert!(got.iter().all(|v| *v == int(1)));
    let text = profile_of(&graph, repeated);
    assert!(!text.contains("Bulk"), "{text}");
}

#[test]
fn a_barrier_in_the_loop_body_merges_per_iteration() {
    let graph = dense();
    // From one start vertex layer 2 is reached by 4 paths: merged per iteration, sack 4^(k-1).
    for (times, len, want) in [(1u32, 4usize, 1i64), (2, 16, 4)] {
        let got = run_on(&graph, |g| {
            g.with_sack_merge(1, Operator::Sum)
                .v(None)
                .has("name", "n00")
                .repeat(__::out(None).barrier())
                .times(times)
                .sack()
        });
        assert_eq!(got.len(), len);
        assert!(got.iter().all(|v| *v == int(want)), "{got:?}");
    }
    // From the whole first layer (4 starts) layer 2 is reached by 4^2 paths: sack 4^k.
    let got = run_on(&graph, |g| {
        g.with_sack_merge(1, Operator::Sum)
            .v(layer0())
            .repeat(__::out(None).barrier())
            .times(2)
            .sack()
    });
    assert_eq!(got.len(), 64);
    assert!(got.iter().all(|v| *v == int(16)), "{got:?}");
    let text = profile_of(&graph, |g| {
        g.with_sack_merge(1, Operator::Sum)
            .v(layer0())
            .repeat(__::out(None).barrier())
            .times(2)
            .sack()
    });
    assert!(text.contains("Bulk"), "{text}");
}

#[test]
fn disabling_lazy_barrier_changes_nothing_under_a_merge_operator() {
    let graph = dense();
    let lock = graph.read();
    fn query<'b>(g: Source<'b>) -> Source<'b> {
        g.with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .barrier()
            .out(None)
            .sack()
    }
    let default = query(lock.traversal()).to_list().unwrap();
    let off = query(
        lock.traversal()
            .with("optimizer.disabled", vec!["lazy_barrier"]),
    )
    .to_list()
    .unwrap();
    assert_eq!(sorted_ints(default), sorted_ints(off));
}

#[test]
fn bulk_one_switches_the_automatic_points_off_too() {
    let graph = dense();
    // No sack: still correct (count of paths), with and without bulk.one.
    fn count<'b>(g: Source<'b>) -> Source<'b> {
        g.v(None).out(None).out(None).out(None).count()
    }
    let lock = graph.read();
    let a = count(lock.traversal()).to_list().unwrap();
    let b = count(lock.traversal().with_bulk(false)).to_list().unwrap();
    assert_eq!(a, b);
    // `repeat` does not merge under bulk.one: the profile has no Bulk column.
    let text = profile_of(&graph, |g| {
        g.with_bulk(false)
            .v(None)
            .repeat(__::out(None))
            .times(2)
            .count()
    });
    assert!(!text.contains("Bulk"), "{text}");
    // With an explicit barrier the duplicates collapse and keep bulk 1.
    let got = run_on(&graph, |g| {
        g.with_bulk(false).v(layer0()).out(None).out(None).barrier()
    });
    assert_eq!(got.len(), 4);
}

// ---- normSack numerator ----

fn floats(values: Vec<ElementProperty>) -> Vec<f64> {
    let mut out: Vec<f64> = values
        .iter()
        .map(|v| match v {
            ElementProperty::Number(n) => n.as_f64().unwrap(),
            other => panic!("not a number: {other:?}"),
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

#[test]
fn norm_sack_numerator_follows_the_merge_operator() {
    // With an operator: lop merges to sack 3, bulk 3; total = 3*3 + 1 + 1 + 1 = 12 and the
    // numerator is sack * bulk (TinkerPop): lop 9/12 (three copies), the others 1/12.
    let with = floats(run(|g| {
        g.with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .barrier_with(Barrier::NormSack)
            .sack()
    }));
    let want = [1.0 / 12.0, 1.0 / 12.0, 1.0 / 12.0, 0.75, 0.75, 0.75];
    assert_eq!(with.len(), 6);
    for (got, want) in with.iter().zip(want) {
        assert!((got - want).abs() < 1e-12, "{with:?}");
    }
    // Without an operator the equal sacks merge invisibly: sack / total, total = 6.
    let without = floats(run(|g| {
        g.with_sack(1)
            .v(None)
            .out(None)
            .barrier_with(Barrier::NormSack)
            .sack()
    }));
    assert_eq!(without.len(), 6);
    for got in &without {
        assert!((got - 1.0 / 6.0).abs() < 1e-12, "{without:?}");
    }
    // ... and the answer does not depend on whether the merge happened.
    let lock = modern();
    let lock = lock.read();
    let unmerged = floats(
        lock.traversal()
            .with("bulk.merge", false)
            .with_sack(1)
            .v(None)
            .out(None)
            .barrier_with(Barrier::NormSack)
            .sack()
            .to_list()
            .unwrap(),
    );
    assert_eq!(without, unmerged);
}

// ---- bulk.merge=false and options ----

#[test]
fn bulk_merge_false_changes_a_merge_operator_result_but_not_the_others() {
    let graph = modern();
    assert_merge_visible(&graph, |g| {
        g.with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .barrier()
            .sack()
    });
    let unmerged = run(|g| {
        g.with("bulk.merge", false)
            .with_sack_merge(1, Operator::Sum)
            .v(None)
            .out(None)
            .barrier()
            .sack()
    });
    assert_eq!(sorted_ints(unmerged), vec![1; 6]);
    // No operator: invisible.
    let lock = graph.read();
    fn q<'b>(g: Source<'b>) -> Source<'b> {
        g.with_sack(0).v(None).out(None).barrier().sack()
    }
    assert_eq!(
        q(lock.traversal().with("bulk.merge", false))
            .to_list()
            .unwrap(),
        q(lock.traversal()).to_list().unwrap()
    );
}

#[test]
fn bulk_one_is_a_strict_boolean_option() {
    let graph = modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .with("bulk.one", "yes")
        .v(None)
        .to_list()
        .expect_err("invalid option");
    assert!(
        matches!(*err, TraverserError::InvalidOption { ref key, .. } if key == "bulk.one"),
        "{err:?}"
    );
    let help = err.help().expect("help");
    assert!(
        help.contains("with_bulk(false)") && help.contains("g.with(\"bulk.one\""),
        "{help}"
    );
    assert!(
        lock.traversal()
            .with("bulk.one", true)
            .v(None)
            .to_list()
            .is_ok()
    );
}

// ---- Rhai ----

#[cfg(feature = "script")]
mod dsl {
    use super::*;
    use std::sync::Arc;

    fn eval_on(graph: Arc<Graph>, script: &str) -> Result<Vec<ElementProperty>, String> {
        let engine = crate::script_host::engine();
        let mut scope = crate::script_host::graph_scope(graph.clone());
        let gts = engine
            .eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(&mut scope, script)
            .map_err(|e| format!("{script}: {e}"))?;
        let lock = graph.read();
        gts.ast_convert_to_gts(&*lock)
            .to_list()
            .map_err(|e| format!("{e:?}"))
    }

    fn eval(script: &str) -> Result<Vec<ElementProperty>, String> {
        eval_on(Arc::new(modern()), script)
    }

    #[test]
    fn both_spellings_of_with_sack_and_with_bulk() {
        for script in [
            "g.withBulk(false).withSack(1, Operator.sum).V().out().barrier().sack()",
            "g.with_bulk(false).with_sack(1, Operator.sum).v().out().barrier().sack()",
            "g.withBulk(false).withSack(1, Operator::sum).V().out().barrier().sack()",
        ] {
            assert_eq!(
                sorted_ints(eval(script).unwrap()),
                vec![1, 1, 1, 3],
                "{script}"
            );
        }
        assert_eq!(
            sorted_ints(eval("g.with_sack(1, Operator.sum).v().out().barrier().sack()").unwrap()),
            vec![1, 1, 1, 3, 3, 3]
        );
        assert_eq!(
            eval("g.with_bulk(false).with_bulk(true).v().out().barrier()")
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            eval("g.with_bulk(false).v().out().barrier()")
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn norm_sack_scenarios() {
        let one = eval(
            r#"g.withBulk(false).withSack(1.0, Operator.sum).V("1").local(__.outE("knows").barrier(Barrier.normSack).inV()).in("knows").barrier().sack()"#,
        )
        .unwrap();
        assert_eq!(one, vec![float(1.0)]);
        let two = eval(
            r#"g.withSack(1.0, Operator.sum).V("1").local(__.out("knows").barrier(Barrier.normSack)).in("knows").barrier().sack()"#,
        )
        .unwrap();
        assert_eq!(two, vec![float(1.0), float(1.0)]);
    }

    #[test]
    fn dense_loop_with_a_barrier_in_the_body() {
        let graph = Arc::new(dense());
        let got = eval_on(
            graph.clone(),
            r#"g.withSack(1, Operator.sum).V().has_p("age", P::lt(5)).repeat(__.out().barrier()).times(2).sack()"#,
        );
        let got = got.unwrap();
        assert_eq!(got.len(), 64);
        assert!(got.iter().all(|v| *v == int(16)));
        let flat = eval_on(
            graph,
            r#"g.withSack(1, Operator.sum).V().has_p("age", P::lt(5)).repeat(__.out()).times(2).sack()"#,
        )
        .unwrap();
        assert_eq!(flat.len(), 64);
        assert!(flat.iter().all(|v| *v == int(1)));
    }

    #[test]
    fn a_split_operator_or_a_lambda_is_rejected() {
        for script in [
            "g.withSack(1, 2).V().sack()",
            r#"g.withSack(1, "sum").V().sack()"#,
            "g.with_sack(1, 2).V().sack()",
        ] {
            let err = eval(script).expect_err(script);
            assert!(err.contains("Operator"), "{script}: {err}");
        }
        let err = eval("g.withSack(1, Operator.sum, Operator.sum).V().sack()")
            .expect_err("three arguments");
        assert!(err.contains("withSack"), "{err}");
    }

    #[test]
    fn with_bulk_is_not_a_mutating_step() {
        let text = eval("g.withBulk(false).V().count()").unwrap();
        assert_eq!(text, vec![int(6)]);
    }
}
