//! The script group of [`AnyGraph`](crate::graph::AnyGraph): what a script terminal and a
//! whole-script unit hand to the graph (`ScriptPlan`, `Terminal`, `UnitKind`, `UnitBody`) and
//! what they get back (`TerminalOutput`, `UnitResult`), and the generic code the blanket
//! `impl AnyGraph for Graph<S>` runs for them (`run_plan_on`, `run_unit_on`).
//!
//! The types are `pub` because they appear in the sealed trait's (hidden) methods; this module
//! is private, so no other crate can name or build them. The engine is monomorphised for `&S`
//! and `&mut S` inside `run_plan_on`: one indirect call per terminal, never one per element
//! (`roadmap/generic-front-ends.md`, sections 3.3 and 5.2).

use super::*;
use crate::error::{BoxedTraverserError, GraphError, TraverserError, TraverserResult};
use crate::graph::{AnyGraph, ChangeSet, Transactional};
use crate::traversal::source::{ExecutedItems, ListItem};

/// A script plan as a terminal hands it to [`AnyGraph::run_plan`]: the script source (its steps,
/// options, access and whether its graph is private) and whether it writes (the terminal then
/// takes the write lock instead of the read lock).
#[doc(hidden)]
pub struct ScriptPlan<'a> {
    pub(crate) source: &'a ScriptGraphTraversalSource,
    pub(crate) mutating: bool,
}

/// The terminal a script runs a plan with ([`AnyGraph::run_plan`]).
#[doc(hidden)]
pub struct Terminal(pub(crate) TerminalKind);

/// The terminal kinds: one per engine terminal the script layer calls.
pub(crate) enum TerminalKind {
    /// `to_list()` with the values materialized ([`ScriptGraphTraversalSource::to_typed_list`]).
    ToList,
    /// `to_list()` keeping graph values as shared handles (`toList()`, `next()`).
    ListWithGraphs,
    /// `profile()` / `profile(flags)`.
    Profile(ProfileType),
    /// `execute()` / `execute(#{..})`.
    Execute(crate::ExecOptions),
    /// A visualizer (`to_table()`, `visualize(..)`, a rendered final value).
    Visualize {
        format: VisualizeFormat,
        options: RenderOptions,
        may_stop_early: bool,
    },
    /// `iterate()`.
    Iterate,
    /// `to_graph()`.
    ToGraph,
}

/// The storage-independent result of a [`Terminal`].
#[doc(hidden)]
pub struct TerminalOutput(pub(crate) OutputKind);

/// The results, one per [`TerminalKind`].
pub(crate) enum OutputKind {
    List(TraverserResult<Vec<ElementProperty>>),
    Items(TraverserResult<Vec<ListItem>>),
    Metrics(TraverserResult<crate::TraversalMetrics>),
    Executed(ExecutedItems<ListItem>),
    Rendered(TraverserResult<RenderedOutput>),
    Unit(TraverserResult<()>),
    Graph(TraverserResult<Arc<Graph>>),
}

/// The error of an output of another kind than the terminal asked for: never produced, because
/// `run_plan_on` answers each terminal with its own kind; a terminal reports it instead of
/// panicking (rule 1).
fn mismatch() -> BoxedTraverserError {
    Box::new(TraverserError::NotImplemented {
        feature: "a script terminal answered with another kind of result",
    })
}

impl TerminalOutput {
    pub(crate) fn into_list(self) -> TraverserResult<Vec<ElementProperty>> {
        match self.0 {
            OutputKind::List(result) => result,
            _ => Err(mismatch()),
        }
    }

    pub(crate) fn into_items(self) -> TraverserResult<Vec<ListItem>> {
        match self.0 {
            OutputKind::Items(result) => result,
            _ => Err(mismatch()),
        }
    }

    pub(crate) fn into_metrics(self) -> TraverserResult<crate::TraversalMetrics> {
        match self.0 {
            OutputKind::Metrics(result) => result,
            _ => Err(mismatch()),
        }
    }

    pub(crate) fn into_executed(self) -> ExecutedItems<ListItem> {
        match self.0 {
            OutputKind::Executed(run) => run,
            _ => ExecutedItems {
                items: Vec::new(),
                error: Some(mismatch()),
                profile: None,
                mutated: false,
                changes: Default::default(),
            },
        }
    }

