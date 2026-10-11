use crate::graph::GraphStorage;
use crate::traversal::bulk::{merge_saves_work, merge_traversers, merge_traversers_adaptive};
use crate::*;

/// `barrier()`: a merge point. It collects the whole incoming stream and merges equal traversers
/// into one that carries their summed bulk (TinkerPop `NoOpBarrierStep`), so every later step
/// processes each distinct traverser once instead of once per duplicate.
///
/// Merging happens only when `bulk.merge` is on and the plan does not mutate the graph; without
/// it the step passes the stream through unchanged. Either way the query returns the same
/// results: bulk is an execution detail, only the order of duplicates may change (equal
/// traversers are emitted together, at their first occurrence).
///
/// `max_size` (`barrier(n)`) is validated and shown in `.profile()`, but does not chunk: the
/// whole stream is merged at once, which is the strongest merge TinkerPop's chunked barrier
/// could reach. The optimizer's merge point is [`StepLazyBarrier`].
///
/// `consumer` is the built-in consumer of `barrier(Barrier.normSack)`: after the merge it
/// rewrites the numeric sacks of the stream, and it runs even when merging is off or the stream
/// has fewer than two traversers (see [`BarrierConsumer`]).
#[derive(Clone)]
pub(crate) struct StepBarrier {
    pub(crate) max_size: Option<u32>,
    pub(crate) consumer: Option<BarrierConsumer>,
}

/// What a `barrier(Consumer)` does with the merged stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarrierConsumer {
    /// `Barrier.normSack`: scale the numeric sacks so that they sum to 1 (bulk-weighted).
    NormSack,
}

impl fmt::Display for StepBarrier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.consumer, self.max_size) {
            (Some(BarrierConsumer::NormSack), _) => write!(f, "barrier(Barrier.normSack)"),
            (None, None) => write!(f, "barrier()"),
            (None, Some(n)) => write!(f, "barrier({n})"),
        }
    }
}

impl StepBarrier {
    /// The error of `barrier(0)`: a barrier holds at least one traverser.
    fn size_error(size: u32) -> TraverserError {
        TraverserError::Script(ScriptError::ArgumentMismatch {
            step_name: "barrier".to_string(),
            expected: "a maximum size of at least 1".to_string(),
            actual: size.to_string(),
        })
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Merges equal traversers into one that carries their summed bulk, so later steps process
    /// each distinct traverser once (TinkerPop `barrier()`).
    ///
    /// Merging is controlled by `g.with("bulk.merge", ..)`; the results are the same with and
    /// without it. It pays off before an expansion that fans out again (`out().barrier().out()`)
    /// when many paths converge on the same vertex.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = GraphSource::tinkerpop_modern();
    /// let lock = graph.read();
    /// let count = lock
    ///     .traversal()
    ///     .with("bulk.merge", true)
    ///     .v(None)
    ///     .both(None)
    ///     .barrier()
    ///     .both(None)
    ///     .count()
    ///     .to_list()
    ///     .unwrap();
    /// let unmerged = lock.traversal().v(None).both(None).both(None).count().to_list().unwrap();
    /// assert_eq!(count[0], unmerged[0]);
    /// ```
    pub fn barrier(self) -> Self {
        self.push_step(barrier_step(None))
    }

    /// `barrier(n)`: like [`barrier`](Self::barrier), with a maximum size of `n` traversers
    /// shown in `.profile()`. The stream is not chunked: all of it is merged at once. `n` must
    /// be at least 1; `0` fails with `TraverserError` when the traversal executes.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = GraphSource::tinkerpop_modern();
    /// let lock = graph.read();
    /// let count = lock.traversal().v(None).barrier_with_size(10).count().to_list().unwrap();
    /// assert_eq!(count[0].as_i64(), Some(6));
    /// ```
    pub fn barrier_with_size(self, n: u32) -> Self {
        self.push_step(barrier_step(Some(n)))
    }

    /// `barrier(Barrier.normSack)`: merges like [`barrier`](Self::barrier), then scales every
    /// numeric sack so that the sacks of the whole stream, weighted by bulk, add up to 1. The
    /// new sacks are `float64`; a `null` sack stays `null`, and a sack that is neither a number
    /// nor null fails with `OperatorFailed`. See [`with_sack`](Self::with_sack).
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = GraphSource::tinkerpop_modern();
    /// let lock = graph.read();
    /// let sacks = lock
    ///     .traversal()
    ///     .with_sack(1.0)
    ///     .v(None)
    ///     .barrier_with(Barrier::NormSack)
    ///     .sack()
    ///     .to_list()
    ///     .unwrap();
    /// assert_eq!(sacks.len(), 6);
    /// assert_eq!(sacks[0].as_f64(), Some(1.0 / 6.0));
    /// ```
    pub fn barrier_with(self, barrier: Barrier) -> Self {
        self.push_step(consumer_step(barrier))
    }
}

fn barrier_step<'g, S: GraphStorage + 'g>(max_size: Option<u32>) -> GraphStep<'g, S> {
    GraphStep::FlatBarrier(
        "barrier",
        Box::new(StepBarrier {
            max_size,
            consumer: None,
        }),
    )
}

