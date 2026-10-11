use crate::graph::GraphStorage;
use crate::*;
use std::sync::Arc;

/// The callback of [`GraphTraversalSource::side_effect`] (and [`AnonymousTraversal::side_effect`]):
/// called with the value of each traverser and its bulk, without transforming the stream.
///
/// The callback runs ONCE per traverser. `bulk` (always at least 1) is how many identical
/// traversers that one stands for: it is 1 unless a merge point (`barrier()`, the `repeat()`
/// frontier, an optimizer-inserted `lazy_barrier()`) merged equal traversers. A callback that
/// counts must therefore add `bulk`, not 1; then the count is the same with and without merging.
/// This matches TinkerPop, whose `sideEffect(Consumer)` sees each (bulked) traverser once.
///
/// `S` is the storage the traversal runs on: `&TraversalGraph` for `graph.read().traversal()`,
/// `&mut TraversalGraph` for `graph.write().traversal_mut()`. The value is borrowed: an element is a lazy handle, so reading its properties needs the graph
/// (a callback cannot reach it). A panic in the callback is the caller's bug; it unwinds through
/// the traversal, which rolls its unit back first (nothing it wrote stays).
///
/// ```
/// use graphersal::prelude::*;
/// use std::sync::Arc;
/// use std::sync::atomic::{AtomicU64, Ordering};
///
/// let graph = GraphSource::tinkerpop_modern();
/// let lock = graph.read();
/// let seen = Arc::new(AtomicU64::new(0));
/// let counter = seen.clone();
/// let callback: SideEffectFn<&TraversalGraph> = Box::new(move |_value, bulk| {
///     counter.fetch_add(bulk, Ordering::Relaxed);
/// });
/// lock.traversal().v(None).out(None).barrier().side_effect(callback).to_list().unwrap();
/// assert_eq!(seen.load(Ordering::Relaxed), 6);
/// ```
pub type SideEffectFn<S> = Box<dyn Fn(&TraverserValue<S>, u64) + Send + Sync>;
/// The shared, clonable form of [`SideEffectFn`] stored inside the step itself.
type SideEffectFnArc<S> = Arc<dyn Fn(&TraverserValue<S>, u64) + Send + Sync>;

struct StepSideEffect<S: GraphStorage> {
    func: SideEffectFnArc<S>,
}

impl<S: GraphStorage> Clone for StepSideEffect<S> {
    fn clone(&self) -> Self {
        Self {
            func: self.func.clone(),
        }
    }
}

impl<S: GraphStorage> fmt::Display for StepSideEffect<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "side_effect(fn)")
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Calls `func` once per traverser with its value and bulk, and passes the traverser on
    /// unchanged; see [`SideEffectFn`] for the bulk rule.
    pub fn side_effect(self, func: SideEffectFn<S>) -> Self {
        self.push_step(GraphStep::SideEffect(
            "side_effect",
            Box::new(StepSideEffect {
                func: Arc::from(func),
            }),
        ))
    }
}

