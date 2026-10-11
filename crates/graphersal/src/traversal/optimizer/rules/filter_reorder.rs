use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;

/// Optimizer rule that reorders adjacent filter steps to execute cheaper filters first.
///
/// Filter cost ranking (lower rank executes first):
/// 0: `has_id` (Index lookup or direct integer/string ID comparison)
/// 1: `has_label` (Metadata check on element label)
/// 2: `has_key` / `has_property` / `has_not` / `is(typeOf(..))` (Key presence or type check without value comparison)
/// 3: `has` / `has_p` / `is` / `has_value` (Property value deserialization & comparison)
/// 4: `where_p` / `where_t` / `not` / `and` / `or` (Sub-traversal execution / complex composite logic)
/// 5: Other filter steps
///
/// Only filters without effects are reordered: a filter that mutates the graph or writes a side
/// effect, itself or in any child traversal (`filter(__.aggregate("x"))`, `drop()`,
/// `where(__.sideEffect(..))`; see `GraphStep::has_effects`), ends the block, so nothing moves
/// across it and it sees exactly the traversers it would see unoptimized.
#[derive(Default)]
pub(crate) struct FilterReorderRule;

impl FilterReorderRule {
    pub fn new() -> Self {
        Self
    }

    /// Assigns a cost rank to filter steps. Lower rank = cheaper, executed earlier.
    fn filter_cost(name: &str) -> usize {
        match name {
            "has_id" => 0,
            "has_label" => 1,
            "has_key" | "has_property" | "has_not" | "is_type_of" => 2,
            "has" | "has_p" | "is" | "has_value" | "has_id_p" | "has_label_p" | "has_key_p" => 3,
            "where_p" | "where_t" | "filter" | "not" | "and" | "or" => 4,
            _ => 5,
        }
    }
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for FilterReorderRule {
    fn name(&self) -> &'static str {
        "filter_reorder"
    }

    fn apply(&self, steps: &mut Plan<'g, S>) -> bool {
        if steps.len() < 2 {
            return false;
        }

        let mut modified = false;
        let mut i = 0;
        // Which steps are filters that may move: a filter with an effect (its own or in a child
        // traversal) runs exactly where the query put it.
        let movable: Vec<bool> = steps
            .steps_mut()
            .map(|step| matches!(step, GraphStep::Filter(_, _)) && !step.has_effects())
            .collect();

        while i < steps.len() {
            // Find a contiguous block of movable Filter steps: [i..end]
            if movable[i] {
                let start = i;
                let mut end = i + 1;
                while end < steps.len() && movable[end] {
                    end += 1;
                }

                // If block has at least 2 filter steps, stable sort by cost
                if end - start > 1 {
                    let slice = &steps[start..end];
                    // Check if already ordered
                    let is_ordered = slice.windows(2).all(|w| {
                        let cost_a = match &w[0] {
                            GraphStep::Filter(name, _) => Self::filter_cost(name),
                            _ => usize::MAX,
                        };
                        let cost_b = match &w[1] {
                            GraphStep::Filter(name, _) => Self::filter_cost(name),
                            _ => usize::MAX,
                        };
                        cost_a <= cost_b
                    });

                    if !is_ordered {
                        steps.sort_block_by_key(start, end, |step| match step {
                            GraphStep::Filter(name, _) => Self::filter_cost(name),
                            _ => usize::MAX,
                        });
                        modified = true;
                    }
                }

                i = end;
            } else {
                i += 1;
            }
        }

        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphSource;
    use crate::traversal::__;

    #[test]
    fn test_filter_reorder_where_and_has_id() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        // Chain: where_t then has_id
        let mut traversal = g.v(None).where_t(__::out(None)).has_id("1");

        let rule = FilterReorderRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        // Step 0: v, Step 1: has_id, Step 2: where_t
        assert_eq!(traversal.steps.len(), 3);
        assert!(matches!(traversal.steps[1], GraphStep::Filter("has_id", _)));
        assert!(matches!(
            traversal.steps[2],
            GraphStep::Filter("where_t", _)
        ));
    }

    #[test]
    fn test_filter_reorder_multiple_filters() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        // Chain: where_t, has_value, has_label, has_id
        let mut traversal = g
            .v(None)
            .where_t(__::out(None))
            .has_value("marko")
            .has_label("person")
            .has_id("1");

        let rule = FilterReorderRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(traversal.steps.len(), 5);
        assert!(matches!(traversal.steps[1], GraphStep::Filter("has_id", _)));
        assert!(matches!(
            traversal.steps[2],
            GraphStep::Filter("has_label", _)
        ));
        assert!(matches!(
            traversal.steps[3],
            GraphStep::Filter("has_value", _)
        ));
        assert!(matches!(
            traversal.steps[4],
            GraphStep::Filter("where_t", _)
        ));
    }

    #[test]
    fn test_filter_reorder_stable_for_same_cost() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();

        // Chain: has_label("person"), has_label("software")
        let mut traversal = g.v(None).has_label("person").has_label("software");

        let rule = FilterReorderRule::new();
        let modified = rule.apply_to_steps(&mut traversal.steps);

        assert!(!modified);
        assert_eq!(traversal.steps.len(), 3);
        if let GraphStep::Filter("has_label", ref f) = traversal.steps[1] {
            assert_eq!(f.labels(), Some(&["person".to_string()][..]));
        }
        if let GraphStep::Filter("has_label", ref f) = traversal.steps[2] {
            assert_eq!(f.labels(), Some(&["software".to_string()][..]));
        }
    }
}