fn consumer_step<'g, S: GraphStorage + 'g>(barrier: Barrier) -> GraphStep<'g, S> {
    let consumer = match barrier {
        Barrier::NormSack => BarrierConsumer::NormSack,
    };
    GraphStep::FlatBarrier(
        "barrier",
        Box::new(StepBarrier {
            max_size: None,
            consumer: Some(consumer),
        }),
    )
}

impl __ {
    /// See [`GraphTraversalSource::barrier`].
    pub fn barrier<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().barrier()
    }

    /// See [`GraphTraversalSource::barrier_with_size`].
    pub fn barrier_with_size<'g, S: GraphStorage + 'g>(n: u32) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().barrier_with_size(n)
    }

    /// See [`GraphTraversalSource::barrier_with`].
    pub fn barrier_with<'g, S: GraphStorage + 'g>(barrier: Barrier) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().barrier_with(barrier)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// See [`GraphTraversalSource::barrier`].
    pub fn barrier(self) -> Self {
        self.push_step(barrier_step(None))
    }

    /// See [`GraphTraversalSource::barrier_with_size`].
    pub fn barrier_with_size(self, n: u32) -> Self {
        self.push_step(barrier_step(Some(n)))
    }

    /// See [`GraphTraversalSource::barrier_with`].
    pub fn barrier_with(self, barrier: Barrier) -> Self {
        self.push_step(consumer_step(barrier))
    }
}

/// `lazy_barrier()`: the merge point the `lazy_barrier` optimizer rule inserts before an
/// adjacency step (`next`, a copy of it). It merges only where that pays: a stream of at least
/// 4096 traversers is merged when the duplicates' fan-out on `next` saves at least one output per
/// two traversers ([`merge_saves_work`]), and then still gives up when almost nothing merges
/// ([`merge_traversers_adaptive`]). Passes the stream through while a sack merge operator or
/// `bulk.one` makes merging visible.
pub(crate) struct StepLazyBarrier<'g, S: GraphStorage + 'g> {
    next: Box<dyn GraphStepFlatMap<'g, S> + 'g>,
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepLazyBarrier<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lazy_barrier()")
    }
}

/// The merge point the `lazy_barrier` optimizer rule inserts before the adjacency step `next`.
pub(crate) fn lazy_barrier_step<'g, S: GraphStorage + 'g>(
    next: Box<dyn GraphStepFlatMap<'g, S> + 'g>,
) -> GraphStep<'g, S> {
    GraphStep::FlatBarrier("barrier", Box::new(StepLazyBarrier { next }))
}

