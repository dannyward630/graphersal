use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;
use std::collections::HashSet;

/// Optimizer rule that folds consecutive `has_label`, `has_id` and `has(key, value)` filter steps
/// directly into initial scan steps (`v` and `e`). A folded `has(key, value)` (literal property
/// name; a jpath key is a different step and never folds) becomes a [`PropertyFilter`] the scan
/// checks before it creates a traverser, so the elements that fail it cost no traverser and no
/// second pass. Not folded into a source that already has a fused `count()`/`group_count()`
/// (those rules run later and skip a source with property filters).
///
/// [`PropertyFilter`]: crate::traversal::step::PropertyFilter
#[derive(Default)]
pub(crate) struct SourceFilterPushdownRule;

impl SourceFilterPushdownRule {
    pub fn new() -> Self {
        Self
    }
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for SourceFilterPushdownRule {
    fn name(&self) -> &'static str {
        "source_filter_pushdown"
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
        let idx = 1;

        // Whether a `has_label` filter has already been folded into the source step's `labels`
        // (as an OR-prefilter). Under multi-label vertices, intersecting a *second* consecutive
        // `has_label`'s candidates into the source step's label list is unsound (D5, Bug 1): a
        // vertex tagged `{a,b}` passes both `.has_label(["a","b"]).has_label(["b"])` individually,
        // but naively intersecting the candidate lists down to `["b"]` and using that as an
        // exact/membership prefilter can silently drop a vertex whose *other* labels would no
        // longer be represented correctly by a single merged list. So: fold only the first
        // `has_label` immediately after the source step (a safe superset — fetch every vertex
        // carrying at least one candidate label); leave every subsequent `has_label` filter as an
        // explicit post-fetch `Filter` step performing the precise D4 set-intersection check.
        let mut label_folded = if let GraphStep::Init(_, init_step) = &steps[0] {
            init_step.labels().is_some()
        } else {
            false
        };

        // Iterate through consecutive filter steps immediately following Init
        while idx < steps.len() {
            match &steps[idx] {
                GraphStep::Filter("has_label", filter_step) => {
                    let filter_labels = filter_step.labels().map(|l| l.to_vec());
                    match filter_labels {
                        Some(new_labels) if !label_folded => {
                            // Extract and mutate the Init step
                            if let GraphStep::Init(_, init_step) = steps.step_mut(0) {
                                init_step.set_labels(super::distinct(new_labels));
                            }
                            steps.absorb(idx, 0);
                            modified = true;
                            label_folded = true;
                            continue;
                        }
                        Some(_) => {
                            // A has_label filter is already folded upstream; leave this one (and
                            // anything after it) as an explicit Filter step.
                            break;
                        }
                        None => break,
                    }
                }
                GraphStep::Filter("has_id", filter_step) => {
                    let filter_ids = filter_step.ids().map(|ids| ids.to_vec());
                    if let Some(new_ids) = filter_ids {
                        // Extract and mutate the Init step
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
                        continue;
                    } else {
                        break;
                    }
                }
                GraphStep::Filter(_, filter_step) => {
                    let Some(filter) = filter_step.property_equality() else {
                        // Non-filterable step encountered; cannot push down past this point
                        break;
                    };
                    let folded = match steps.step_mut(0) {
                        GraphStep::Init(_, init_step)
                            if !init_step.is_count_only()
                                && !init_step.is_group_count_by_label() =>
                        {
                            init_step.push_property_filter(filter)
                        }
                        _ => false,
                    };
                    if !folded {
                        break;
                    }
                    steps.absorb(idx, 0);
                    modified = true;
                    continue;
                }
                _ => {
                    // Non-filterable step encountered; cannot push down past this point
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
    fn test_pushdown_has_label() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(None).has_label("person");

        let rule = SourceFilterPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 1);
        if let GraphStep::Init("v", step) = &traversal.steps[0] {
            assert_eq!(step.labels(), Some(&["person".to_string()][..]));
            assert_eq!(step.ids(), None);
        } else {
            panic!("Expected Init step");
        }
    }

    #[test]
    fn test_pushdown_has_id() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(None).has_id("1");

        let rule = SourceFilterPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 1);
        if let GraphStep::Init("v", step) = &traversal.steps[0] {
            assert_eq!(step.ids(), Some(&["1".to_string()][..]));
            assert_eq!(step.labels(), None);
        } else {
            panic!("Expected Init step");
        }
    }

