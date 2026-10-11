//! Cooperative cancellation of running traversals: [`CancelToken`].
//!
//! A host that runs queries for someone else (the dev server of `graphersal-cli`) needs a Stop
//! button that ends a running query without killing the process. A [`CancelToken`] is a shared
//! flag: another thread calls [`CancelToken::cancel`], and every traversal started inside
//! [`CancelToken::scope`] on the running thread stops at its next deadline check (where
//! `evaluationTimeout` is checked: between steps and every 1024 units of work) with
//! [`TraverserError::Cancelled`](crate::TraverserError::Cancelled). The run's unit rolls back
//! like any failing traversal, so a cancelled query leaves nothing behind.
//!
//! The token reaches the executions through the running thread (a thread-local scope), not
//! through the traversal sources, so every source a script creates (`g`, `__`,
//! `GraphSource::empty()`, ...) is covered without plumbing. A script loop that runs no
//! traversal is the host's to stop: install a Rhai `on_progress` callback that checks
//! [`CancelToken::is_cancelled`].

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    /// The token of the innermost [`CancelToken::scope`] on this thread.
    static CURRENT: RefCell<Option<CancelToken>> = const { RefCell::new(None) };
}

/// A shared cancellation flag for running traversals (see the module documentation).
///
/// Cloning shares the flag. Cancelling is sticky until [`reset`](Self::reset): a host resets it
/// before it starts the next query.
///
/// ```rust
/// use graphersal::prelude::*;
/// use graphersal::exec::CancelToken;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let token = CancelToken::new();
/// token.cancel(); // e.g. from another thread, while the query runs
/// let lock = graph.read();
/// let result = token.scope(|| lock.traversal().v(None).to_list());
/// let err = result.unwrap_err();
/// assert!(matches!(err.root_cause(), TraverserError::Cancelled { .. }));
/// token.reset();
/// assert_eq!(token.scope(|| lock.traversal().v(None).to_list()).unwrap().len(), 6);
/// ```
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// A token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation: the traversals running in a [`scope`](Self::scope) of this token
    /// stop at their next check.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    /// Whether [`cancel`](Self::cancel) was called since the last [`reset`](Self::reset).
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Clears a cancellation, so the next query runs.
    pub fn reset(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }

    /// Runs `run` with this token as the current thread's cancellation token: every traversal
    /// executed by `run` on this thread checks it. Scopes nest; the innermost token applies, and
    /// the previous one is restored afterwards (also when `run` panics).
    pub fn scope<R>(&self, run: impl FnOnce() -> R) -> R {
        struct Restore(Option<CancelToken>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                // Never panics: `try_with` fails only while the thread is being torn down.
                let _ = CURRENT.try_with(|current| {
                    if let Ok(mut current) = current.try_borrow_mut() {
                        *current = previous;
                    }
                });
            }
        }
        let previous = CURRENT
            .try_with(|current| {
                current
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut current| current.replace(self.clone()))
            })
            .ok()
            .flatten();
        let _restore = Restore(previous);
        run()
    }

    /// The token of the innermost [`scope`](Self::scope) on this thread, if any.
    pub(crate) fn current() -> Option<CancelToken> {
        CURRENT
            .try_with(|current| current.try_borrow().ok().and_then(|c| c.clone()))
            .ok()
            .flatten()
    }
}
