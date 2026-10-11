use crate::graph::GraphStorage;
use crate::*;

/// `filter(traversal)` (TinkerPop's `TraversalFilterStep`): keeps a traverser when the child
/// traversal yields at least one result for it.
///
/// It is **not** an exact alias of `where(traversal)`: `where()` treats a leading or trailing
/// `as()` of its child as a scope label (start / end label) and also takes a predicate; here
/// `as()` inside the child is an ordinary step label of the child's own path, with no
/// start/end-label semantics. For a plain child (no leading or trailing `as()`) the two are
/// equivalent.
struct StepFilter<'g, S: GraphStorage + 'g> {
    traversal: AnonymousTraversal<'g, S>,
}

impl<'g, S: GraphStorage + 'g> Clone for StepFilter<'g, S> {
    fn clone(&self) -> Self {
        Self {
            traversal: self.traversal.clone(),
        }
    }
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepFilter<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "filter({})", self.traversal)
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `filter(traversal)`: keeps the traversers for which `traversal` yields at least one
    /// result. Unlike `where_t`, a leading or trailing `as()` of the child is an ordinary
    /// step label, not a scope label.
    pub fn filter_t(self, traversal: AnonymousTraversal<'g, S>) -> Self {
        self.push_step(GraphStep::Filter(
            "filter",
            Box::new(StepFilter { traversal }),
        ))
    }
}

impl __ {
    /// Anonymous `filter(traversal)`, see [`GraphTraversalSource::filter_t`].
    pub fn filter_t<'g, S: GraphStorage + 'g>(
        traversal: AnonymousTraversal<'g, S>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().filter_t(traversal)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `filter(traversal)` on an anonymous traversal, see [`GraphTraversalSource::filter_t`].
    pub fn filter_t(self, traversal: AnonymousTraversal<'g, S>) -> Self {
        self.push_step(GraphStep::Filter(
            "filter",
            Box::new(StepFilter { traversal }),
        ))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepFilter<'g, S> for StepFilter<'g, S> {
    #[inline]
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: &Traverser<S>,
    ) -> TraverserResult<bool> {
        self.traversal.exists(context, source)
    }

    fn child_traversals_mut(&mut self) -> Vec<&mut crate::traversal::AnonymousTraversal<'g, S>> {
        vec![&mut self.traversal]
    }

    fn analyze_nested_paths(
        &mut self,
        analysis: &crate::traversal::optimizer::path_analysis::PathAnalysis,
        _downstream: &crate::traversal::optimizer::path_analysis::PathRequirement,
    ) -> crate::traversal::optimizer::path_analysis::PathRequirement {
        // Existence test only: the child's output does not continue the path.
        self.traversal.analyze_paths(
            analysis,
            crate::traversal::optimizer::path_analysis::PathRequirement::None,
        )
    }

    fn clone_box(&self) -> Box<dyn GraphStepFilter<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