    pub(crate) fn into_rendered(self) -> TraverserResult<RenderedOutput> {
        match self.0 {
            OutputKind::Rendered(result) => result,
            _ => Err(mismatch()),
        }
    }

    pub(crate) fn into_unit(self) -> TraverserResult<()> {
        match self.0 {
            OutputKind::Unit(result) => result,
            _ => Err(mismatch()),
        }
    }

    pub(crate) fn into_graph(self) -> TraverserResult<Arc<Graph>> {
        match self.0 {
            OutputKind::Graph(result) => result,
            _ => Err(mismatch()),
        }
    }
}

/// Runs `plan` with `terminal` on `graph`: lowers the plan for `&mut S` under the write lock
/// (a mutating plan) or for `&S` under the read lock, and runs the terminal. The body of
/// [`AnyGraph::run_plan`] for every `Graph<S>`.
pub(crate) fn run_plan_on<S: GraphStorage>(
    graph: &Graph<S>,
    plan: &ScriptPlan<'_>,
    terminal: Terminal,
) -> TerminalOutput {
    if plan.mutating {
        let mut storage = graph.write();
        run_terminal(plan.source.ast_convert_to_gts(&mut *storage), terminal)
    } else {
        let storage = graph.read();
        run_terminal(plan.source.ast_convert_to_gts(&*storage), terminal)
    }
}

/// Runs `terminal` on the lowered traversal `gts`.
fn run_terminal<'g, T: GraphStorage + 'g>(
    gts: GraphTraversalSource<'g, T>,
    terminal: Terminal,
) -> TerminalOutput {
    TerminalOutput(match terminal.0 {
        TerminalKind::ToList => OutputKind::List(gts.to_list()),
        TerminalKind::ListWithGraphs => OutputKind::Items(gts.into_list_with_graphs()),
        TerminalKind::Profile(flags) => OutputKind::Metrics(gts.profile_with(flags)),
        TerminalKind::Execute(options) => {
            OutputKind::Executed(gts.execute_items(&options, crate::traversal::source::list_item))
        }
        TerminalKind::Visualize {
            format,
            options,
            may_stop_early,
        } => OutputKind::Rendered(gts.visualize_bounded(format, &options, may_stop_early)),
        TerminalKind::Iterate => OutputKind::Unit(gts.iterate()),
        TerminalKind::ToGraph => OutputKind::Graph(gts.to_graph()),
    })
}

/// How [`AnyGraph::run_unit`] closes the whole-script unit.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    /// Commit when the body succeeds, roll back when it fails (`eval_value_atomic`).
    Atomic,
    /// Always roll back, without hooks, and report what the body changed
    /// (`eval_value_dry_run`).
    DryRun,
}

/// What the body of a whole-script unit returns: the script's value or its error.
#[doc(hidden)]
pub struct UnitBody(pub(crate) Result<Dynamic, Box<rhai::EvalAltResult>>);

/// What [`AnyGraph::run_unit`] returns: the body's value (with the change set of a dry run) or
/// the error that closed the unit.
#[doc(hidden)]
pub struct UnitResult(pub(crate) Result<(Dynamic, Option<ChangeSet>), Box<rhai::EvalAltResult>>);

