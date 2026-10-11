use crate::graph::GraphStorage;
use crate::*;

/// `sack()`: the traverser's sack as its new value (TinkerPop `sack()`, `SackStep`).
///
/// A traverser without a sack (a query without `withSack`) yields `null`. The sack and the bulk
/// travel on unchanged; as a map step it adds a position to a full path like `id()` or `label()`.
///
/// **Deviation from TinkerPop:** a `Vertex` or `Edge` is not a valid sack value (see
/// [`GraphTraversalSource::with_sack`]); sacks are immutable values shared by handle, never
/// cloned per traverser.
#[derive(Default, Clone)]
struct StepSack {}

impl fmt::Display for StepSack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sack()")
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Replaces each traverser's value with its sack (`null` when there is none). See
    /// [`with_sack`](Self::with_sack).
    pub fn sack(self) -> Self {
        self.push_step(GraphStep::Map("sack", Box::new(StepSack::default())))
    }
}

impl __ {
    /// `sack()` as a child traversal: reads the sack of the traverser it runs for.
    pub fn sack<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().sack()
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Replaces each traverser's value with its sack (`null` when there is none).
    pub fn sack(self) -> Self {
        let step = GraphStep::Map("sack", Box::new(StepSack::default()));
        self.push_step(step)
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepSack {
    #[inline]
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        let value = context.sacks.get(source.sack).clone();
        Ok(Traverser::with_parts(
            TraverserValue::Computed(value, None),
            source.path,
            source.bulk,
            source.sack,
        ))
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
