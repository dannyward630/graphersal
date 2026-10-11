//! Test-only panic injection for the unwind guards of the units (`close_on_unwind`): the engine
//! never panics on purpose, so the tests arm a probe that panics at the end of the N-th
//! traversal run of the current thread, after its steps (and their writes) ran. Compiled only
//! into the crate's own unit tests.

use std::cell::Cell;

thread_local! {
    /// Runs left before the probe panics; `0` is disarmed.
    static RUNS_LEFT: Cell<u32> = const { Cell::new(0) };
}

/// The panic message of the probe.
pub(crate) const PROBE_PANIC: &str = "unwind probe: injected panic after the steps of a run";

/// Arms the probe: the `nth` traversal run (1-based) of this thread panics after its steps.
pub(crate) fn panic_after_run(nth: u32) {
    RUNS_LEFT.with(|left| left.set(nth));
}

/// Disarms the probe.
pub(crate) fn disarm() {
    RUNS_LEFT.with(|left| left.set(0));
}

/// Called by `GraphTraversalSource::run` after the steps ran: panics when the armed run is
/// reached.
pub(crate) fn after_steps() {
    let fire = RUNS_LEFT.with(|left| match left.get() {
        0 => false,
        1 => {
            left.set(0);
            true
        }
        n => {
            left.set(n - 1);
            false
        }
    });
    if fire {
        std::panic::panic_any(PROBE_PANIC);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// Runs `f`, which must panic with the probe; the probe is disarmed afterwards.
    fn expect_probe_panic<T>(f: impl FnOnce() -> T) {
        let outcome = catch_unwind(AssertUnwindSafe(f));
        disarm();
        let payload = match outcome {
            Ok(_) => panic!("the probe did not fire"),
            Err(payload) => payload,
        };
        assert_eq!(payload.downcast_ref::<&str>(), Some(&PROBE_PANIC));
    }

    #[test]
    fn a_panicking_run_rolls_its_writes_back() {
        let mut graph = TraversalGraph::new();
        panic_after_run(1);
        expect_probe_panic(|| {
            graph
                .traversal_mut()
                .add_v("ghost")
                .property("k", 1)
                .to_list()
        });
        assert_eq!(graph.vertex_count(), 0);
        // The graph is usable and no unit is left open: a later traversal commits normally.
        graph.traversal_mut().add_v("kept").to_list().unwrap();
        assert_eq!(graph.vertex_count(), 1);
        assert_eq!(graph.undo_depth(), 0);
    }

    #[test]
    fn a_panicking_execute_rolls_its_writes_back() {
        let mut graph = TraversalGraph::new();
        panic_after_run(1);
        expect_probe_panic(|| graph.traversal_mut().add_v("ghost").execute());
        assert_eq!(graph.vertex_count(), 0);
        assert_eq!(graph.undo_depth(), 0);
    }

    #[test]
    fn a_panic_in_a_transaction_rolls_the_whole_transaction_back() {
        let mut graph = TraversalGraph::new();
        panic_after_run(2);
        expect_probe_panic(|| {
            graph.transaction(|g| -> TraverserResult<()> {
                g.traversal_mut().add_v("first").to_list()?;
                g.traversal_mut().add_v("second").to_list()?;
                Ok(())
            })
        });
        assert_eq!(
            graph.vertex_count(),
            0,
            "the committed savepoint is undone too"
        );
        assert_eq!(graph.undo_depth(), 0);
    }

    #[test]
    fn a_panic_in_a_dry_run_discards_it() {
        let mut graph = TraversalGraph::new();
        panic_after_run(2);
        expect_probe_panic(|| {
            graph.dry_run(|g| -> TraverserResult<()> {
                g.traversal_mut().add_v("first").to_list()?;
                g.traversal_mut().add_v("second").to_list()?;
                Ok(())
            })
        });
        assert_eq!(graph.vertex_count(), 0);
        assert_eq!(graph.undo_depth(), 0);
    }

    #[test]
    fn an_unarmed_probe_changes_nothing() {
        let mut graph = TraversalGraph::new();
        graph.traversal_mut().add_v("a").to_list().unwrap();
        assert_eq!(graph.vertex_count(), 1);
        assert_eq!(graph.undo_depth(), 0);
    }

    #[cfg(feature = "script")]
    mod script {
        use super::*;
        use crate::script::{ScriptLimits, eval_value_atomic, eval_value_dry_run};
        use std::sync::Arc;

        const SCRIPT: &str = r#"g.addV("ghost").next(); g.V().count().next()"#;

        #[test]
        fn a_panic_in_an_atomic_script_rolls_the_script_back() {
            let graph = Arc::new(GraphSource::tinkerpop_modern());
            panic_after_run(2);
            expect_probe_panic(|| {
                eval_value_atomic(
                    graph.clone(),
                    SCRIPT,
                    Default::default(),
                    &ScriptLimits::default(),
                    Arc::new(crate::graph::AllowAll),
                )
            });
            let graph = graph.read();
            assert_eq!(
                graph.vertex_count(),
                6,
                "the first statement is rolled back"
            );
            assert_eq!(graph.undo_depth(), 0);
        }

        #[test]
        fn a_panic_in_a_dry_run_script_leaves_the_graph_as_it_was() {
            let graph = Arc::new(GraphSource::tinkerpop_modern());
            panic_after_run(2);
            expect_probe_panic(|| {
                eval_value_dry_run(
                    graph.clone(),
                    SCRIPT,
                    Default::default(),
                    &ScriptLimits::default(),
                    Arc::new(crate::graph::AllowAll),
                )
            });
            let graph = graph.read();
            assert_eq!(graph.vertex_count(), 6);
            assert_eq!(graph.undo_depth(), 0);
        }
    }
}
