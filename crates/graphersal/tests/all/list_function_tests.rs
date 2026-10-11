//! Cross-cutting behaviour of the list functions `combine`, `conjoin`, `difference`, `disjunct`,
//! `intersect`, `merge` and `product`: path recording for a child that reads labels, bulk and
//! sack propagation, the fused `count()` and the DSL spellings. The per-step semantics are tested
//! in each step's module.

#![cfg(feature = "script")]

use std::sync::Arc;

use graphersal::prelude::*;

use crate::script_host::eval_value;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn names(values: &[ElementProperty]) -> Vec<String> {
    values
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect()
}

/// A child traversal that reads a path label makes the step record that label upstream.
#[test]
fn a_child_traversal_reads_the_path_of_the_incoming_traverser() {
    let graph = modern();
    let lock = graph.read();
    let result = lock
        .traversal()
        .v("1")
        .as_("a")
        .out("knows")
        .constant(vec!["x"])
        .combine_traversal(__::select(vec!["a"]).values("name").fold())
        .conjoin("+")
        .to_list()
        .unwrap();
    let mut got = names(&result);
    got.sort();
    assert_eq!(got, vec!["x+marko", "x+marko"]);
}

/// A merged (bulk > 1) traverser keeps its bulk through the step: the results are the same
/// multiset with and without merging.
#[test]
fn bulk_is_kept() {
    let graph = modern();
    let lock = graph.read();
    let count = |merge: bool| {
        lock.traversal()
            .with("bulk.merge", merge)
            .v(None)
            .both(None)
            .barrier()
            .local(__::fold().combine(vec![1]))
            .count()
            .next()
            .unwrap()
            .unwrap()
    };
    assert_eq!(count(true).as_i64(), Some(12));
    assert_eq!(count(false).as_i64(), Some(12));
}

/// The sack of the incoming traverser travels with the result.
#[test]
fn the_sack_is_kept() {
    let result = eval_value(
        modern(),
        r#"g.withSack(7).V("1").fold().product([1, 2]).sack().toList()"#,
    )
    .unwrap();
    assert_eq!(result.to_string(), "[7]");
}

/// A fused `count()` after the step still runs it, so its errors are raised.
#[test]
fn a_fused_count_does_not_skip_the_step() {
    for step in [
        r#"combine([1])"#,
        r#"difference([1])"#,
        r#"disjunct([1])"#,
        r#"intersect([1])"#,
        r#"merge([1])"#,
        r#"product([1])"#,
        r#"conjoin("-")"#,
    ] {
        let err = eval_value(
            modern(),
            &format!(r#"g.V().values("age").{step}.count().next()"#),
        )
        .expect_err(step)
        .to_string();
        assert!(err.contains("as its incoming value"), "{step}: {err}");
    }
    let ok = eval_value(modern(), r#"g.V().fold().combine([1]).count().next()"#).unwrap();
    assert_eq!(ok.as_int().unwrap(), 1);
}

/// Each name has one spelling (snake and camel case coincide) and takes a list, a traversal or,
/// for `merge`, a map.
#[test]
fn dsl_forms() {
    for (query, expected) in [
        (r#"g.inject([1, 2]).combine([2]).toList()"#, "[[1, 2, 2]]"),
        (r#"g.inject([1, 2]).difference([2]).toList()"#, "[[1]]"),
        (r#"g.inject([1, 2]).disjunct([2, 3]).toList()"#, "[[1, 3]]"),
        (r#"g.inject([1, 2]).intersect([2, 3]).toList()"#, "[[2]]"),
        (r#"g.inject([1, 2]).merge([2, 3]).toList()"#, "[[1, 2, 3]]"),
        (r#"g.inject([1]).product([2]).toList()"#, "[[[1, 2]]]"),
        (r#"g.inject([1, 2]).conjoin("+").toList()"#, r#"["1+2"]"#),
        (
            r#"g.inject([1]).combine(__.constant([2])).toList()"#,
            "[[1, 2]]",
        ),
        (
            r#"g.inject(#{a: 1}).merge(#{b: 2}).toList()"#,
            r#"[#{"a": 1, "b": 2}]"#,
        ),
    ] {
        let value = eval_value(modern(), query).unwrap_or_else(|e| panic!("{query}: {e}"));
        assert_eq!(value.to_string(), expected, "{query}");
    }
}
