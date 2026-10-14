//! `tree("a")` is a side effect: the traversers pass through unchanged and the paths accumulate
//! into the tree `a` (TinkerPop `TreeSideEffectStep`); only `tree()` is the reducing barrier.
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

fn text(script: &str) -> String {
    format!("{:?}", run(script))
}

#[test]
fn the_stream_continues_after_a_keyed_tree() {
    for q in [
        r#"g.V().out().tree("a").count().next()"#,
        r#"g.v().out().tree("a").count().next()"#,
    ] {
        assert_eq!(count(q), 6, "{q}");
    }
    // Six rows, each the size of the tree (three roots).
    let rows = run(r#"g.V().out().tree("a").select("a").count(Scope.local).toList()"#)
        .into_array()
        .unwrap();
    assert_eq!(rows.len(), 6);
    assert!(rows.iter().all(|r| r.as_int() == Ok(3)));
    // Without a key `tree()` is still the reducing barrier.
    assert_eq!(count(r#"g.V().out().tree().count().next()"#), 1);
}

#[test]
fn by_and_expansion_after_a_keyed_tree() {
    let direct = text(r#"g.V("1").out().out().tree("a").by("name").both().both().cap("a").next()"#);
    let old = text(r#"g.V("1").out().out().tree().by("name").next()"#);
    assert_eq!(direct, old);
    assert_eq!(
        count(r#"g.v("1").out().out().tree("a").by("name").both().both().count().next()"#),
        count(r#"g.v("1").out().out().both().both().count().next()"#)
    );
    let by_label =
        text(r#"g.V("1").out().out().tree("a").by(T.label).both().both().cap("a").next()"#);
    assert!(by_label.contains("person"), "{by_label}");
}

#[test]
fn two_keyed_trees_accumulate() {
    let one = text(r#"g.V("1").out().tree("a").by("name").cap("a").next()"#);
    let two =
        text(r#"g.V("1").out().tree("a").by("name").out().tree("a").by("name").cap("a").next()"#);
    assert!(one.contains("vadas") && one.contains("lop"), "{one}");
    assert!(two.contains("ripple") && two.contains("vadas"), "{two}");
}

#[test]
fn an_empty_stream_leaves_an_empty_tree() {
    let out = run(r#"g.V().hasLabel("nope").tree("a").cap("a").next()"#);
    assert_eq!(out.cast::<rhai::Map>().len(), 0);
}

#[test]
fn bulk_merging_does_not_change_the_tree() {
    let q = |merge: bool| {
        text(&format!(
            r#"g.with("bulk.merge", {merge}).V().both().both().tree("a").by("name").cap("a").next()"#
        ))
    };
    assert_eq!(q(true), q(false));
}

#[test]
fn rust_api_and_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let n = lock
        .traversal()
        .v(None)
        .out(None)
        .tree("a")
        .count()
        .to_list()
        .unwrap();
    assert_eq!(n[0].as_i64(), Some(6));
    let profile = lock
        .traversal()
        .v("1")
        .out(None)
        .tree("a")
        .profile()
        .unwrap()
        .to_string();
    assert!(profile.contains(r#"tree("a")"#), "{profile}");
    assert!(!profile.contains("tree(Some"), "{profile}");
    let plain = lock
        .traversal()
        .v("1")
        .out(None)
        .tree(None)
        .profile()
        .unwrap()
        .to_string();
    assert!(plain.contains("tree()"), "{plain}");
}

#[test]
fn a_tree_inside_a_union_child_accumulates_over_the_batch() {
    let q = r#"g.V().union(__.out().tree("a")).count().next()"#;
    assert_eq!(count(q), 6);
}

/// `local()` runs its child once per traverser, so a tree built and read inside it grows while the
/// traversers flow (the TinkerPop scenario `g_V_out_order_byXnameX_localXtreeXaX_...`).
#[test]
fn a_tree_built_and_read_inside_local_grows_per_traverser() {
    let mut rows: Vec<i64> =
        run(r#"g.V().out().local(__.tree("a").select("a").count(Scope.local)).toList()"#)
            .into_array()
            .unwrap()
            .into_iter()
            .map(|v| v.as_int().unwrap())
            .collect();
    rows.sort();
    assert_eq!(rows, vec![1, 1, 1, 2, 2, 3]);
}

/// Deviation (book `tree_side_effect.md`): a step after `local(tree("a"))` outside the local child
/// sees the finished tree, not the progressive one, because steps run on whole batches.
#[test]
fn a_step_after_local_tree_sees_the_finished_tree() {
    let rows = run(r#"g.V().out().local(__.tree("a")).select("a").count(Scope.local).toList()"#)
        .into_array()
        .unwrap();
    assert!(rows.iter().all(|r| r.as_int() == Ok(3)), "{rows:?}");
}