impl<'g, S: GraphStorage + 'g> GraphStepFlatBarrier<'g, S> for StepLazyBarrier<'g, S> {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Vec<Traverser<S>>,
    ) -> TraverserResult<Option<Vec<Traverser<S>>>> {
        if !context.auto_merge
            || !context.bulk_merge
            || context.path_mode.is_full()
            || !merge_saves_work(context, &source, self.next.as_ref())
        {
            return Ok(Some(source));
        }
        Ok(Some(
            merge_traversers_adaptive(context, source, "lazy_barrier")?.0,
        ))
    }

    fn clone_box(&self) -> Box<dyn GraphStepFlatBarrier<'g, S> + 'g> {
        Box::new(StepLazyBarrier {
            next: self.next.clone_box(),
        })
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepFlatBarrier<'g, S> for StepBarrier {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Vec<Traverser<S>>,
    ) -> TraverserResult<Option<Vec<Traverser<S>>>> {
        if self.max_size == Some(0) {
            return Err(Box::new(Self::size_error(0)));
        }
        let merged = merge_traversers(context, source, "barrier")?;
        let mut merged = merged;
        match self.consumer {
            Some(BarrierConsumer::NormSack) => norm_sack(context, &mut merged)?,
            None => {}
        }
        Ok(Some(merged))
    }

    fn clone_box(&self) -> Box<dyn GraphStepFlatBarrier<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

/// The sack as an `f64` for `normSack`; `None` for anything that is not a number.
fn sack_number(value: &ElementProperty) -> Option<f64> {
    match value {
        ElementProperty::Number(n) => n.as_i64().map(|i| i as f64).or_else(|| n.as_f64()),
        _ => None,
    }
}

/// The failure of `normSack` over a sack that is neither a number nor null.
fn norm_sack_error(value: &ElementProperty) -> TraverserError {
    ValueError::OperatorFailed {
        operator: "normSack",
        kind: OperatorFailure::IncompatibleOperands { expected: "number" },
        left: crate::value::operator::describe(value),
        right: "the sum of the sacks (float64)".to_string(),
    }
    .into()
}

/// `Barrier.normSack` (TinkerPop `SackFunctions.Barrier.normSack`): `total = 0.0 + sum(sack * bulk)`
/// over the numeric sacks, then every numeric sack becomes `sack / total` (always `float64`).
/// A `null` sack is skipped and stays `null`. Division by a zero total follows IEEE (no error).
///
/// TinkerPop's numerator is `sack * bulk`, which only differs from `sack` where a merge produced
/// a bulk above 1. With a merge operator (`withSack(initial, Operator)`) that bulk comes from
/// TinkerPop's own semantic merge, so the numerator is `sack * bulk` there. Without one,
/// TinkerPop never merges sack-carrying traversers, while this engine merges equal ones
/// invisibly; the numerator is then `sack`, so the answer does not depend on whether equal
/// traversers happened to be merged (documented deviation).
fn norm_sack<S: GraphStorage>(
    context: &mut TraverserContext<'_, S>,
    stream: &mut [Traverser<S>],
) -> TraverserResult<()> {
    let with_operator = context.sack_merge.is_some();
    let mut total = 0.0_f64;
    for traverser in stream.iter() {
        context.tick()?;
        let sack = context.sacks.get(traverser.sack);
        match sack {
            ElementProperty::Null => {}
            other => match sack_number(other) {
                Some(n) => total += n * traverser.bulk as f64,
                None => return Err(Box::new(norm_sack_error(other))),
            },
        }
    }
    for traverser in stream.iter_mut() {
        context.tick()?;
        let Some(n) = sack_number(context.sacks.get(traverser.sack)) else {
            continue;
        };
        let numerator = if with_operator {
            n * traverser.bulk as f64
        } else {
            n
        };
        traverser.sack = context
            .sacks
            .intern(ElementProperty::from(numerator / total))?;
    }
    Ok(())
}
