//! Side effects hold stream values, not snapshots: `aggregate`/`store` keep element handles, so
//! identity-based steps work on what `cap`/`select` give back.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> rhai::Dynamic {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn count(script: &str) -> i64 {
    run(script)
        .as_int()
        .unwrap_or_else(|_| panic!("{script}: not an int"))
}

fn sorted_strings(script: &str) -> Vec<String> {
    let mut out: Vec<String> = run(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn unfolded_bucket_holds_vertices() {
    for q in [
        r#"g.V().aggregate("a").cap("a").unfold().dedup().count().next()"#,
        r#"g.v().store("a").cap("a").unfold().dedup().count().next()"#,
    ] {
        assert_eq!(count(q), 6, "{q}");
    }
    // `out()` only works on vertex handles, not on map snapshots.
    assert_eq!(
        count(r#"g.V().aggregate("a").cap("a").unfold().out().count().next()"#),
        6
    );
    assert_eq!(
        count(r#"g.v().aggregate("a").cap("a").unfold().out().count().next()"#),
        6
    );
}

#[test]
fn select_of_a_bucket_keeps_vertices() {
    assert_eq!(
        count(r#"g.V().aggregate("a").select("a").limit(1).unfold().dedup().out().count().next()"#),
        6
    );
}

#[test]
fn edges_and_property_handles_keep_their_kind() {
    assert_eq!(
        sorted_strings(r#"g.E().aggregate("a").cap("a").unfold().label().toList()"#),
        ["created", "created", "created", "created", "knows", "knows"]
    );
    assert_eq!(
        sorted_strings(
            r#"g.V().properties("name").aggregate("a").cap("a").unfold().value().toList()"#
        ),
        ["josh", "lop", "marko", "peter", "ripple", "vadas"]
    );
}

#[test]
fn by_modulated_aggregate_stores_values_and_store_is_the_same() {
    for step in ["aggregate", "store"] {
        let q = format!(
            r#"g.V().hasLabel("person").{step}("a").by("name").cap("a").unfold().toList()"#
        );
        assert_eq!(
            sorted_strings(&q),
            ["josh", "marko", "peter", "vadas"],
            "{q}"
        );
    }
}

#[test]
fn where_within_and_without_use_identity() {
    assert_eq!(
        count(r#"g.V().aggregate("a").V().where(P.within("a")).count().next()"#),
        36
    );
    assert_eq!(
        count(
            r#"g.V().hasLabel("person").aggregate("a").V().where(P.without("a")).count().next()"#
        ),
        8
    );
}

#[test]
fn seeds_behave_as_before() {
    assert_eq!(
        count(r#"g.withSideEffect("x", 42).V().cap("x").next()"#),
        42
    );
    assert_eq!(
        sorted_strings(r#"g.withSideEffect("x", [1, 2]).V().limit(1).aggregate("x").cap("x").unfold().toList()"#).len(),
        3
    );
    assert_eq!(
        run(r#"g.withSideEffect("x", "s").V().limit(1).cap("x").next()"#).to_string(),
        "s"
    );
}

fn multiset(merge: bool) -> Vec<ElementProperty> {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let mut out = lock
        .traversal()
        .with("bulk.merge", merge)
        .v(None)
        .repeat(__::out(None))
        .times(2)
        .aggregate("a")
        .cap("a")
        .unfold()
        .to_list()
        .expect("query runs");
    out.sort_by_key(|v| format!("{v:?}"));
    out
}

#[test]
fn bulk_merge_does_not_change_the_bucket() {
    assert_eq!(multiset(false), multiset(true));
}

/// `within`/`without` against a side effect range over its members: a reduced bucket's list
/// items or its single value, and a `subgraph()` bucket's edges.
#[test]
fn within_ranges_over_reduced_and_subgraph_buckets() {
    assert_eq!(
        sorted_strings(
            r#"g.withSideEffect("a", ["marko", "josh"], Operator.addAll).V().values("name").where(P.within("a")).toList()"#
        ),
        ["josh", "marko"]
    );
    assert_eq!(
        sorted_strings(
            r#"g.withSideEffect("a", "marko", Operator.assign).V().values("name").where(P.within("a")).toList()"#
        ),
        ["marko"]
    );
    // two `knows` edges in the bucket; each of the two traversers then sees all six edges
    assert_eq!(
        count(
            r#"g.E().hasLabel("knows").subgraph("sg").barrier().E().where(P.within("sg")).count().next()"#
        ),
        4
    );
    assert_eq!(
        count(
            r#"g.E().hasLabel("knows").subgraph("sg").barrier().E().where(P.without("sg")).count().next()"#
        ),
        8
    );
}

/// `math()` reads a subgraph bucket as its list of edges, which is not a number.
#[test]
fn math_over_a_subgraph_bucket_explains_the_list() {
    let err = eval_value(
        Arc::new(GraphSource::tinkerpop_modern()),
        r#"g.E().hasLabel("knows").subgraph("sg").barrier().V().math("sg + 1").toList()"#,
    )
    .expect_err("a list is not a number")
    .to_string();
    assert!(
        err.contains("must resolve to a number, but it is a list"),
        "{err}"
    );
}
