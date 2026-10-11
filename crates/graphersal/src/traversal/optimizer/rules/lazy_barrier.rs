use crate::graph::GraphStorage;
use crate::traversal::optimizer::plan::Plan;
use crate::traversal::optimizer::rule::OptimizationRule;
use crate::traversal::step::GraphStep;
use crate::traversal::step::flatbarrier_steps::step_barrier::lazy_barrier_step;

/// Optimizer rule that inserts a merge point (`lazy_barrier()`) between two adjacent adjacency
/// steps, so equal traversers that the first step produced are merged into one traverser with a
/// bulk before the second step fans out again (TinkerPop's `LazyBarrierStrategy`).
///
/// A barrier goes between `A` and `B` (modulators between them are ignored) when `B` walks the
/// graph (`out`, `in`, `both`, `out_e`, `in_e`, `both_e`, `out_v`, `in_v`, `both_v`, `other_v`)
/// and `A` produces vertices (`out`, `in`, `both`, `out_v`, `in_v`, `both_v`, `other_v`), the
/// only adjacency steps whose output can contain duplicates: an `out_e()` yields each edge once
/// per input. Nothing is inserted across an `as()` (or any other non-modulator step), at the end
/// of a level, or next to an existing barrier. Nested levels are reached by the optimizer's own
/// recursion. Without merging (`bulk.merge` false, a mutating plan, full path recording) the
/// inserted step passes the stream through untouched; on a large stream it also skips the merge
/// when the duplicates' fan-out on the following step would not pay for it (see
/// `StepLazyBarrier`).
#[derive(Default)]
pub(crate) struct LazyBarrierRule;

impl LazyBarrierRule {
    pub fn new() -> Self {
        Self
    }
}

/// An adjacency step whose output is a stream of vertices, where duplicates arise.
fn yields_vertices<'g, S: GraphStorage + 'g>(step: &GraphStep<'g, S>) -> bool {
    matches!(
        step,
        GraphStep::FlatMap(name, flatmap)
            if flatmap.is_adjacency()
                && matches!(
                    *name,
                    "out" | "in" | "both" | "out_v" | "in_v" | "both_v" | "other_v"
                )
    )
}

impl<'g, S: GraphStorage + 'g> OptimizationRule<'g, S> for LazyBarrierRule {
    fn name(&self) -> &'static str {
        "lazy_barrier"
    }

    fn apply(&self, steps: &mut Plan<'g, S>) -> bool {
        let mut modified = false;
        let mut previous: Option<usize> = None;
        let mut index = 0;
        while index < steps.len() {
            match &steps[index] {
                GraphStep::Modulator(_, _) => {}
                GraphStep::FlatMap(_, next) if next.is_adjacency() => {
                    if let Some(first) = previous
                        && yields_vertices(&steps[first])
                    {
                        // Right before `index`, after any modulators of `first`; the barrier
                        // keeps a copy of the step it feeds to judge whether a merge pays.
                        let next = next.clone_box();
                        steps.insert_synthetic(index, lazy_barrier_step(next));
                        modified = true;
                        index += 1;
                    }
                    previous = Some(index);
                }
                _ => previous = None,
            }
            index += 1;
        }
        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphSource;

    fn names(steps: &[GraphStep<'_, &crate::graph::TraversalGraph>]) -> Vec<String> {
        steps.iter().map(|step| step.get_name()).collect()
    }

    #[test]
    fn inserts_between_adjacent_expansions_only() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(None).out(None).out(None).in_(None).values("name");

        let modified = LazyBarrierRule::new().apply_to_steps(&mut traversal.steps);

        assert!(modified);
        assert_eq!(
            names(&traversal.steps),
            vec![
                "v()",
                "out()",
                "lazy_barrier()",
                "out()",
                "lazy_barrier()",
                "in()",
                "values(\"name\")"
            ]
        );
    }

    #[test]
    fn leaves_other_shapes_alone() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let untouched = |mut steps: Vec<GraphStep<'_, &crate::graph::TraversalGraph>>| {
            assert!(!LazyBarrierRule::new().apply_to_steps(&mut steps));
        };
        untouched(g_lock.traversal().v(None).out(None).take_steps());
        untouched(g_lock.traversal().v(None).out(None).count().take_steps());
        untouched(
            g_lock
                .traversal()
                .v(None)
                .out(None)
                .as_("a")
                .out(None)
                .take_steps(),
        );
        untouched(
            g_lock
                .traversal()
                .v(None)
                .out(None)
                .values("name")
                .out(None)
                .take_steps(),
        );
        untouched(g_lock.traversal().v(None).out_e(None).in_v().take_steps());
        untouched(
            g_lock
                .traversal()
                .v(None)
                .out(None)
                .barrier()
                .out(None)
                .take_steps(),
        );
    }

    #[test]
    fn edge_then_vertex_steps_get_one_barrier_after_the_vertices() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let mut traversal = g.v(None).out_e(None).in_v().out(None);
        assert!(LazyBarrierRule::new().apply_to_steps(&mut traversal.steps));
        assert_eq!(
            names(&traversal.steps),
            vec!["v()", "out_e()", "in_v()", "lazy_barrier()", "out()"]
        );
    }
}
