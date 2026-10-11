use crate::graph::GraphStorage;
use crate::*;

/// `set_label("L")`: changes the label of each incoming edge and emits the same edge.
#[derive(Clone)]
struct StepSetLabel {
    label: CompactString,
}

impl fmt::Display for StepSetLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_label({})",
            crate::value::literal::rhai_string(&self.label)
        )
    }
}

fn set_label_step<'g, S: GraphStorage + 'g>(label: CompactString) -> GraphStep<'g, S> {
    GraphStep::SideEffectMut("set_label", Box::new(StepSetLabel { label }))
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Changes the label of each incoming edge to `label` and lets the edge continue.
    ///
    /// A Graphersal extension: TinkerPop labels are immutable and TinkerPop has no such step, so
    /// this adds to Gremlin rather than deviating from it. Edges stay single-labeled: the old
    /// label is replaced, an unlabeled edge gets `label`. An edge that already carries `label`
    /// is unchanged (no error), so a bulk-n traverser changes its edge once. With a schema active
    /// the edge as stored must be valid under the new label (`Closed`: the label is declared and
    /// the endpoints match a declared connection; `Open`/`Closed`: the new label's `required`
    /// keys are present and the stored values have the declared types; nothing is coerced); a
    /// violation fails the traversal, whose unit rolls back. The storage must support label
    /// changes ([`GraphStorage::set_edge_label`]).
    /// A vertex is an error whose help names [`Self::add_label`] and [`Self::drop_label`]; any
    /// other value is an error too.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let mut graph = TraversalGraph::tinkerpop_modern();
    /// graph.traversal_mut().e("0").set_label("met").next().unwrap();
    /// assert_eq!(
    ///     graph.traversal().e(None).has_label("met").count().next().unwrap(),
    ///     Some(ElementProperty::from(1))
    /// );
    /// ```
    pub fn set_label(self, label: impl Into<CompactString>) -> Self {
        self.push_step(set_label_step(label.into()))
    }
}

impl __ {
    /// Anonymous `set_label()`; see [`GraphTraversalSource::set_label`].
    pub fn set_label<'g, S: GraphStorage + 'g>(
        label: impl Into<CompactString>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().set_label(label)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Anonymous `set_label()`; see [`GraphTraversalSource::set_label`].
    pub fn set_label(self, label: impl Into<CompactString>) -> Self {
        self.push_step(set_label_step(label.into()))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepSideEffectMut<'g, S> for StepSetLabel {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Option<Traverser<S>>,
    ) -> TraverserResult<Vec<Traverser<S>>> {
        let Some(source) = source else {
            return Ok(Vec::new());
        };
        match &source.value {
            TraverserValue::Edge(edge) => {
                context.storage.set_edge_label(*edge, self.label.clone())?;
            }
            other => {
                return Err(Box::new(TraverserError::label_step_element_kind(
                    "set_label",
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
