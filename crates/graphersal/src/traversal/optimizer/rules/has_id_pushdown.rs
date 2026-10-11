use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;
use std::collections::HashSet;

/// Optimizer rule that folds `has_id` filter steps directly into initial `v` (and `e`) scan steps.
///
/// Specifically:
/// - Detects `[GraphStep::Init("v", StepV { arg: None }), GraphStep::Filter("has_id", step)]`
/// - Extracts IDs from `has_id` and assigns to `StepV.arg` (or intersects with existing IDs).
/// - Removes the redundant `has_id` filter step.
/// - Also applies similarly to edge scan `e`.
#[derive(Default)]
pub(crate) struct HasIdPushdownRule;

impl HasIdPushdownRule {
    pub fn new() -> Self {
        Self
    }
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for HasIdPushdownRule {
    fn name(&self) -> &'static str {
        "has_id_pushdown"
    }

    fn apply(&self, steps: &mut Plan<'g, S>) -> bool {
        if steps.is_empty() {
            return false;
        }

        // Check if the first step is an Init step ("v" or "e")
        let is_init = matches!(&steps[0], GraphStep::Init(name, _) if *name == "v" || *name == "e");
        if !is_init {
            return false;
        }

        let mut modified = false;
        let mut idx = 1;

        // Iterate through consecutive filter steps immediately following Init
        // where has_id can be folded into Init.
        while idx < steps.len() {
            match &steps[idx] {
                GraphStep::Filter("has_id", filter_step) => {
                    let filter_ids = filter_step.ids().map(|ids| ids.to_vec());
                    if let Some(new_ids) = filter_ids {
                        if let GraphStep::Init(_, init_step) = steps.step_mut(0) {
                            match init_step.ids() {
                                None => {
                                    init_step.set_ids(super::distinct(new_ids));
                                }
                                Some(existing) => {
                                    let filter_set: HashSet<_> = new_ids.iter().collect();
                                    let intersected: Vec<String> = existing
                                        .iter()
                                        .filter(|id| filter_set.contains(id))
                                        .cloned()
                                        .collect();
                                    init_step.set_ids(intersected);
                                }
                            }
                        }
                        steps.absorb(idx, 0);
                        modified = true;
                        // Don't advance idx because an element was removed
                        continue;
                    } else {
                        break;
                    }
                }
                // Allow looking past has_label steps if already present, or stop at other steps
                GraphStep::Filter("has_label", _) => {
                    idx += 1;
                }
                _ => {
                    break;
                }
            }
        }

        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphSource;

    #[test]
    fn test_has_id_pushdown_v() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(None).has_id("1");

        let rule = HasIdPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 1);
        if let GraphStep::Init("v", step) = &traversal.steps[0] {
            assert_eq!(step.ids(), Some(&["1".to_string()][..]));
        } else {
            panic!("Expected Init('v') step");
        }
    }

    #[test]
    fn test_has_id_pushdown_with_existing_ids() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(vec!["1", "2"]).has_id("1");

        let rule = HasIdPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 1);
        if let GraphStep::Init("v", step) = &traversal.steps[0] {
            assert_eq!(step.ids(), Some(&["1".to_string()][..]));
        } else {
            panic!("Expected Init('v') step");
        }
    }

    #[test]
    fn test_has_id_pushdown_past_has_label() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(None).has_label("person").has_id("1");

        let rule = HasIdPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 2);
        if let GraphStep::Init("v", step) = &traversal.steps[0] {
            assert_eq!(step.ids(), Some(&["1".to_string()][..]));
        } else {
            panic!("Expected Init('v') step");
        }
        assert_eq!(traversal.steps[1].get_name(), "has_label(\"person\")");
    }
}
