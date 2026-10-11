//! The marker of a source step (`with_sack()`, `with_side_effect()`) written inside a child
//! traversal.

use crate::error::TraverserError;
use crate::graph::GraphStorage;
use crate::*;

/// Stands in a child traversal where the script DSL recorded a step that only the traversal
/// source `g` has (`__.with_sack(1)`): it fails with [`TraverserError::SourceStepInChild`] in
/// the plan-time validation pass (`plan_check`), before execution, wherever the child sits.
/// Builders cannot fail, hence the marker instead of an error at build time.
#[derive(Clone)]
struct StepSourceInChild {
    /// The source step as the user writes it in snake_case, such as `with_sack`.
    step: &'static str,
}

impl fmt::Display for StepSourceInChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}()", self.step)
    }
}

impl StepSourceInChild {
    fn error(&self) -> TraverserResult<()> {
        Err(Box::new(TraverserError::SourceStepInChild {
            step: self.step.to_string(),
        }))
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Records a source step (`step`, such as `with_sack`) written inside this child traversal;
    /// see [`StepSourceInChild`]. Used by the script DSL lowering only.
    #[cfg_attr(not(feature = "script"), allow(dead_code))]
    pub(crate) fn source_step_in_child(self, step: &'static str) -> Self {
        self.push_step(GraphStep::Filter(
            step,
            Box::new(StepSourceInChild { step }),
        ))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepFilter<'g, S> for StepSourceInChild {
    fn process(
        &self,
        _context: &mut TraverserContext<'_, S>,
        _source: &Traverser<S>,
    ) -> TraverserResult<bool> {
        self.error().map(|()| false)
    }

    fn plan_check(&self) -> TraverserResult<()> {
        self.error()
    }

    fn clone_box(&self) -> Box<dyn GraphStepFilter<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