    /// D5 (Bug 1) regression: a *second* consecutive `has_label` must not be folded into the
    /// source step's label list via intersection anymore. Only the first folds (as a safe
    /// OR-prefilter); the second stays an explicit `Filter` step downstream, so the plan still has
    /// two steps and the source step keeps the *first* filter's full candidate set.
    #[test]
    fn test_pushdown_only_first_has_label_folds() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g
            .v(None)
            .has_label(vec!["person", "software"])
            .has_label("software");

        let rule = SourceFilterPushdownRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        // The first has_label folds into the source step; the second remains an explicit Filter.
        assert_eq!(traversal.steps.len(), 2);
        if let GraphStep::Init("v", step) = &traversal.steps[0] {
            assert_eq!(
                step.labels(),
                Some(&["person".to_string(), "software".to_string()][..])
            );
        } else {
            panic!("Expected Init step");
        }
        assert!(matches!(
            &traversal.steps[1],
            GraphStep::Filter("has_label", _)
        ));
    }

    /// D5, Bug 1's exact regression scenario: a vertex tagged `{a,b}` must survive
    /// `.has_label("a").has_label("b")` end to end through the optimized pipeline, not just
    /// unoptimized. Before the fix, the second `has_label` intersected into the source step's
    /// label list, which could silently under-fetch a multi-labeled vertex.
    #[test]
    fn test_multi_label_vertex_survives_consecutive_has_label() {
        use crate::graph::GraphStorage;

        let mut tg = crate::graph::TraversalGraph::new();
        GraphStorage::add_vertex(
            &mut tg,
            Some("v1".into()),
            vec!["a".into(), "b".into()],
            crate::value::PropertyMap::default(),
        )
        .unwrap();

        let g = tg.traversal();
        let count = g
            .v(None)
            .has_label("a")
            .has_label("b")
            .count()
            .to_list()
            .unwrap();
        assert_eq!(count[0].as_i64(), Some(1));
    }

    fn names(steps: &[GraphStep<'_, &crate::graph::TraversalGraph>]) -> Vec<String> {
        steps.iter().map(|step| step.get_name()).collect()
    }

    #[test]
    fn has_key_value_folds_into_the_source_after_labels_and_ids() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let mut traversal = g_lock
            .traversal()
            .v(None)
            .has_label("person")
            .has("age", 29)
            .has("name", "marko")
            .values("name");

        assert!(SourceFilterPushdownRule::new().apply_to_steps(&mut traversal.steps));
        assert_eq!(
            names(&traversal.steps),
            [
                r#"v(labels: ["person"]).has("age", 29).has("name", "marko")"#,
                r#"values("name")"#
            ]
        );
        let mut edges = g_lock.traversal().e(None).has("weight", 0.4);
        assert!(SourceFilterPushdownRule::new().apply_to_steps(&mut edges.steps));
        assert_eq!(names(&edges.steps), [r#"e().has("weight", 0.4)"#]);
    }

    #[test]
    fn jpath_predicate_and_later_has_steps_stay_filters() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let unchanged = |mut steps: Vec<GraphStep<'_, &crate::graph::TraversalGraph>>| {
            let before = names(&steps);
            assert!(!SourceFilterPushdownRule::new().apply_to_steps(&mut steps));
            assert_eq!(names(&steps), before);
        };
        let path: crate::value::JsonPath = "name".parse().unwrap();
        unchanged(g_lock.traversal().v(None).has(path, "marko").take_steps());
        unchanged(
            g_lock
                .traversal()
                .v(None)
                .has_p("age", crate::P::Gt(29))
                .take_steps(),
        );
        unchanged(
            g_lock
                .traversal()
                .v(None)
                .out(None)
                .has("name", "josh")
                .take_steps(),
        );
        // Folding stops at the first step it cannot fold.
        let mut traversal = g_lock
            .traversal()
            .v(None)
            .has("name", "marko")
            .out(None)
            .has("name", "josh");
        assert!(SourceFilterPushdownRule::new().apply_to_steps(&mut traversal.steps));
        assert_eq!(
            names(&traversal.steps),
            [
                r#"v().has("name", "marko")"#,
                "out()",
                r#"has("name", "josh")"#
            ]
        );
    }
}