impl __ {
    /// Calls `func` once per traverser with its value and bulk, and passes the traverser on
    /// unchanged. Starts an anonymous traversal; see [`SideEffectFn`] for the bulk rule.
    pub fn side_effect<'g, S: GraphStorage + 'g>(
        func: SideEffectFn<S>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().side_effect(func)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Calls `func` once per traverser with its value and bulk, and passes the traverser on
    /// unchanged; see [`SideEffectFn`] for the bulk rule.
    pub fn side_effect(self, func: SideEffectFn<S>) -> Self {
        let step = GraphStep::SideEffect(
            "side_effect",
            Box::new(StepSideEffect {
                func: Arc::from(func),
            }),
        );
        self.push_step(step)
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepSideEffect<'g, S> for StepSideEffect<S> {
    #[inline]
    fn process(
        &self,
        _context: &mut TraverserContext<'_, S>,
        source: &Traverser<S>,
    ) -> TraverserResult<()> {
        // Once per traverser object (TinkerPop): the bulk is passed, never expanded.
        (self.func)(&source.value, source.bulk);
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn GraphStepSideEffect<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

/// `sideEffect(traversal)`: runs `child` for every incoming traverser, discards what it produces
/// and lets the incoming traverser continue unchanged (same path, same bulk). Side effects inside
/// the child (`aggregate`, `property`, `addV`, `drop`, ...) are real. The child always runs to
/// completion, and its errors fail the traversal.
struct StepSideEffectTraversal<'g, S: GraphStorage + 'g> {
    child: AnonymousTraversal<'g, S>,
}

impl<'g, S: GraphStorage + 'g> Clone for StepSideEffectTraversal<'g, S> {
    fn clone(&self) -> Self {
        Self {
            child: self.child.clone(),
        }
    }
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepSideEffectTraversal<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "side_effect({})", self.child)
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `sideEffect(traversal)`: runs `child` once per incoming traverser and discards its output;
    /// the incoming traverser continues unchanged.
    ///
    /// Deviation (bulk): a child that writes side effects runs once per unit of a merged
    /// (bulk-n) traverser, TinkerPop runs it once per merged traverser object; see
    /// `users_guide/bulk_and_barriers.md`.
    ///
    /// ```
    /// use graphersal::prelude::*;
    /// let graph = GraphSource::tinkerpop_modern();
    /// let g_lock = graph.read();
    /// let g = g_lock.traversal();
    /// let n = g.v(None).side_effect_t(__::values("name")).count().next().unwrap().unwrap();
    /// assert_eq!(n.as_i64(), Some(6));
    /// ```
    pub fn side_effect_t(self, child: AnonymousTraversal<'g, S>) -> Self {
        self.push_step(GraphStep::SideEffect(
            "side_effect",
            Box::new(StepSideEffectTraversal { child }),
        ))
    }
}

impl __ {
    /// Anonymous `sideEffect(traversal)`; see [`GraphTraversalSource::side_effect_t`].
    pub fn side_effect_t<'g, S: GraphStorage + 'g>(
        child: AnonymousTraversal<'g, S>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().side_effect_t(child)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Anonymous `sideEffect(traversal)`; see [`GraphTraversalSource::side_effect_t`].
    pub fn side_effect_t(self, child: AnonymousTraversal<'g, S>) -> Self {
        self.push_step(GraphStep::SideEffect(
            "side_effect",
            Box::new(StepSideEffectTraversal { child }),
        ))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepSideEffect<'g, S> for StepSideEffectTraversal<'g, S> {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: &Traverser<S>,
    ) -> TraverserResult<()> {
        // `apply_to` runs the child once per unit of bulk when it writes side effects; its
        // output is dropped on purpose.
        self.child.apply_to(&mut *context, source)?;
        Ok(())
    }

    fn child_traversals_mut(&mut self) -> Vec<&mut crate::traversal::AnonymousTraversal<'g, S>> {
        vec![&mut self.child]
    }

    fn analyze_nested_paths(
        &mut self,
        analysis: &crate::traversal::optimizer::path_analysis::PathAnalysis,
        _downstream: &crate::traversal::optimizer::path_analysis::PathRequirement,
    ) -> crate::traversal::optimizer::path_analysis::PathRequirement {
        // The child's output is discarded, so it never continues the path downstream.
        self.child.analyze_paths(
            analysis,
            crate::traversal::optimizer::path_analysis::PathRequirement::None,
        )
    }

    fn clone_box(&self) -> Box<dyn GraphStepSideEffect<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .v(None)
            .side_effect(Box::new(move |_, _| {}))
            .count()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(result.as_i64(), Some(6));
    }

    #[test]
    fn where_first() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .v(None)
            .where_t(__::side_effect(Box::new(move |_, _| {})))
            .count()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(result.as_i64(), Some(6));
    }

    #[test]
    fn where_second() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .v(None)
            .where_t(__::both(None).side_effect(Box::new(move |_, _| {})))
            .count()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(result.as_i64(), Some(6));
    }

    #[test]
    fn profile() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .v(None)
            .side_effect(Box::new(move |_, _| {}))
            .count()
            .profile()
            .unwrap();
        assert!(result.to_string().contains("side_effect("));
    }
}
