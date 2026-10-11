//! `union()`/`choose()`/`branch()` children with global state run over the whole routed batch.
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
fn union_counts_cover_both_start_vertices() {
    for q in [
        r#"g.V("1", "2").union(__.outE().count(), __.inE().count(), __.outE().values("weight").sum()).toList()"#,
        r#"g.V("1", "2").union(__.out_e().count(), __.in_e().count(), __.out_e().values("weight").sum()).toList()"#,
    ] {
        assert_eq!(strings(q), ["3", "1", "1.9"], "{q}");
    }
}

#[test]
fn union_global_count_then_local_counts() {
    let out = strings(r#"g.V().union(__.outE().count(), __.local(__.inE().count())).toList()"#);
    assert_eq!(out.len(), 7);
    assert_eq!(out[0], "6");
    let mut rest: Vec<i64> = out[1..].iter().map(|s| s.parse().unwrap()).collect();
    rest.sort();
    assert_eq!(rest, [0, 0, 1, 1, 1, 3]);
}

#[test]
fn union_of_limits_limits_the_whole_stream() {
    assert_eq!(
        count("g.V().union(__.limit(1), __.limit(2)).count().next()"),
        3
    );
    assert_eq!(
        count("g.V().union(__.limit(1), __.limit(2)).toList().len()"),
        3
    );
}

#[test]
fn stateless_union_keeps_traverser_major_order() {
    let q = r#"g.V().limit(2).union(__.out(), __.in()).id().toList()"#;
    // vertex 1: out(), then in() (none); vertex 2: out() (none), then in() (1)
    assert_eq!(strings(q), ["4", "3", "2", "1"]);
}

#[test]
fn choose_with_stateful_options_sees_the_routed_stream() {
    assert_eq!(
        count(
            r#"g.V().choose(__.values("age").is(P.lte(30)), __.out().order().by("name").limit(1), __.out().order().by("name").limit(2)).count().next()"#
        ),
        3
    );
    // branch(): both options fire for every traverser
    assert_eq!(
        count(
            r#"g.V().branch(__.label()).option("person", __.out().limit(1)).option("software", __.in().limit(2)).count().next()"#
        ),
        3
    );
}

#[test]
fn empty_input_runs_nothing() {
    assert_eq!(
        count(r#"g.V().has("name", "nobody").union(__.count()).toList().len()"#),
        0
    );
    assert_eq!(
        count(r#"g.V().has("name", "nobody").union(__.count()).count().next()"#),
        0
    );
    assert_eq!(
        count(
            r#"g.V().has("name", "nobody").choose(__.identity(), __.count(), __.count()).toList().len()"#
        ),
        0
    );
}

#[test]
fn local_child_inside_a_batched_union_stays_per_traverser() {
    assert_eq!(
        count("g.V().union(__.local(__.out().limit(1)), __.count()).count().next()"),
        // 6 vertices: local limit gives one out per vertex that has one (marko, josh, peter = 3), plus count = 1
        4
    );
}

#[test]
fn repeat_body_union_with_limit_sees_the_frontier() {
    assert_eq!(
        count(
            r#"g.V().repeat(__.union(__.constant("y").limit(1), __.identity())).times(2).count().next()"#
        ),
        8
    );
}

#[test]
fn batched_union_profile_reports_whole_stream() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let text = eval_value(graph, "g.V().union(__.out().count()).profile().to_string()")
        .map(|d| d.to_string())
        .unwrap();
    let line = text
        .lines()
        .find(|l| l.starts_with("union("))
        .expect("union row");
    let cols: Vec<&str> = line.split_whitespace().collect();
    // ... Call In Out
    let n = cols.len();
    assert_eq!(&cols[n - 5..n - 2], ["1", "6", "1"], "{line}");
}

#[test]
fn bulk_merge_is_invisible_to_batched_branches() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let go = |merge: bool| {
        let g = lock.traversal().with("bulk.merge", merge);
        let mut v = g
            .v(None)
            .out(None)
            .union(vec![
                __::in_(None).limit(2),
                __::out(None).count(),
                __::identity(),
            ])
            .to_list()
            .unwrap();
        v.sort_by_key(|p| format!("{p:?}"));
        v
    };
    assert_eq!(go(false), go(true));
}
