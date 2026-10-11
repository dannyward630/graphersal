//! Cooperative cancellation (`CancelToken`): a traversal running inside a token's scope stops at
//! its next deadline check once the token is cancelled, with `TraverserError::Cancelled`, and its
//! unit rolls back. The dev server's Stop button (`POST /api/cancel`) is built on it.

use std::time::Duration;

use graphersal::exec::CancelToken;
use graphersal::prelude::*;

/// A traversal that adds a vertex and then loops for a long time.
fn add_then_loop(graph: &mut TraversalGraph) -> TraverserResult<Vec<ElementProperty>> {
    graph
        .traversal_mut()
        .inject(vec![ElementProperty::from(1)])
        .side_effect_t(__::add_v("ghost"))
        .repeat(__::identity())
        .times(1_000_000_000)
        .count()
        .to_list()
}

#[test]
fn a_cancelled_token_stops_the_next_traversal_of_its_scope() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let token = CancelToken::new();
    token.cancel();
    let err = token
        .scope(|| lock.traversal().v(None).out(None).to_list())
        .unwrap_err();
    assert!(
        matches!(err.root_cause(), TraverserError::Cancelled { .. }),
        "{err:?}"
    );
    let diagnostic = graphersal::error::format_diagnostic(&*err);
    assert!(diagnostic.contains("cancelled"), "{diagnostic}");
    assert!(diagnostic.contains("rolled back"), "{diagnostic}");
    // Outside the scope the token does not apply; after a reset it lets the query run.
    assert_eq!(lock.traversal().v(None).to_list().unwrap().len(), 6);
    token.reset();
    assert_eq!(
        token
            .scope(|| lock.traversal().v(None).to_list())
            .unwrap()
            .len(),
        6
    );
}

#[test]
fn cancelling_from_another_thread_stops_a_running_traversal_and_rolls_it_back() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let token = CancelToken::new();
    let remote = token.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        remote.cancel();
    });
    let result = token.scope(|| add_then_loop(&mut graph));
    canceller.join().unwrap();
    let err = result.unwrap_err();
    assert!(
        matches!(err.root_cause(), TraverserError::Cancelled { .. }),
        "{err:?}"
    );
    // The vertex the traversal added before it was cancelled is gone again.
    assert_eq!(graph.vertex_count(), 6);
}

#[test]
fn scopes_nest_and_restore_the_outer_token() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let outer = CancelToken::new();
    let inner = CancelToken::new();
    outer.cancel();
    outer.scope(|| {
        // The inner (not cancelled) token applies inside its scope ...
        assert!(inner.scope(|| lock.traversal().v(None).to_list()).is_ok());
        // ... and the outer one again after it.
        assert!(lock.traversal().v(None).to_list().is_err());
    });
}