/// The body of [`AnyGraph::run_unit`] for every `Graph<S>`: the whole-script unit of
/// `eval_value_atomic` / `eval_value_dry_run`.
///
/// The script's terminals lock their graph themselves, so the shared lock cannot simply be held
/// while they run. Instead the shared write lock is held for the whole body (nobody else sees the
/// graph in between), the storage is moved into a private `Graph<S>` (`std::mem::take`, hence
/// `S: Default`), and the body runs on that one, whose locks are uncontended. The open unit lives
/// in the storage and moves along; the storage moves back when the body returns, also when it
/// unwinds ([`RestoreGraph`]), after the unit was closed.
pub(crate) fn run_unit_on<S: GraphStorage + Default + Send + Sync + 'static>(
    graph: &Graph<S>,
    kind: UnitKind,
    body: &mut dyn FnMut(&Arc<dyn AnyGraph>) -> UnitBody,
) -> UnitResult {
    let mut shared = graph.write();
    UnitResult(match kind {
        UnitKind::Atomic => in_private_lock(graph, &mut *shared, |private, any| {
            let mark = private.write().begin_unit();
            // A panic (an engine bug) rolls the whole script back before it continues to the
            // host; the storage then moves back into the shared lock.
            let evaluated = crate::graph::storage::close_on_unwind(
                &mut (),
                |_| body(any).0,
                |_| {
                    private.write().rollback_unit(
                        mark,
                        &crate::graph::RollbackReason::failed(&crate::graph::storage::UNWOUND),
                    )
                },
            );
            match evaluated {
                Ok(value) => match private.write().commit_unit(mark) {
                    Ok(()) => Ok((value, None)),
                    Err(err) => Err(script_error(Box::new(TraverserError::from(err)))),
                },
                Err(err) => {
                    private
                        .write()
                        .rollback_unit(mark, &crate::graph::RollbackReason::failed(&err));
                    Err(err)
                }
            }
        }),
        UnitKind::DryRun => shared
            .dry_run(|unit| {
                in_private_lock(graph, unit, |_, any| body(any).0).map_err(DryRunFailure)
            })
            .map(|(value, changes)| (value, Some(changes)))
            .map_err(|failure| failure.0),
    })
}

/// The error of a script dry run: a script error, or the storage's own failure to discard the
/// unit, as a script error ([`Transactional::dry_run`] needs `E: From<GraphError>`).
struct DryRunFailure(Box<rhai::EvalAltResult>);

impl From<GraphError> for DryRunFailure {
    fn from(err: GraphError) -> Self {
        DryRunFailure(script_error(Box::new(TraverserError::from(err))))
    }
}

/// Runs `body` with the storage in `unit` moved into a private `Graph<S>` (given to `body` both
/// typed and as `Arc<dyn AnyGraph>`), and moves it back when `body` returns, also when it
/// unwinds ([`RestoreGraph`]). The caller closes its unit on unwind itself, so a panic never
/// leaves a half-applied script behind. The private graph has the dyn-boundary capabilities of
/// `template`, the shared graph (`Graph::new_like`: a persistent graph stays writable as a
/// snapshot inside the unit).
fn in_private_lock<S, R>(
    template: &Graph<S>,
    unit: &mut S,
    body: impl FnOnce(&Arc<Graph<S>>, &Arc<dyn AnyGraph>) -> R,
) -> R
where
    S: GraphStorage + Default + Send + Sync + 'static,
{
    let private: Arc<Graph<S>> = Arc::new(template.new_like(std::mem::take(unit)));
    let any: Arc<dyn AnyGraph> = private.clone();
    let _restore = RestoreGraph {
        unit,
        private: &private,
    };
    body(&private, &any)
}

/// Moves the storage of a whole-script unit back from its private lock into the shared one when
/// [`in_private_lock`] ends, also when it unwinds.
struct RestoreGraph<'a, S: GraphStorage> {
    unit: &'a mut S,
    private: &'a Arc<Graph<S>>,
}

impl<S: GraphStorage> Drop for RestoreGraph<'_, S> {
    fn drop(&mut self) {
        std::mem::swap(self.unit, &mut *self.private.write());
    }
}

/// The graph a source without one (`g: None`, such as the `mergeV()` identity child) runs on:
/// a process-wide empty scratch graph for a plan that only reads, a fresh one for a plan that
/// writes (so a write never reaches the next run).
pub(crate) fn scratch_graph(mutating: bool) -> Arc<dyn AnyGraph> {
    static EMPTY: std::sync::OnceLock<Arc<Graph>> = std::sync::OnceLock::new();
    if mutating {
        Arc::new(Graph::new(TraversalGraph::new()))
    } else {
        EMPTY
            .get_or_init(|| Arc::new(Graph::new(TraversalGraph::new())))
            .clone()
    }
}

/// Whether `a` and `b` are the same shared graph: their `Arc` DATA pointers are equal (a fat
/// `Arc<dyn AnyGraph>` pointer also carries a vtable pointer, which may differ for one graph).
pub(crate) fn same_graph<A: ?Sized, B: ?Sized>(a: &Arc<A>, b: &Arc<B>) -> bool {
    std::ptr::eq(Arc::as_ptr(a) as *const (), Arc::as_ptr(b) as *const ())
}
