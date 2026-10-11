//! The `execute()` terminal: one run of a traversal handed back as an [`Execution`] (results,
//! error and, when requested, the profile) instead of a `Result`.

use crate::error::{BoxedTraverserError, TraverserError, TraverserResult};
use crate::traversal::context::ProfileType;
use crate::traversal::metrics::TraversalMetrics;
use crate::value::ElementProperty;

/// Options of [`GraphTraversalSource::execute_with`](crate::GraphTraversalSource::execute_with).
///
/// `#[non_exhaustive]`: built with [`ExecOptions::new`] / [`ExecOptions::profile`] and the
/// `with_*` methods, so later options are additive.
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let g = graph.read();
/// let exec = g
///     .traversal()
///     .v(None::<()>)
///     .has_label("person")
///     .execute_with(ExecOptions::profile(ProfileType::none()));
/// assert_eq!(exec.results().len(), 4);
/// assert!(exec.error().is_none());
/// assert_eq!(exec.profile().unwrap().metrics()[0].id(), "0");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExecOptions {
    profile: Option<ProfileType>,
}

impl ExecOptions {
    /// No profiling: the [`Execution`] carries results and error only.
    pub fn new() -> Self {
        Self::default()
    }

    /// Profile the run, collecting the extra categories in `types` (`ProfileType::none()` for
    /// timings and counts only, `ProfileType::Memory` for a memory section per step).
    pub fn profile(types: ProfileType) -> Self {
        Self::new().with_profile(types)
    }

    /// Turns profiling on with the extra categories in `types`.
    pub fn with_profile(mut self, types: ProfileType) -> Self {
        self.profile = Some(types);
        self
    }

    /// Turns profiling off again.
    pub fn without_profile(mut self) -> Self {
        self.profile = None;
        self
    }

    /// The requested profile categories, or `None` when the run is not profiled.
    pub fn profile_types(&self) -> Option<ProfileType> {
        self.profile
    }
}

/// The outcome of one run of a traversal through `execute()` / `execute_with()`.
///
/// Every error is part of the value, never a separate `Err`: a runtime error (together with
/// the profile recorded up to the failing step, when profiling was requested) as well as an
/// error that prevented the run from starting (an invalid option, a plan check, a permission
/// denial). The run is one unit of atomicity: when it failed, none of its changes are applied
/// (they are rolled back on a storage that supports it, see
/// [`GraphStorage::begin_unit`](crate::GraphStorage::begin_unit)); the profile still shows what
/// ran before the error. [`TraverserError::step_location`] tells which step failed, numbered like
/// [`Metrics::id`](crate::Metrics::id). [`Self::into_result`] converts to a `Result` for `?`.
///
/// `#[non_exhaustive]`: the home of later per-run data (the change set of the run once
/// transactions exist, warnings, truncation), added as new getters.
#[derive(Debug)]
#[non_exhaustive]
pub struct Execution {
    pub(crate) results: Vec<ElementProperty>,
    pub(crate) error: Option<BoxedTraverserError>,
    pub(crate) profile: Option<TraversalMetrics>,
    pub(crate) mutated: bool,
    pub(crate) changes: crate::graph::UnitChanges,
}

impl Execution {
    /// What `to_list()` would have returned; empty when the run failed (and then none of the
    /// run's changes are applied).
    pub fn results(&self) -> &[ElementProperty] {
        &self.results
    }

    /// The results, consuming the execution.
    pub fn into_results(self) -> Vec<ElementProperty> {
        self.results
    }

    /// The error that stopped the run, if any.
    pub fn error(&self) -> Option<&TraverserError> {
        self.error.as_deref()
    }

    /// Whether the run changed the graph and its unit kept the change.
    ///
    /// - On its own (implicit auto-commit) the run is the outermost unit: `true` when it changed
    ///   something and was committed (the commit sequence number advanced).
    /// - Nested in an enclosing unit (inside
    ///   [`Transactional::transaction`](crate::graph::Transactional::transaction) or
    ///   [`dry_run`](crate::graph::Transactional::dry_run), or in a whole-script unit of the script
    ///   DSL): `true` when it changed something that is now part of the enclosing unit. Whether
    ///   that becomes final is decided by the enclosing unit, which may still roll it back.
    ///
    /// `false` for a read-only run, a failed or vetoed run (nothing applied), for a unit that
    /// only raised an auto-id sequence (a sequence-only commit, see
    /// [`changes`](crate::changes): no data changed; no traversal step raises a sequence
    /// without adding an element), and on a storage that tracks no units
    /// ([`GraphStorage::unit_changes`](crate::GraphStorage::unit_changes) and
    /// [`GraphStorage::last_commit_seq`](crate::GraphStorage::last_commit_seq) keep their
    /// defaults).
    pub fn mutated(&self) -> bool {
        self.mutated
    }

    /// What the run changed and kept, under the same rules as [`Self::mutated`]: `data`
    /// (vertices, edges, labels, properties) and `schema`. A traversal has no schema-changing
    /// step, so `schema` is `false` here; a whole script that calls `set_schema`/`patch_schema`
    /// reports it in its own result (the playground's `changes`, from
    /// [`GraphStorage::unit_changes`](crate::GraphStorage::unit_changes) of the script's unit).
    pub fn changes(&self) -> crate::graph::UnitChanges {
        self.changes
    }

    /// Whether the run succeeded.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// The profile, when one was requested; partial (up to the failing step) when the run
    /// failed.
    pub fn profile(&self) -> Option<&TraversalMetrics> {
        self.profile.as_ref()
    }

    /// The profile, consuming the execution.
    pub fn into_profile(self) -> Option<TraversalMetrics> {
        self.profile
    }

    /// The results, or the error for `?`.
    pub fn into_result(self) -> TraverserResult<Vec<ElementProperty>> {
        match self.error {
            Some(err) => Err(err),
            None => Ok(self.results),
        }
    }
}
