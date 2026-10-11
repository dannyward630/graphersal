use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::step::GraphStep;

/// A rule that inspects and transforms traversal steps.
#[allow(dead_code)]
pub(crate) trait OptimizationRule<'g, S: GraphStorage + 'g>: Send + Sync {
    /// Human-readable identifier for the rule.
    fn name(&self) -> &'static str;

    /// Attempts to apply the optimization rule to the plan in-place.
    /// Returns `true` if steps were modified, `false` otherwise.
    fn apply(&self, plan: &mut Plan<'g, S>) -> bool;

    /// Applies the rule to bare steps, discarding logical provenance.
    ///
    /// Only for callers that have no plan to maintain, such as rule unit tests.
    fn apply_to_steps(&self, steps: &mut Vec<GraphStep<'g, S>>) -> bool {
        let mut plan = Plan::from_steps(std::mem::take(steps), true);
        let modified = self.apply(&mut plan);
        *steps = plan.into_steps();
        modified
    }
}
