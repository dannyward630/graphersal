use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;

/// Optimizer rule that folds the constant `property(key, value)` steps directly following an
/// `add_v()`/`add_e()` into that step, so the element is created with all its properties in ONE
/// storage call (`add_vertex`/`add_edge`).
///
/// This is what lets a schema with required properties accept `addV("Person").property("name", ..)`,
/// and what makes a failing property leave no element behind: the storage validates the complete
/// element before inserting it.
///
/// Fold boundaries:
/// - The run starts right after the add step. Modulators (`from()`/`to()`) and `as()` labels do
///   not break it.
/// - Only `property(name, constant)` folds. The run stops at the first other step: a
///   traversal-valued `property(k, __...)` (its child traversal receives the NEW element as input,
///   so it cannot be pre-evaluated), a `property(Cardinality, ..)`, a `jpath` key, `property(map)`,
///   or any non-property step. Everything after that point stays unfused.
/// - The same key repeated: the last value wins, as in the unfused sequence.
#[derive(Default)]
pub(crate) struct AddPropertyFoldRule;

impl AddPropertyFoldRule {
    pub fn new() -> Self {
        Self
    }
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for AddPropertyFoldRule {
    fn name(&self) -> &'static str {
        "add_property_fold"
    }

    fn apply(&self, plan: &mut Plan<'g, S>) -> bool {
        let mut modified = false;
        let mut add = 0;
        while add < plan.len() {
            if !matches!(&plan[add], GraphStep::SideEffectMut("add_v" | "add_e", _)) {
                add += 1;
                continue;
            }
            let mut next = add + 1;
            while next < plan.len() {
                let folded = match &plan[next] {
                    GraphStep::Modulator(_, _) | GraphStep::As(_, _) => {
                        next += 1;
                        continue;
                    }
                    GraphStep::SideEffectMut("property", step) => step
                        .constant_property()
                        .map(|(key, value)| (key.clone(), value.clone())),
                    _ => None,
                };
                let Some((key, value)) = folded else {
                    break;
                };
                if let GraphStep::SideEffectMut(_, step) = plan.step_mut(add)
                    && let Some(element) = step.as_add_element_mut()
                {
                    element.fold_property(key, value);
                }
                plan.absorb(next, add);
                modified = true;
            }
            add += 1;
        }
        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn folds_the_whole_run_and_stops_at_a_traversal_value() {
        let mut graph = TraversalGraph::new();
        let mut traversal = graph
            .traversal_mut()
            .add_v("p")
            .property("a", 1)
            .property("b", 2)
            .property_by("c", __::values("a"))
            .property("d", 4);
        let rule = AddPropertyFoldRule::new();
        assert!(rule.apply_to_steps(&mut traversal.steps));
        // add_v + property(c, traversal) + property(d)
        assert_eq!(traversal.steps.len(), 3);
        assert!(!rule.apply_to_steps(&mut traversal.steps));
    }
}
