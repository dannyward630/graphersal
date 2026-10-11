//! `V()`/`E()` inside child traversals (never dropped), `g.union(..)` as a start step, and the
//! one-argument forms of `union`/`coalesce`/`and`/`or`.
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

fn strings(script: &str) -> Vec<String> {
    run(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect()
}

#[test]
fn nested_v_maps_each_incoming_traverser_to_every_vertex() {
    assert_eq!(
        count("g.V().limit(1).union(__.V(), __.identity()).count().next()"),
        7
    );
    assert_eq!(
        count("g.V().limit(1).union(__.v(), __.identity()).count().next()"),
        7
    );
    assert_eq!(count("g.V().limit(2).union(__.V()).count().next()"), 12);
    assert_eq!(
        count(r#"g.V().limit(1).local(__.V("1", "2")).count().next()"#),
        2
    );
}

#[test]
fn nested_e_maps_each_incoming_traverser_to_every_edge() {
    assert_eq!(count("g.V().limit(1).union(__.E().count()).next()"), 6);
    assert_eq!(count("g.V().limit(3).union(__.E()).count().next()"), 18);
}

#[test]
fn start_union_with_nested_sources() {
    assert_eq!(
        strings(r#"g.union(__.V("1"), __.V("4")).values("name").toList()"#),
        ["marko", "josh"]
    );
    assert_eq!(
        count(r#"g.union(__.V().hasLabel("software"), __.V().hasLabel("person")).count().next()"#),
        6
    );
}

#[test]
fn start_union_inject_children_do_not_echo_the_seed() {
    let values = run("g.union(__.inject(1), __.inject(2)).toList()")
        .into_array()
        .unwrap();
    let ints: Vec<i64> = values.iter().map(|v| v.as_int().unwrap()).collect();
    assert_eq!(ints, [1, 2]);
}

#[test]
fn start_union_seed_is_an_ordinary_null_traverser() {
    let values = run("g.union(__.identity()).toList()").into_array().unwrap();
    assert_eq!(values.len(), 1);
    assert!(values[0].is_unit());
}

#[test]
fn start_union_runs_mutating_children_once() {
    let script = r#"
        let names = g.union(__.addV("person").property("name", "alice"),
                            __.addV("person").property("name", "bob"),
                            __.addV("person").property("name", "chris")).values("name").toList();
        names.len() * 100 + g.V().count().next()
    "#;
    assert_eq!(count(script), 3 * 100 + 9);
}

#[test]
fn merge_e_over_a_union_of_side_effect_maps() {
    let script = r#"
        g.withSideEffect("m1", #{"T.label": "knows", "Direction.OUT": "1", "Direction.IN": "2"})
         .withSideEffect("m2", #{"T.label": "self", "Direction.OUT": "2", "Direction.IN": "2"})
         .union(__.select("m1"), __.select("m2")).mergeE().count().next()
    "#;
    assert_eq!(count(script), 2);
}

#[test]
fn path_of_nested_v_restarts_at_the_nested_source() {
    let paths = run(
        r#"g.union(__.V().out().out(), __.V().hasLabel("software")).path().by("name").toList()"#,
    )
    .into_array()
    .unwrap();
    let lens: Vec<usize> = paths
        .iter()
        .map(|p| p.clone().into_array().map(|a| a.len()).unwrap_or(0))
        .collect();
    // two three-element paths from marko, then two single-element paths.
    assert_eq!(lens.iter().filter(|l| **l == 3).count(), 2, "{paths:?}");
    assert_eq!(lens.iter().filter(|l| **l == 1).count(), 2, "{paths:?}");
}

#[test]
fn nested_source_label_filter_is_folded_and_shown_in_the_profile() {
    assert_eq!(
        count(r#"g.V().limit(1).union(__.V().hasLabel("software")).count().next()"#),
        2
    );
    let profile = run(r#"g.union(__.V().hasLabel("software")).profile().to_string()"#).to_string();
    assert!(profile.contains(r#"v(labels: ["software"])"#), "{profile}");
}

#[test]
fn one_argument_forms() {
    assert_eq!(count("g.V().union(__.out()).count().next()"), 6);
    assert_eq!(count("g.V().union([__.out(), __.in()]).count().next()"), 12);
    assert_eq!(
        count(r#"g.V().coalesce(__.out("knows")).count().next()"#),
        2
    );
    assert_eq!(count(r#"g.V().and(__.out("created")).count().next()"#), 3);
    assert_eq!(count(r#"g.V().or(__.out("knows")).count().next()"#), 1);
    assert_eq!(count("g.V().union(__.out(), __.in()).count().next()"), 12);
}

#[test]
fn rust_api_nested_sources_and_start_union() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let n = lock
        .traversal()
        .v("1")
        .union(vec![__::v(None::<()>), __::identity()])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(n.as_i64().unwrap(), 7);
    let n = lock
        .traversal()
        .union(vec![__::v("1"), __::e("1")])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(n.as_i64().unwrap(), 2);
}
