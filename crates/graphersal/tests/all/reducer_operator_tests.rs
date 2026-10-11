//! Reducer operators (sack series, step 5): `fold(seed, Operator)` and
//! `withSideEffect(name, initial, Operator)` with `aggregate()`/`store()`.

use graphersal::prelude::*;

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

type Source<'b> = GraphTraversalSource<'b, &'b TraversalGraph>;

fn int(v: i64) -> ElementProperty {
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

/// The same result with merging off and on (bulk is invisible for reducers: bulk n applies n times).
fn assert_merge_invisible<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    let unmerged = query(lock.traversal().with("bulk.merge", false))
        .to_list()
        .unwrap();
    let merged = query(lock.traversal().with("bulk.merge", true))
        .to_list()
        .unwrap();
    assert_eq!(unmerged, merged);
    merged
}

fn ints(values: &[i64]) -> ElementProperty {
    ElementProperty::Array(values.iter().map(|v| int(*v)).collect())
}

fn sorted_ints(value: &ElementProperty) -> Vec<i64> {
    let ElementProperty::Array(items) = value else {
        panic!("expected a list, got {value:?}");
    };
    let mut out: Vec<i64> = items.iter().map(|v| v.as_i64().expect("int")).collect();
    out.sort();
    out
}

// ---- fold(seed, Operator) ----

#[test]
fn fold_sum_over_ages() {
    assert_eq!(
        run(|g| g.v(None).values("age").fold_with(0, Operator::Sum)),
        vec![int(123)]
    );
}

#[test]
fn fold_of_an_empty_stream_is_the_seed() {
    assert_eq!(
        run(|g| g
            .v(None)
            .has_label("none")
            .values("age")
            .fold_with(0, Operator::Sum)),
        vec![int(0)]
    );
}

#[test]
fn fold_minus_follows_stream_order() {
    // 100 - 29 - 27 - 32 - 35 = -23 (stream order of the modern graph's ages).
    assert_eq!(
        run(|g| g.v(None).values("age").fold_with(100, Operator::Minus)),
        vec![int(-23)]
    );
}

#[test]
fn fold_of_scalars_into_a_list_appends_and_booleans_fold() {
    let result = run(|g| {
        g.v(None)
            .values("age")
            .fold_with(ElementProperty::Array(vec![]), Operator::AddAll)
    });
    assert_eq!(sorted_ints(&result[0]), vec![27, 29, 32, 35]);
    assert_eq!(
        run(|g| g.v(None).constant(false).fold_with(true, Operator::And)),
        vec![ElementProperty::Boolean(false)]
    );
}

#[test]
fn fold_with_a_string_seed_fails_with_operator_failed() {
    let message = fails(|g| g.v(None).values("age").fold_with("", Operator::Sum));
    assert!(message.contains("sum"), "{message}");
}

#[test]
fn fold_bulk_is_invisible_through_a_barrier() {
    let merged = assert_merge_invisible(|g| {
        g.v(None)
            .out(None)
            .values("age")
            .barrier()
            .fold_with(0, Operator::Sum)
    });
    // marko -> vadas 27, josh 32, (lop, no age); josh -> ripple, lop; peter -> lop: ages of targets.
    assert_eq!(merged, vec![int(27 + 32)]);
    assert_merge_invisible(|g| {
        g.v(None)
            .out(None)
            .values("age")
            .barrier()
            .fold_with(100, Operator::Max)
    });
}

#[test]
fn fold_emits_the_initial_sack() {
    assert_eq!(
        run(|g| g
            .with_sack(3)
            .v(None)
            .values("age")
            .fold_with(0, Operator::Sum)
            .sack()),
        vec![int(3)]
    );
}

#[test]
fn fold_profile_name() {
    let graph = modern();
    let lock = graph.read();
    let text = lock
        .traversal()
        .v(None)
        .values("age")
        .fold_with(0, Operator::Sum)
        .profile()
        .unwrap()
        .to_string();
    assert!(text.contains("fold(0, Operator.sum)"), "{text}");
}

// ---- withSideEffect(name, initial, Operator) ----

