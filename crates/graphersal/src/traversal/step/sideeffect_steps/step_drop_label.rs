use crate::graph::GraphStorage;
use crate::*;

/// `drop_label("L")`: removes a label from the label set of each incoming vertex and emits the
/// same vertex.
#[derive(Clone)]
struct StepDropLabel {
    label: CompactString,
}

impl fmt::Display for StepDropLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "drop_label({})",
            crate::value::literal::rhai_string(&self.label)
        )
    }
}

fn drop_label_step<'g, S: GraphStorage + 'g>(label: CompactString) -> GraphStep<'g, S> {
    GraphStep::SideEffectMut("drop_label", Box::new(StepDropLabel { label }))
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Removes `label` from the label set of each incoming vertex and lets the vertex continue.
    ///
    /// A Graphersal extension: TinkerPop labels are immutable and TinkerPop has no such step, so
    /// this adds to Gremlin rather than deviating from it. A vertex that does not carry the label
    /// is unchanged (no error), so a bulk-n traverser changes its vertex once. Dropping the last
    /// label leaves the vertex unlabeled, which is allowed unless the schema rejects it. With a
    /// schema active the vertex must stay valid under its remaining labels (`Closed`: an
    /// unlabeled vertex, a property no remaining label declares, or an incident edge no longer
    /// matching a declared connection is rejected; `Open`/`Closed`: the remaining labels are
    /// re-checked); a violation fails the traversal, whose unit rolls back. An edge is an error
    /// whose help names [`Self::set_label`]; any other value is an error too.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let mut graph = TraversalGraph::tinkerpop_modern();
    /// graph.traversal_mut().v("1").drop_label("person").next().unwrap();
    /// assert_eq!(
    ///     graph.traversal().v(None).has_label("person").count().next().unwrap(),
    ///     Some(ElementProperty::from(3))
    /// );
    /// ```
    pub fn drop_label(self, label: impl Into<CompactString>) -> Self {
        self.push_step(drop_label_step(label.into()))
    }
}

impl __ {
    /// Anonymous `drop_label()`; see [`GraphTraversalSource::drop_label`].
    pub fn drop_label<'g, S: GraphStorage + 'g>(
        label: impl Into<CompactString>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().drop_label(label)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Anonymous `drop_label()`; see [`GraphTraversalSource::drop_label`].
    pub fn drop_label(self, label: impl Into<CompactString>) -> Self {
        self.push_step(drop_label_step(label.into()))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepSideEffectMut<'g, S> for StepDropLabel {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Option<Traverser<S>>,
    ) -> TraverserResult<Vec<Traverser<S>>> {
        let Some(source) = source else {
            return Ok(Vec::new());
        };
        match &source.value {
            TraverserValue::Vertex(node) => {
                context.storage.remove_vertex_label(*node, &self.label)?;
            }
            other => {
                return Err(Box::new(TraverserError::label_step_element_kind(
                    "drop_label",
                    other,
                )));
            }
        }
        Ok(vec![source])
    }

    fn extends_path(&self) -> bool {
        false
    }

    fn clone_box(&self) -> Box<dyn GraphStepSideEffectMut<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
