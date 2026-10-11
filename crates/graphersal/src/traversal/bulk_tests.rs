//! Traverser bulk seen from the raw traversers: how many traversers a run hands back and the
//! bulk each one carries (merging at `barrier()`, the `repeat()` frontier and `lazy_barrier()`,
//! the cases that must not merge, the adaptive skip). These need the crate-internal
//! `GraphTraversalSource::internal_to_list`; the result-level checks (bulk is invisible in the
//! results) are the integration tests in `tests/all/bulk_tests.rs`.

use crate::prelude::*;

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

type Source<'b> = GraphTraversalSource<'b, &'b TraversalGraph>;

/// Runs `query` with `bulk.merge` off and on, checks both results are the same multiset and
/// returns the merged result.
fn assert_bulk_invisible_on<F>(graph: &Graph, query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let lock = graph.read();
    let run = |merge: bool| {
        query(lock.traversal().with("bulk.merge", merge))
            .to_list()
            .expect("query runs")
    };
    let unmerged = run(false);
    let merged = run(true);
    assert_eq!(unmerged.len(), merged.len(), "result sizes differ");
    let mut left: Vec<String> = unmerged.iter().map(|item| format!("{item:?}")).collect();
    let mut right: Vec<String> = merged.iter().map(|item| format!("{item:?}")).collect();
    left.sort();
    right.sort();
    assert!(
        left == right,
        "merged and unmerged results are different multisets"
    );
    merged
}

/// Three layers of four vertices, complete between neighbouring layers: every vertex of layer
/// two is reached by 4 paths from layer zero. Names are `n<layer><i>`, ages `10 * layer + i + 1`
/// (the same graph as `dense()` of the integration tests).
fn dense() -> Graph {
    let mut tg = TraversalGraph::new();
    let mut layers: Vec<Vec<_>> = Vec::new();
    for layer in 0..3 {
        let mut vertices = Vec::new();
        for i in 0..4 {
            let mut element = PropertyMap::default();
            element.insert("name", format!("n{layer}{i}").as_str().into());
            element.insert("age", ElementProperty::Number((10 * layer + i + 1).into()));
            vertices.push(
                tg.add_vertex(
                    Some(format!("l{layer}_{i}").into()),
                    vec!["n".into()],
                    element,
                )
                .unwrap(),
            );
        }
        layers.push(vertices);
    }
    for pair in layers.windows(2) {
        for from in &pair[0] {
            for to in &pair[1] {
                tg.add_edge(
                    None,
                    Some("next".into()),
                    *from,
                    *to,
                    PropertyMap::default(),
                )
                .unwrap();
            }
        }
    }
    Graph::new(tg)
}

/// `n` sources, each with one edge to its own sink (nothing converges).
fn sparse(n: usize) -> Graph {
    let mut tg = TraversalGraph::new();
    for i in 0..n {
        let mut add = |id: String, label: &str| {
            tg.add_vertex(Some(id.into()), vec![label.into()], PropertyMap::default())
                .unwrap()
        };
        let a = add(format!("a{i}"), "a");
        let b = add(format!("b{i}"), "b");
        let c = add(format!("c{i}"), "c");
        tg.add_edge(None, Some("e".into()), a, b, PropertyMap::default())
            .unwrap();
        tg.add_edge(None, Some("e".into()), b, c, PropertyMap::default())
            .unwrap();
    }
    Graph::new(tg)
}

/// `n` sources that all point at one hub, which points at one sink.
fn star(n: usize) -> Graph {
    let mut tg = TraversalGraph::new();
    let hub = tg
        .add_vertex(
            Some("hub".into()),
            vec!["hub".into()],
            PropertyMap::default(),
        )
        .unwrap();
    let sink = tg
        .add_vertex(
            Some("sink".into()),
            vec!["sink".into()],
            PropertyMap::default(),
        )
        .unwrap();
    tg.add_edge(None, Some("e".into()), hub, sink, PropertyMap::default())
        .unwrap();
    for i in 0..n {
        let a = tg
            .add_vertex(
                Some(format!("a{i}").into()),
                vec!["a".into()],
                PropertyMap::default(),
            )
            .unwrap();
        tg.add_edge(None, Some("e".into()), a, hub, PropertyMap::default())
            .unwrap();
    }
    Graph::new(tg)
}

