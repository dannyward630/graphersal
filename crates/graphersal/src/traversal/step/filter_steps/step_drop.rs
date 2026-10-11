use crate::graph::GraphStorage;
use crate::*;

#[derive(Default, Clone)]
struct StepDrop {}

impl fmt::Display for StepDrop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "drop()")
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Deletes the incoming element or property handle and emits nothing. To remove properties
    /// and keep the element in the stream use [`GraphTraversalSource::remove_property`].
    pub fn drop(self) -> Self {
        self.push_step(GraphStep::Filter("drop", Box::new(StepDrop {})))
    }
}

impl __ {
    /// Removes the incoming vertices, edges or properties from the graph (a vertex takes its edges
    /// with it) and yields nothing (see the book page Dropping Elements). Starts an anonymous
    /// traversal (`__.drop()`).
    pub fn drop<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().drop()
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Deletes the incoming element or property handle and emits nothing. To remove properties
    /// and keep the element in the stream use [`GraphTraversalSource::remove_property`].
    pub fn drop(self) -> Self {
        let step = GraphStep::Filter("drop", Box::new(StepDrop {}));
        self.push_step(step)
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepFilter<'g, S> for StepDrop {
    #[inline]
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: &Traverser<S>,
    ) -> TraverserResult<bool> {
        match &source.value {
            TraverserValue::VertexProperty(prop, node_index, _) => {
                context.storage.drop_vertex_property(prop, *node_index)?;
                Ok(false)
            }
            TraverserValue::Vertex(node_index) => {
                context.storage.drop_vertex(*node_index)?;
                Ok(false)
            }
            TraverserValue::Edge(edge_index) => {
                context.storage.drop_edge(*edge_index)?;
                Ok(false)
            }
            TraverserValue::EdgeProperty(prop, edge_index, _) => {
                context.storage.drop_edge_property(prop, *edge_index)?;
                Ok(false)
            }

            // Nothing else is removable; the error names the value's real kind (`int64`,
            // `array<..>`, `path`, `object`, `pathProperty`, ...).
            other @ (TraverserValue::Computed(_, _)
            | TraverserValue::Path(_)
            | TraverserValue::PathProperty(_)
            | TraverserValue::Entry(_)
            | TraverserValue::Object(_)
            | TraverserValue::Record { .. }
            | TraverserValue::Array(_)
            | TraverserValue::Schema(_)
            | TraverserValue::Graph(_)) => Err(Box::new(TraverserError::cast_exception_str(
                &["vertex", "vertexProperty", "edge", "edgeProperty"],
                other.kind_name(),
            ))),
        }
    }

    fn clone_box(&self) -> Box<dyn GraphStepFilter<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
