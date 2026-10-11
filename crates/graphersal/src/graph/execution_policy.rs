//! [`ExecutionPolicy`]: a host-set, immutable-after-construction ceiling on which `g.with()`
//! execution options a query may set, and what their defaults are when it doesn't.
//!
//! `evaluationTimeout` and `repeat.max_loops` exist to bound a runaway query's resource use, but
//! both are ordinary `g.with()` keys read fresh from the query's own options every execution —
//! nothing stops the query itself from overriding them to a huge value and then submitting a
//! `repeat()`-bomb. A host embedding this engine somewhere untrusted input can reach (a public web
//! playground, a multi-tenant backend) needs a way to fix these ceilings *outside* the query's
//! reach. [`crate::GraphTraversalSource`] can't hold that ceiling — it's constructed fresh per
//! query/session and its `options` map is exactly what the untrusted query populates — so it lives
//! on the storage instance itself instead, set once by the trusted host before any query ever
//! touches it. See [`crate::graph::GraphStorage::execution_policy`].

use compact_str::CompactString;
use std::collections::{HashMap, HashSet};

use crate::value::ElementProperty;

/// Host-set, immutable-after-construction ceiling on which `g.with()` options a query may set,
/// and what their defaults are when it doesn't.
///
/// Built once, by a trusted host, before the owning storage instance (typically
/// [`crate::TraversalGraph`]) is ever shared with untrusted code — see
/// [`crate::TraversalGraph::with_execution_policy`]. There is deliberately no setter anywhere
/// else: not on [`crate::GraphTraversalSource`] (which is what an untrusted query controls), and
/// not on the storage instance after construction, so even a caller holding a write lock on the
/// graph's data cannot change the policy a query executes under.
///
/// Two independent knobs per key:
/// - **`defaults`**: applied when the query's own `g.with()` doesn't set that key. Takes
///   precedence over the engine's own hardcoded default (e.g. `evaluationTimeout`'s engine
///   default is "no timeout"; a host's policy default could be `2000` instead), but a query's own
///   value still wins for any key that isn't locked.
/// - **`locked`**: a query's own `g.with()` attempt to set this key at all is a hard
///   [`crate::TraverserError::LockedOption`], regardless of the value it tried to set. The
///   policy's default (or, absent one, the engine's hardcoded default) always applies instead.
///
/// V1 is binary per key — a range-constrained "lock evaluationTimeout to at most 5000 but let the
/// query set anything below that" is a plausible future extension, not implemented here.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExecutionPolicy {
    pub(crate) defaults: HashMap<CompactString, ElementProperty>,
    pub(crate) locked: HashSet<CompactString>,
}

impl ExecutionPolicy {
    /// No defaults overridden, nothing locked: every query can set every option, exactly today's
    /// (pre-policy) behavior. This is the implicit policy for any [`crate::graph::GraphStorage`]
    /// that doesn't override [`crate::graph::GraphStorage::execution_policy`], and for a
    /// [`crate::TraversalGraph`] built without calling
    /// [`crate::TraversalGraph::with_execution_policy`].
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    /// use graphersal::exec::ExecutionPolicy;
    ///
    /// let policy = ExecutionPolicy::permissive();
    /// assert!(!policy.is_locked("evaluationTimeout"));
    /// assert!(policy.default_for("evaluationTimeout").is_none());
    /// ```
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Builder-style: sets the default applied to `key` when a query's own `g.with()` omits it.
    /// Does not lock `key` — pair with [`Self::with_locked`] if the query must not be able to
    /// override it either.
    pub fn with_default(
        mut self,
        key: impl Into<CompactString>,
        value: impl Into<ElementProperty>,
    ) -> Self {
        self.defaults.insert(key.into(), value.into());
        self
    }

    /// Builder-style: locks `key` — any query attempt to set it via `g.with()` fails with
    /// [`crate::TraverserError::LockedOption`] when the traversal executes. Combine with
    /// [`Self::with_default`] to also fix the value every query executes under; a locked key with
    /// no policy default falls back to the engine's own hardcoded default.
    pub fn with_locked(mut self, key: impl Into<CompactString>) -> Self {
        self.locked.insert(key.into());
        self
    }

    /// Whether `key` is locked: a query's own `g.with(key, ..)` fails with `LockedOption`.
    pub fn is_locked(&self, key: &str) -> bool {
        self.locked.contains(key)
    }

    /// This policy's default for `key`, if one was set with [`Self::with_default`].
    pub fn default_for(&self, key: &str) -> Option<&ElementProperty> {
        self.defaults.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissive_locks_and_defaults_nothing() {
        let policy = ExecutionPolicy::permissive();
        assert!(!policy.is_locked("evaluationTimeout"));
        assert!(policy.default_for("evaluationTimeout").is_none());
        assert_eq!(policy, ExecutionPolicy::default());
    }

    #[test]
    fn builder_sets_defaults_and_locks_independently() {
        let policy = ExecutionPolicy::permissive()
            .with_default("evaluationTimeout", 2000)
            .with_locked("repeat.max_loops");
        assert_eq!(
            policy.default_for("evaluationTimeout"),
            Some(&ElementProperty::from(2000))
        );
        assert!(!policy.is_locked("evaluationTimeout"));
        assert!(policy.is_locked("repeat.max_loops"));
        assert!(policy.default_for("repeat.max_loops").is_none());
    }
}
