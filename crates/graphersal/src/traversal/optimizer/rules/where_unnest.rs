use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;

/// Optimizer rule that unnests (flattens) `where_t` sub-traversal filter steps
/// that contain only pure local element filter predicates directly into the
/// surrounding step sequence.
///
/// This removes nested traversal execution overhead and allows downstream
/// rules (like `FilterReorderRule`, `SourceFilterPushdownRule`, `BarrierPushdownRule`)
/// to further optimize and fold the filter predicates.
#[derive(Default)]
pub(crate) struct WhereUnnestRule;

impl WhereUnnestRule {
    pub fn new() -> Self {
        Self
    }

    /// Checks whether a graph step is a pure local element filter that does not
    /// navigate the graph, mutate state, or change stream cardinality.
    fn is_local_filter<'g, S: GraphStorage + 'g>(step: &GraphStep<'g, S>) -> bool {
        match step {
            GraphStep::Filter(name, _) => matches!(
                *name,
                "has"
                    | "has_label"
                    | "has_id"
                    | "has_not"
                    | "has_key"
                    | "has_property"
                    | "has_value"
                    | "has_p"
                    | "is"
                    | "is_type_of"
            ),
            _ => false,
        }
    }
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for WhereUnnestRule {
    fn name(&self) -> &'static str {
        "where_unnest"
    }

    fn apply(&self, steps: &mut Plan<'g, S>) -> bool {
        let mut modified = false;
        let mut i = 0;

        while i < steps.len() {
            let should_unnest = if let GraphStep::Filter("where_t", filter_step) = &steps[i] {
                if let Some(inner) = filter_step.as_where_traversal() {
                    // A start/end label changes what the child runs on: it cannot be flattened.
                    !filter_step.has_scope_labels() && inner.steps.iter().all(Self::is_local_filter)
                } else {
                    false
                }
            } else {
                false
            };

            if should_unnest {
                let (step, origin) = steps.take_step(i);
                if let GraphStep::Filter("where_t", filter_step) = step
                    && let Some(inner_steps) = filter_step.into_inner_steps()
                {
                    let inner_len = inner_steps.len();
                    steps.insert_steps(i, inner_steps, &origin);
                    modified = true;
                    i += inner_len;
                    continue;
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
    use crate::traversal::{__, AnonymousTraversal};

    #[test]
    fn test_where_unnest_single_has_label() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        let mut traversal = g.v(None).where_t(__::has_label("person"));

        let rule = WhereUnnestRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 2);
        assert!(matches!(traversal.steps[0], GraphStep::Init("v", _)));
        assert!(matches!(
            traversal.steps[1],
            GraphStep::Filter("has_label", _)
        ));
        if let GraphStep::Filter("has_label", ref f) = traversal.steps[1] {
            assert_eq!(f.labels(), Some(&["person".to_string()][..]));
        }
    }

    #[test]
    fn test_where_unnest_multiple_local_filters() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        let mut traversal = g
            .v(None)
            .where_t(__::has_label("person").has_id("1").has("age", 29));

        let rule = WhereUnnestRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 4);
        assert!(matches!(traversal.steps[0], GraphStep::Init("v", _)));
        assert!(matches!(
            traversal.steps[1],
            GraphStep::Filter("has_label", _)
        ));
        assert!(matches!(traversal.steps[2], GraphStep::Filter("has_id", _)));
        assert!(matches!(traversal.steps[3], GraphStep::Filter("has", _)));
    }

    #[test]
    fn test_where_unnest_skips_non_filter_steps() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        let mut traversal = g.v(None).where_t(__::out(None).has_label("person"));

        let rule = WhereUnnestRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(!modified);
        assert_eq!(traversal.steps.len(), 2);
        assert!(matches!(traversal.steps[0], GraphStep::Init("v", _)));
        assert!(matches!(
            traversal.steps[1],
            GraphStep::Filter("where_t", _)
        ));
    }

    #[test]
    fn test_where_unnest_empty_traversal_drops_step() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        let mut traversal = g.v(None).where_t(AnonymousTraversal::new());

        let rule = WhereUnnestRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 1);
        assert!(matches!(traversal.steps[0], GraphStep::Init("v", _)));
    }

    #[test]
    fn test_where_unnest_multiple_where_steps() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        let mut traversal = g
            .v(None)
            .where_t(__::has_label("person"))
            .where_t(__::has_id("1"));

        let rule = WhereUnnestRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 3);
        assert!(matches!(traversal.steps[0], GraphStep::Init("v", _)));
        assert!(matches!(
            traversal.steps[1],
            GraphStep::Filter("has_label", _)
        ));
        assert!(matches!(traversal.steps[2], GraphStep::Filter("has_id", _)));
    }
}