fn aggregate_age(op: Operator, initial: i64) -> ElementProperty {
    run(|g| {
        g.with_side_effect_reduced("a", initial, op)
            .v(None)
            .aggregate("a")
            .by("age")
            .cap("a")
    })
    .remove(0)
}

fn local_aggregate_age(op: Operator, initial: i64) -> ElementProperty {
    run(|g| {
        g.with_side_effect_reduced("a", initial, op)
            .v(None)
            .local(__::aggregate("a").by("age"))
            .cap("a")
    })
    .remove(0)
}

#[test]
fn numeric_reducers_global_and_local() {
    let table = [
        (Operator::Sum, 1, 124),
        (Operator::Minus, 123, 0),
        (Operator::Mult, 2, 1_753_920),
        (Operator::Div, 876_960, 1),
        (Operator::Min, 1, 1),
        (Operator::Min, 100, 27),
        (Operator::Max, 1, 35),
        (Operator::Max, 100, 100),
    ];
    for (op, initial, expected) in table {
        assert_eq!(aggregate_age(op, initial), int(expected), "global {op}");
        assert_eq!(
            local_aggregate_age(op, initial),
            int(expected),
            "local {op}"
        );
    }
}

#[test]
fn boolean_reducers() {
    for (op, expected) in [(Operator::And, false), (Operator::Or, true)] {
        let global = run(|g| {
            g.with_side_effect_reduced("a", true, op)
                .v(None)
                .constant(false)
                .aggregate("a")
                .cap("a")
        });
        let local = run(|g| {
            g.with_side_effect_reduced("a", true, op)
                .v(None)
                .constant(false)
                .local(__::aggregate("a"))
                .cap("a")
        });
        assert_eq!(
            global,
            vec![ElementProperty::Boolean(expected)],
            "global {op}"
        );
        assert_eq!(
            local,
            vec![ElementProperty::Boolean(expected)],
            "local {op}"
        );
    }
}

#[test]
fn add_all_concatenates_the_collection() {
    for local in [false, true] {
        let result = run(|g| {
            let g = g
                .with_side_effect_reduced("a", vec![1, 2, 3], Operator::AddAll)
                .v(None);
            if local {
                g.local(__::aggregate("a").by("age")).cap("a")
            } else {
                g.aggregate("a").by("age").cap("a")
            }
        });
        assert_eq!(
            sorted_ints(&result[0]),
            vec![1, 2, 3, 27, 29, 32, 35],
            "local={local}"
        );
    }
}

#[test]
fn assign_receives_the_whole_batch_of_one_execution() {
    // Global: one execution sees all four ages, `assign` keeps them as one list.
    let global = aggregate_with_array_seed_assign(false);
    assert_eq!(sorted_ints(&global), vec![27, 29, 32, 35]);
    // Local: one execution per vertex; the last one (ordered by age) wins, as a one-element list.
    let local = aggregate_with_array_seed_assign(true);
    assert_eq!(local, ints(&[35]));
}

fn aggregate_with_array_seed_assign(local: bool) -> ElementProperty {
    run(|g| {
        let g = g
            .with_side_effect_reduced("a", vec![1, 2, 3], Operator::Assign)
            .v(None);
        if local {
            g.order()
                .by("age")
                .local(__::aggregate("a").by("age"))
                .cap("a")
        } else {
            g.aggregate("a").by("age").cap("a")
        }
    })
    .remove(0)
}

#[test]
fn an_empty_execution_leaves_the_seed() {
    assert_eq!(
        run(|g| g
            .with_side_effect_reduced("a", 7, Operator::Sum)
            .v(None)
            .has_label("none")
            .aggregate("a")
            .cap("a")),
        vec![int(7)]
    );
}

#[test]
fn store_folds_per_element() {
    assert_eq!(
        run(|g| g
            .with_side_effect_reduced("a", 1, Operator::Sum)
            .v(None)
            .store("a")
            .by("age")
            .cap("a")),
        vec![int(124)]
    );
    // `store` hands the reducer one value per element: assign leaves the last list of one.
    assert_eq!(
        run(|g| g
            .with_side_effect_reduced("a", 0, Operator::Assign)
            .v(None)
            .order()
            .by("age")
            .store("a")
            .by("age")
            .cap("a")),
        vec![ints(&[35])]
    );
}

