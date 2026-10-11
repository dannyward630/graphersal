use crate::graph::GraphStorage;
use crate::*;

/// `add_label("L")`: adds a label to the label set of each incoming vertex and emits the same
/// vertex.
#[derive(Clone)]
struct StepAddLabel {
    label: CompactString,
}

impl fmt::Display for StepAddLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "add_label({})",
            crate::value::literal::rhai_string(&self.label)
        )
    }
}

fn add_label_step<'g, S: GraphStorage + 'g>(label: CompactString) -> GraphStep<'g, S> {
    GraphStep::SideEffectMut("add_label", Box::new(StepAddLabel { label }))
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Adds `label` to the label set of each incoming vertex and lets the vertex continue.
    ///
    /// A Graphersal extension: TinkerPop labels are immutable and TinkerPop has no such step, so
    /// this adds to Gremlin rather than deviating from it. A vertex that already carries the
    /// label is unchanged (no error), so a bulk-n traverser changes its vertex once. The first
    /// label stays the primary one (`label()`); the new label is appended to the set
    /// (`labels()`), and an unlabeled vertex gets it as its only label. With a schema active the
    /// vertex must be valid under its new label set (`Closed`: the label is declared and every
    /// incident edge still matches a declared connection; `Open`/`Closed`: the new label's
    /// `required` keys are present and the stored values have the declared types); a violation
    /// fails the traversal, whose unit rolls back. A label containing `"::"` is rejected.
    /// An edge is an error whose help names [`Self::set_label`]; any other value is an error too.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let mut graph = TraversalGraph::tinkerpop_modern();
    /// graph.traversal_mut().v("1").add_label("employee").next().unwrap();
    /// assert_eq!(
    ///     graph.traversal().v(None).has_label("employee").count().next().unwrap(),
    ///     Some(ElementProperty::from(1))
    /// );
    /// ```
    pub fn add_label(self, label: impl Into<CompactString>) -> Self {
        self.push_step(add_label_step(label.into()))
    }
}

impl __ {
    /// Anonymous `add_label()`; see [`GraphTraversalSource::add_label`].
    pub fn add_label<'g, S: GraphStorage + 'g>(
        label: impl Into<CompactString>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().add_label(label)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Anonymous `add_label()`; see [`GraphTraversalSource::add_label`].
    pub fn add_label(self, label: impl Into<CompactString>) -> Self {
        self.push_step(add_label_step(label.into()))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepSideEffectMut<'g, S> for StepAddLabel {
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
                context
                    .storage
                    .add_vertex_label(*node, self.label.clone())?;
            }
            other => {
                return Err(Box::new(TraverserError::label_step_element_kind(
                    "add_label",
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
