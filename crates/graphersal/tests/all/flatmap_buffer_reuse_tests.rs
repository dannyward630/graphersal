//! Regression tests for the `GraphStepFlatMap::process()` buffer-ownership rewrite (D5 of the
//! `2026-09-23-flatmap-process-buffer-reuse` spec): steps write into a caller-owned `output`
//! buffer instead of returning a freshly allocated `Vec`. These tests exercise the concrete risk
//! areas the spec calls out by name rather than just checking the change compiles:
//! - `union()`'s branch loop extending the shared `output` buffer instead of its own local `Vec`;
//! - `coalesce()`'s early-return-on-first-nonempty-branch behavior with several branches, where
//!   branches tried before the winning one pushed nothing to `output`;
//! - multiple root traversers feeding a branching `FlatMap` step, which stresses `source.rs`'s
//!   `result[before..]`-scoped path bookkeeping (site 1) across several `process()` calls sharing
//!   one accumulator.

use graphersal::prelude::*;

/// `union()` with three branches, run over multiple root traversers at once, so the shared
/// `output` accumulator in `source.rs`'s main loop grows across several `process()` calls in one
/// step execution (not just within a single call's branch loop).
#[test]
fn union_three_branches_over_multiple_roots() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let names = g
        .v(vec!["1", "4"])
        .union(vec![__::out("knows"), __::out("created"), __::identity()])
        .values("name")
        .to_list()
        .unwrap();

    let mut name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();
    name_strs.sort();

    // v("1") = marko: knows -> vadas, josh; created -> lop; identity -> marko.
    // v("4") = josh: knows -> (none); created -> ripple, lop; identity -> josh.
    let mut expected = vec!["vadas", "josh", "lop", "marko", "ripple", "lop", "josh"];
    expected.sort();
    assert_eq!(name_strs, expected);
}

/// `union()`'s own path tracking (`extends_path() == false`, since each branch's nested
/// traversal already extended the path) still works once its local accumulator is gone and every
/// branch extends the caller's shared `output` buffer directly.
#[test]
fn union_records_paths_for_every_branch_and_every_root() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let paths = g
        .v(vec!["1", "4"])
        .union(vec![__::out("knows"), __::out("created")])
        .path()
        .to_list()
        .unwrap();

    // v("1"): knows -> {vadas, josh} (2), created -> {lop} (1) = 3 paths.
    // v("4"): knows -> {} (0), created -> {ripple, lop} (2) = 2 paths.
    assert_eq!(paths.len(), 5);
    for p in &paths {
        match p {
            ElementProperty::Array(items) => {
                assert_eq!(items.len(), 2, "path should hold root + branch result")
            }
            other => panic!("expected a path array, got {other:?}"),
        }
    }
}

/// `coalesce()` with three branches where the first two produce nothing: confirms the winning
/// branch's items land in `output` and the empty branches tried before it leave no stray state
/// (D5's coalesce concern).
#[test]
fn coalesce_skips_two_empty_branches_before_a_non_empty_third() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let names = g
        .v("1")
        .coalesce(vec![
            __::out_e("bogus1"),
            __::out_e("bogus2"),
            __::out_e("knows"),
        ])
        .in_v()
        .values("name")
        .to_list()
        .unwrap();

    let name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();
    assert_eq!(name_strs, vec!["josh", "vadas"]);
}

/// The same three-branch `coalesce()`, run over two root traversers, so the winning branch for
/// one root and the (all-empty, so passthrough-less) result for another share the main loop's one
/// accumulator across two `process()` calls.
#[test]
fn coalesce_over_multiple_roots_only_the_matching_root_contributes() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // v("2") = vadas has no outgoing edges at all, so every branch is empty for it.
    let names = g
        .v(vec!["1", "2"])
        .coalesce(vec![
            __::out_e("bogus1"),
            __::out_e("bogus2"),
            __::out_e("knows"),
        ])
        .in_v()
        .values("name")
        .to_list()
        .unwrap();

    let name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();
    assert_eq!(name_strs, vec!["josh", "vadas"]);
}

/// `optional()` producing more than one item (the traversal branch is non-empty, so its full
/// result list is extended into `output`, not just a single passthrough item).
#[test]
fn optional_extends_output_with_every_item_of_a_multi_item_branch() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // v("4") = josh has two outgoing edges (created -> ripple, lop).
    let names = g
        .v("4")
        .optional(__::out(None))
        .values("name")
        .to_list()
        .unwrap();

    let mut name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();
    name_strs.sort();
    assert_eq!(name_strs, vec!["lop", "ripple"]);
}

/// `optional()` over multiple roots, mixing a root whose branch is empty (passthrough: the
/// source item itself) with one whose branch produces several items, so the main loop's shared
/// `output` buffer holds a 1-item contribution and a multi-item contribution back to back.
#[test]
fn optional_over_multiple_roots_mixes_passthrough_and_multi_item_results() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    // v("5") = ripple (software) has no outgoing edges: optional() passes it through unchanged.
    // v("4") = josh has two outgoing edges: optional() replaces it with both targets.
    let names = g
        .v(vec!["5", "4"])
        .optional(__::out(None))
        .values("name")
        .to_list()
        .unwrap();

    let mut name_strs: Vec<String> = names
        .iter()
        .filter_map(|v| v.as_string().map(String::from))
        .collect();
    name_strs.sort();
    assert_eq!(name_strs, vec!["lop", "ripple", "ripple"]);
}

/// D3/D5: `coalesce()`/`optional()`/`union()` have no own `count()` override, so `.count()` after
/// them goes through `GraphStepFlatMap`'s new default (`process()` into a scratch buffer, then
/// `.len()`) — meaning they still run their full nested traversal(s) to produce a count, exactly
/// as before this spec. This is a "confirm, don't assume" check for D5's last bullet, not a
/// behavior change.
#[test]
fn coalesce_optional_union_count_still_runs_the_full_nested_traversal() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();

    let coalesce_count = g_lock
        .traversal()
        .v("1")
        .coalesce(vec![
            __::out_e("bogus1"),
            __::out_e("bogus2"),
            __::out_e("knows"),
        ])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(coalesce_count.as_i64().unwrap(), 2);

    let optional_count = g_lock
        .traversal()
        .v("4")
        .optional(__::out(None))
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(optional_count.as_i64().unwrap(), 2);

    let union_count = g_lock
        .traversal()
        .v("1")
        .union(vec![__::out("knows"), __::out("created")])
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(union_count.as_i64().unwrap(), 3);
}
