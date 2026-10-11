use crate::graph::GraphStorage;
use crate::traversal::optimizer::path_analysis::{PathAnalysis, PathRequirement};
use crate::value::Operator;
use crate::*;

/// The `reason` text of `TraverserError::InvalidModulator` for a second `by()` on `sack(op)`;
/// its `help()` keys on it.
pub(crate) mod reason {
    /// A second `by()`: the sack is updated with one operand per traverser.
    pub(crate) const TOO_MANY: &str = "Sack step can only have one by modulator";
}

/// `sack(Operator)` / `sack(Operator).by(..)`: replaces the traverser's sack with
/// `operator(sack, operand)` (TinkerPop `SackValueStep`).
///
/// The operand is the value the `by()` modulator resolves for the traverser (a property key, a
/// `T.id`/`T.label` token, or the first result of a child traversal; a child runs with the
/// traverser's own sack), or the traverser's own object without a `by()`. A `by()` that resolves
/// nothing (a missing property, an empty child traversal) drops the traverser, as TinkerPop's
/// `SackValueStep` does with an empty traverser. Only one `by()` is accepted.
///
/// The traverser, its path and its bulk travel on unchanged: the step is a side effect on the
/// sack, it does not add a path position. A bulk-n traverser is updated once, all of its copies
/// share the new sack. Sacks are immutable values shared by handle, so the update interns the
/// new value and replaces the handle of this one traverser; a child traversal's own sack updates
/// never flow back to the parent.
///
/// **Deviations from TinkerPop** (see `book/src/users_guide/sack_and_operators.md`): integer
/// arithmetic that leaves the `int64` range fails with `OperatorFailed` instead of promoting to
/// a wider type, a vertex or edge cannot become a sack value, and `sack(BiFunction)` (a lambda)
/// does not exist: only an [`Operator`] is accepted.
struct StepSackValue<'g, S: GraphStorage + 'g> {
    operator: Operator,
    by: Option<By<'g, S>>,
    by_check: ByModulatorCheck,
}

impl<'g, S: GraphStorage + 'g> Clone for StepSackValue<'g, S> {
    fn clone(&self) -> Self {
        Self {
            operator: self.operator,
            by: self.by.clone(),
            by_check: self.by_check.clone(),
        }
    }
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepSackValue<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sack(Operator.{})", self.operator)?;
        match &self.by {
            None => Ok(()),
            Some(by) => f.write_str(&crate::traversal::render_by(by)),
        }
    }
}

fn sack_step<'g, S: GraphStorage + 'g>(operator: Operator) -> GraphStep<'g, S> {
    GraphStep::FlatMap(
        "sack",
        Box::new(StepSackValue {
            operator,
            by: None,
            by_check: ByModulatorCheck::default(),
        }),
    )
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Updates each traverser's sack with `operator(sack, operand)`; the operand is set by a
    /// following `by(..)`, and without one it is the traverser's own object (TinkerPop
    /// `sack(Operator)`). A `by()` that resolves nothing drops the traverser.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = GraphSource::tinkerpop_modern();
    /// let lock = graph.read();
    /// let sacks = lock
    ///     .traversal()
    ///     .with_sack(0)
    ///     .v(None)
    ///     .out_e(None)
    ///     .sack_with(Operator::Sum)
    ///     .by("weight")
    ///     .in_v()
    ///     .sack()
    ///     .to_list()
    ///     .unwrap();
    /// assert_eq!(sacks.len(), 6);
    /// ```
    pub fn sack_with(self, operator: Operator) -> Self {
        self.push_step(sack_step(operator))
    }
}

impl __ {
    /// `sack(Operator)` as a child traversal; see [`GraphTraversalSource::sack_with`].
    pub fn sack_with<'g, S: GraphStorage + 'g>(operator: Operator) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().sack_with(operator)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `sack(Operator)`; see [`GraphTraversalSource::sack_with`].
    pub fn sack_with(self, operator: Operator) -> Self {
        self.push_step(sack_step(operator))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepFlatMap<'g, S> for StepSackValue<'g, S> {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        mut source: Traverser<S>,
        output: &mut Vec<Traverser<S>>,
    ) -> TraverserResult<()> {
        self.by_check.check("sack")?;
        let resolved = match &self.by {
            Some(by) => {
                TraverserValue::fetch_traverser_value_by(context, &source, &source.value, by)?
            }
            None => Some(source.value.clone()),
        };
        // Unproductive by(): the traverser is dropped (TinkerPop `EmptyTraverser`).
        let Some(resolved) = resolved else {
            return Ok(());
        };
        let storage: &S = context.storage;
        let operand = resolved.into_node_property(storage)?;
        let current = context.sacks.get(source.sack).clone();
        let updated = self.operator.apply_owned(current, operand)?;
        source.sack = context.sacks.intern(updated)?;
        output.push(source);
        Ok(())
    }

    fn extends_path(&self) -> bool {
        false
    }

    fn push_modulator_by(&mut self, args: By<'g, S>, _modulator_name: &str) -> ByModulation {
        self.by_check.observe(&args);
        if self.by.is_some() {
            self.by_check.reject_last(reason::TOO_MANY);
        }
        self.by = Some(args);
        ByModulation::Accepted
    }

    fn child_traversals_mut(&mut self) -> Vec<&mut crate::traversal::AnonymousTraversal<'g, S>> {
        self.by
            .as_mut()
            .and_then(By::traversal_mut)
            .into_iter()
            .collect()
    }

    fn analyze_nested_paths(
        &mut self,
        analysis: &PathAnalysis,
        _downstream: &PathRequirement,
    ) -> PathRequirement {
        match &mut self.by {
            Some(by) => by.analyze_paths(analysis),
            None => PathRequirement::None,
        }
    }

    fn clone_box(&self) -> Box<dyn GraphStepFlatMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
