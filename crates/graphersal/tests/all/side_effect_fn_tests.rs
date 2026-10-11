//! `side_effect(SideEffectFn)` called from outside the crate, through public paths only: the
//! callback gets each traverser's value and bulk once, and a failure after it rolls the unit back.

use graphersal::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Counts calls, sums bulks and collects the values' ids.
#[derive(Default)]
struct Seen {
    calls: AtomicU64,
    bulks: AtomicU64,
    ids: Mutex<Vec<String>>,
}

fn recorder<S: GraphStorage>(seen: &Arc<Seen>) -> SideEffectFn<S> {
    let seen = seen.clone();
    Box::new(move |value, bulk| {
        seen.calls.fetch_add(1, Ordering::Relaxed);
        seen.bulks.fetch_add(bulk, Ordering::Relaxed);
        if let TraverserValue::Vertex(_) = value {
            seen.ids.lock().unwrap().push(format!("{value:?}"));
        }
    })
}

#[test]
fn the_callback_sees_each_traverser_once_with_its_bulk() {
    let graph = GraphSource::tinkerpop_modern();
    let seen = Arc::new(Seen::default());
    let lock = graph.read();
    // v().out() yields lop three times, vadas, josh and ripple; the barrier merges the three lop
    // traversers into one with bulk 3.
    let names = lock
        .traversal()
        .v(None)
        .out(None)
        .barrier()
        .side_effect(recorder(&seen))
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(
        names.len(),
        6,
        "the stream keeps its multiplicity: {names:?}"
    );
    assert_eq!(seen.bulks.load(Ordering::Relaxed), 6);
    assert_eq!(
        seen.calls.load(Ordering::Relaxed),
        4,
        "lop once, with bulk 3"
    );
    assert_eq!(
        seen.ids.lock().unwrap().len(),
        4,
        "the values are the vertices"
    );
}

#[test]
fn without_a_merge_every_bulk_is_one() {
    let graph = GraphSource::tinkerpop_modern();
    let seen = Arc::new(Seen::default());
    let lock = graph.read();
    let count = lock
        .traversal()
        .with("bulk.merge", false)
        .v(None)
        .both(None)
        .side_effect(recorder(&seen))
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count.as_i64(), Some(12));
    assert_eq!(seen.calls.load(Ordering::Relaxed), 12);
    assert_eq!(seen.bulks.load(Ordering::Relaxed), 12);
}

#[test]
fn the_anonymous_form_is_callable_too() {
    let graph = GraphSource::tinkerpop_modern();
    let seen = Arc::new(Seen::default());
    let lock = graph.read();
    let count = lock
        .traversal()
        .v(None)
        .where_t(__::side_effect(recorder(&seen)))
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(count.as_i64(), Some(6));
    assert_eq!(seen.calls.load(Ordering::Relaxed), 6);
}

#[test]
fn a_failure_after_the_side_effect_rolls_back() {
    let graph = GraphSource::tinkerpop_modern();
    let seen = Arc::new(Seen::default());
    let result = {
        let mut lock = graph.write();
        lock.traversal_mut()
            .add_v("marker")
            .side_effect(recorder(&seen))
            .fail(Some("stop"))
            .to_list()
    };
    assert!(result.is_err(), "{result:?}");
    assert_eq!(seen.calls.load(Ordering::Relaxed), 1, "the callback ran");
    let lock = graph.read();
    assert_eq!(lock.vertex_count(), 6, "the added vertex was rolled back");
}

#[test]
fn a_panicking_callback_rolls_back_before_unwinding() {
    let graph = GraphSource::tinkerpop_modern();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut lock = graph.write();
        lock.traversal_mut()
            .add_v("marker")
            .side_effect(Box::new(|_, _| panic!("callback bug")))
            .to_list()
    }));
    assert!(outcome.is_err(), "the panic propagates");
    let lock = graph.read();
    assert_eq!(lock.vertex_count(), 6, "the added vertex was rolled back");
}