#[test]
fn every_traverser_has_bulk_one() {
    let graph = modern();
    let lock = graph.read();
    let mut queries: Vec<Source<'_>> = vec![
        lock.traversal().v(None).out(None).out(None),
        lock.traversal().v(None).has_label("person").values("age"),
        lock.traversal().e(None),
        lock.traversal().v(None).repeat(__::out(None)).times(2),
    ];
    for query in &mut queries {
        let traversers = query.internal_to_list().unwrap();
        assert!(!traversers.is_empty());
        assert!(traversers.iter().all(|t| t.bulk() == 1));
    }
}

/// The merge pass really merges: fewer traversers, some with a bulk above 1, same total.
#[test]
fn barrier_merges_and_keeps_the_total_bulk() {
    for graph in [modern(), dense()] {
        let lock = graph.read();
        let unmerged = lock
            .traversal()
            .with("bulk.merge", false)
            .v(None)
            .both(None)
            .both(None)
            .barrier()
            .internal_to_list()
            .unwrap();
        let merged = lock
            .traversal()
            .with("bulk.merge", true)
            .v(None)
            .both(None)
            .both(None)
            .barrier()
            .internal_to_list()
            .unwrap();
        assert!(unmerged.iter().all(|t| t.bulk() == 1));
        assert!(merged.len() < unmerged.len());
        assert!(merged.iter().any(|t| t.bulk() > 1));
        assert_eq!(
            merged.iter().map(|t| t.bulk()).sum::<u64>(),
            unmerged.len() as u64
        );
    }
}

#[test]
fn barrier_is_a_pass_through_when_merging_is_off() {
    let graph = dense();
    let lock = graph.read();
    let traversers = lock
        .traversal()
        .with("bulk.merge", false)
        .v(None)
        .both(None)
        .barrier()
        .internal_to_list()
        .unwrap();
    // Two layers of 16 edges, walked in both directions.
    assert_eq!(traversers.len(), 64);
    assert!(traversers.iter().all(|t| t.bulk() == 1));
}

#[test]
fn dedup_resets_the_bulk() {
    for graph in [modern(), dense()] {
        assert_bulk_invisible_on(&graph, |g| {
            g.v(None).both(None).both(None).barrier().dedup()
        });
        assert_bulk_invisible_on(&graph, |g| {
            g.v(None).both(None).both(None).barrier().dedup().by("name")
        });
        let lock = graph.read();
        let survivors = lock
            .traversal()
            .with("bulk.merge", true)
            .v(None)
            .both(None)
            .both(None)
            .barrier()
            .dedup()
            .internal_to_list()
            .unwrap();
        assert!(survivors.iter().all(|t| t.bulk() == 1));
        // A later count sees one per distinct vertex.
        let count = lock
            .traversal()
            .with("bulk.merge", true)
            .v(None)
            .both(None)
            .both(None)
            .barrier()
            .dedup()
            .count()
            .to_list()
            .unwrap();
        assert_eq!(count[0].as_i64(), Some(survivors.len() as i64));
    }
}

#[test]
fn a_limit_cuts_a_straddling_traverser() {
    let graph = dense();
    let lock = graph.read();
    let mut query = lock
        .traversal()
        .with("bulk.merge", true)
        .v(None)
        .both(None)
        .both(None)
        .barrier()
        .limit(5);
    let traversers = query.internal_to_list().unwrap();
    assert_eq!(traversers.iter().map(|t| t.bulk()).sum::<u64>(), 5);
}

#[test]
fn full_path_mode_does_not_merge() {
    for graph in [modern(), dense()] {
        let merged =
            assert_bulk_invisible_on(&graph, |g| g.v(None).both(None).both(None).barrier().path());
        assert!(!merged.is_empty());
        let lock = graph.read();
        let traversers = lock
            .traversal()
            .with("bulk.merge", true)
            .v(None)
            .both(None)
            .both(None)
            .barrier()
            .path()
            .internal_to_list()
            .unwrap();
        assert!(traversers.iter().all(|t| t.bulk() == 1));
    }
}

