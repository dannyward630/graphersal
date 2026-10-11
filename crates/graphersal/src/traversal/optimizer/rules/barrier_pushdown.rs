use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;
use std::collections::HashSet;

/// Optimizer rule that folds consecutive `has_label` filter steps following
/// edge flatmap navigation steps (`out_e`, `in_e`, `both_e`)
/// directly into the edge step's label parameter.
#[derive(Default)]
pub(crate) struct BarrierPushdownRule;

impl BarrierPushdownRule {
    pub fn new() -> Self {
        Self
    }
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for BarrierPushdownRule {
    fn name(&self) -> &'static str {
        "barrier_pushdown"
    }

    fn apply(&self, steps: &mut Plan<'g, S>) -> bool {
        if steps.len() < 2 {
            return false;
        }

        let mut modified = false;
        let mut i = 0;

        while i < steps.len() {
            // Check if step at i is one of the supported FlatMap edge navigation steps
            let is_foldable_edge_flatmap = matches!(
                &steps[i],
                GraphStep::FlatMap(name, _)
                    if *name == "out_e"
                        || *name == "in_e"
                        || *name == "both_e"
            );

            if !is_foldable_edge_flatmap {
                i += 1;
                continue;
            }

            // Check following steps for consecutive has_label filter steps
            let j = i + 1;
            while j < steps.len() {
                match &steps[j] {
                    GraphStep::Filter("has_label", filter_step) => {
                        let filter_labels = filter_step.labels().map(|l| l.to_vec());
                        if let Some(new_labels) = filter_labels {
                            if let GraphStep::FlatMap(_, flatmap_step) = steps.step_mut(i) {
                                match flatmap_step.labels() {
                                    None => {
                                        flatmap_step.set_labels(new_labels);
                                    }
                                    Some(existing) => {
                                        let filter_set: HashSet<_> = new_labels.iter().collect();
                                        let intersected: Vec<String> = existing
                                            .iter()
                                            .filter(|l| filter_set.contains(l))
                                            .cloned()
                                            .collect();
                                        flatmap_step.set_labels(intersected);
                                    }
                                }
                            }
                            steps.absorb(j, i);
                            modified = true;
                            // Do not increment j; check next step shifted into position j
                            continue;
                        } else {
                            break;
                        }
                    }
                    _ => break,
                }
            }

            i += 1;
        }

        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphSource;

    #[test]
    fn test_fold_out_e_has_label() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v("1").out_e(None).has_label("knows");

        let rule = BarrierPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 2); // Init("v"), FlatMap("out_e")
        if let GraphStep::FlatMap("out_e", step) = &traversal.steps[1] {
            assert_eq!(step.labels(), Some(&["knows".to_string()][..]));
        } else {
            panic!("Expected FlatMap('out_e') step");
        }
    }

    #[test]
    fn test_fold_out_e_intersect_labels() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v("1").out_e(vec!["knows", "created"]).has_label("knows");

        let rule = BarrierPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 2);
        if let GraphStep::FlatMap("out_e", step) = &traversal.steps[1] {
            assert_eq!(step.labels(), Some(&["knows".to_string()][..]));
        } else {
            panic!("Expected FlatMap('out_e') step");
        }
    }

    #[test]
    fn test_fold_in_e_and_both_e() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let mut traversal = g_lock.traversal().v("1").in_e(None).has_label("created");

        let rule = BarrierPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);
        assert!(modified);
        assert_eq!(traversal.steps.len(), 2);
        if let GraphStep::FlatMap("in_e", step) = &traversal.steps[1] {
            assert_eq!(step.labels(), Some(&["created".to_string()][..]));
        } else {
            panic!("Expected FlatMap('in_e') step");
        }

        let mut traversal_both = g_lock.traversal().v("1").both_e(None).has_label("knows");
        let modified = rule.apply_to_steps(&mut traversal_both.steps);
        assert!(modified);
        assert_eq!(traversal_both.steps.len(), 2);
        if let GraphStep::FlatMap("both_e", step) = &traversal_both.steps[1] {
            assert_eq!(step.labels(), Some(&["knows".to_string()][..]));
        } else {
            panic!("Expected FlatMap('both_e') step");
        }
    }
}