#[test]
fn reducer_is_read_by_select_and_where() {
    assert_eq!(
        run(|g| g
            .with_side_effect_reduced("a", 1, Operator::Sum)
            .v(None)
            .aggregate("a")
            .by("age")
            .limit(1)
            .select("a")),
        vec![int(124)]
    );
    // A reduced scalar behaves like a single value in a comparison.
    let kept = run(|g| {
        g.with_side_effect_reduced("limit", 30, Operator::Sum)
            .v(None)
            .has_label("person")
            .values("age")
            .where_p(P::Lt("limit"))
    });
    assert!(!kept.is_empty(), "{kept:?}");
}

#[test]
fn bulk_n_applies_n_times_and_merging_is_invisible() {
    for (op, initial) in [
        (Operator::Sum, 0),
        (Operator::Max, 0),
        (Operator::Min, 100),
        (Operator::Mult, 1),
    ] {
        assert_merge_invisible(|g| {
            g.with_side_effect_reduced("a", initial, op)
                .v(None)
                .out(None)
                .barrier()
                .aggregate("a")
                .by(__::constant(2))
                .cap("a")
        });
    }
    // out() reaches lop three times: sum of constants is 2 * 6.
    assert_eq!(
        run(|g| g
            .with_side_effect_reduced("a", 0, Operator::Sum)
            .v(None)
            .out(None)
            .barrier()
            .aggregate("a")
            .by(__::constant(2))
            .cap("a")),
        vec![int(12)]
    );
}

#[test]
fn a_failing_operator_fails_the_query() {
    let message = fails(|g| {
        g.with_side_effect_reduced("a", "x", Operator::Sum)
            .v(None)
            .aggregate("a")
            .by("age")
            .cap("a")
    });
    assert!(message.contains("sum"), "{message}");
}

#[test]
fn existing_buckets_are_unchanged() {
    let list = run(|g| {
        g.with_side_effect("a", vec![1, 2])
            .v(None)
            .aggregate("a")
            .by("age")
            .cap("a")
    });
    assert_eq!(sorted_ints(&list[0]), vec![1, 2, 27, 29, 32, 35]);
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
    fn fold_and_with_side_effect_in_both_spellings() {
        assert_eq!(
            ok(r#"g.V().values("age").fold(0, Operator.sum)"#),
            vec![int(123)]
        );
        assert_eq!(
            ok(r#"g.v().values("age").fold(0, Operator::sum)"#),
            vec![int(123)]
        );
        assert_eq!(
            ok(r#"g.withSideEffect("a", 1, Operator.sum).V().aggregate("a").by("age").cap("a")"#),
            vec![int(124)]
        );
        assert_eq!(
            ok(r#"g.with_side_effect("a", 1, Operator.sum).v().aggregate("a").by("age").cap("a")"#),
            vec![int(124)]
        );
        // a seeded fold always emits: one result per vertex, vertices without age give the seed
        assert_eq!(
            ok(r#"g.V().local(__.values("age").fold(0, Operator.sum))"#).len(),
            6
        );
    }

    #[test]
    fn fold_add_all_with_a_map_seed() {
        let result = ok(r#"g.inject(#{a: 1}, #{b: 2}, #{b: 4}).fold(#{}, Operator.addAll)"#);
        assert_eq!(result.len(), 1);
        assert!(format!("{:?}", result[0]).contains('4'), "{:?}", result[0]);
    }

    #[test]
    fn a_second_argument_that_is_not_an_operator_is_an_error() {
        let fold = eval(r#"g.V().fold(0, 5)"#).unwrap_err();
        assert!(fold.contains("fold(seed, BiFunction)"), "{fold}");
        let side_effect = eval(r#"g.withSideEffect("a", 0, "sum")"#).unwrap_err();
        assert!(side_effect.contains("Operator"), "{side_effect}");
    }

    #[test]
    fn the_existing_with_side_effect_forms_still_work() {
        assert_eq!(
            ok(r#"g.withSideEffect("a", 5).V().limit(1).select("a")"#),
            vec![int(5)]
        );
        assert!(
            ok(r#"g.withSideEffect("a", Set.of()).V().values("age").aggregate("a").cap("a")"#)
                .len()
                == 1
        );
    }
}