#[test]
fn coin_on_a_large_bulk_rounds_the_expectation() {
    // A single vertex reached 4 * 4 * 4 * ... times: bulk above 100 takes round(p * bulk).
    let graph = dense();
    let lock = graph.read();
    let mut query = lock
        .traversal()
        .with("bulk.merge", true)
        .v(None)
        .has("name", "n00")
        .out(None)
        .in_(None)
        .out(None)
        .in_(None)
        .out(None)
        .in_(None)
        .barrier();
    let merged = query.internal_to_list().unwrap();
    let biggest = merged.iter().map(|t| t.bulk()).max().unwrap();
    assert!(biggest >= 100, "{biggest}");
    let kept = lock
        .traversal()
        .with("bulk.merge", true)
        .v(None)
        .has("name", "n00")
        .out(None)
        .in_(None)
        .out(None)
        .in_(None)
        .out(None)
        .in_(None)
        .barrier()
        .coin(0.5)
        .count()
        .to_list()
        .unwrap()[0]
        .as_i64()
        .unwrap();
    let total: u64 = merged.iter().map(|t| t.bulk()).sum();
    let expected: i64 = merged
        .iter()
        .map(|t| {
            if t.bulk() >= 100 {
                (t.bulk() as f64 * 0.5).round() as i64
            } else {
                0
            }
        })
        .sum();
    let small: u64 = merged
        .iter()
        .filter(|t| t.bulk() < 100)
        .map(|t| t.bulk())
        .sum();
    assert!(
        kept >= expected && kept <= expected + small as i64,
        "{kept} vs {expected}..+{small} of {total}"
    );
}

#[test]
fn merging_is_on_by_default() {
    let graph = dense();
    let lock = graph.read();
    let traversers = lock
        .traversal()
        .v(None)
        .both(None)
        .barrier()
        .internal_to_list()
        .unwrap();
    assert!(traversers.len() < 64);
    assert_eq!(traversers.iter().map(|t| t.bulk()).sum::<u64>(), 64);
}

#[test]
fn repeat_merges_the_frontier() {
    let graph = dense();
    let lock = graph.read();
    let merged = lock
        .traversal()
        .v(None)
        .repeat(__::out(None))
        .times(2)
        .internal_to_list()
        .unwrap();
    // Four layer-two vertices, each reached by four paths from layer zero... times four starts.
    assert_eq!(merged.len(), 4);
    assert!(merged.iter().all(|t| t.bulk() == 16));
    let dfs = lock
        .traversal()
        .with("repeat.order", "dfs")
        .v(None)
        .repeat(__::out(None))
        .times(2)
        .internal_to_list()
        .unwrap();
    assert_eq!(dfs.len(), 64);
    assert!(dfs.iter().all(|t| t.bulk() == 1));
}

#[test]
fn full_paths_prevent_the_frontier_merge() {
    let graph = dense();
    let lock = graph.read();
    let paths = lock
        .traversal()
        .v(None)
        .repeat(__::out(None))
        .times(2)
        .path()
        .internal_to_list()
        .unwrap();
    assert_eq!(paths.len(), 64);
}

#[test]
fn full_path_recording_makes_the_barrier_a_pass_through() {
    let graph = dense();
    let lock = graph.read();
    let traversers = lock
        .traversal()
        .v(None)
        .out(None)
        .out(None)
        .path()
        .internal_to_list()
        .unwrap();
    assert!(traversers.iter().all(|t| t.bulk() == 1));
    assert_eq!(traversers.len(), 4 * 4 * 4);
}

#[test]
fn a_large_stream_without_duplicates_is_passed_through_untouched() {
    let graph = sparse(6000);
    let lock = graph.read();
    let traversers = lock
        .traversal()
        .v(None)
        .has_label("a")
        .out(None)
        .out(None)
        .internal_to_list()
        .unwrap();
    assert_eq!(traversers.len(), 6000);
    assert!(traversers.iter().all(|t| t.bulk() == 1));
}

#[test]
fn a_large_stream_with_many_duplicates_still_merges_completely() {
    let graph = star(6000);
    let lock = graph.read();
    let traversers = lock
        .traversal()
        .v(None)
        .has_label("a")
        .out(None)
        .out(None)
        .internal_to_list()
        .unwrap();
    assert_eq!(traversers.len(), 1);
    assert_eq!(traversers[0].bulk(), 6000);
}

#[test]
fn an_explicit_barrier_always_merges_the_whole_stream() {
    let graph = star(6000);
    let lock = graph.read();
    let traversers = lock
        .traversal()
        .v(None)
        .has_label("a")
        .out(None)
        .barrier()
        .internal_to_list()
        .unwrap();
    assert_eq!(traversers.len(), 1);
    assert_eq!(traversers[0].bulk(), 6000);
    // Even when the first 4096 traversers are all distinct.
    let sparse_graph = sparse(5000);
    let lock = sparse_graph.read();
    let all = lock
        .traversal()
        .v(None)
        .barrier()
        .internal_to_list()
        .unwrap();
    assert_eq!(all.len(), 15000);
}
