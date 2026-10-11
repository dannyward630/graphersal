//! Logical-to-physical provenance for the rule-based optimizer.
//!
//! Rules rewrite a traversal before it runs: filters are reordered, folded into scans and
//! expansions, `count()` collapses into an init step, and local `where()` traversals are unnested.
//! Once that happened, a physical step index no longer says anything about the query the user
//! wrote — which is exactly what a diagnostic needs to point at.
//!
//! [`Plan`] carries the steps together with the logical positions each one stands for, and exposes
//! the handful of mutations the rules actually perform, so provenance is maintained by
//! construction rather than by every rule remembering to do it.
//!
//! Tracking is pay-as-you-go: a level no rule touches allocates nothing, and a rewritten level only
//! pays for the steps that actually changed. Steps taken out of the plan are kept rather than
//! rendered, so the logical query is reconstructed when a step fails and never on the happy path.

use std::ops::Deref;

use crate::graph::GraphStorage;
use crate::traversal::step::GraphStep;

/// The logical (pre-optimization) positions a single physical step stands for.
///
/// Folding rules absorb a step into a neighbour, so one physical step can represent several
/// logical ones: `v().has_label("person").count()` executes as a single `v(labels, count_only)`
/// step that originates from all three.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StepOrigin {
    /// Position of the step itself in the logical plan.
    primary: u32,
    /// Positions of the steps folded into this one; empty for the common one-to-one case.
    absorbed: Vec<u32>,
}

impl StepOrigin {
    /// Creates a one-to-one origin for a step at `position` in the logical plan.
    fn new(position: usize) -> Self {
        Self {
            primary: position as u32,
            absorbed: Vec::new(),
        }
    }

    /// Returns every logical position this step stands for, in ascending order.
    fn positions(&self) -> Vec<usize> {
        let mut positions: Vec<usize> = std::iter::once(self.primary as usize)
            .chain(self.absorbed.iter().map(|p| *p as usize))
            .collect();
        positions.sort_unstable();
        positions.dedup();
        positions
    }

    /// Folds another step's provenance into this one.
    fn merge(&mut self, other: StepOrigin) {
        self.absorbed.push(other.primary);
        self.absorbed.extend(other.absorbed);
    }
}

/// A traversal level that rules rewrote, holding what is needed to reconstruct the query as the
/// user wrote it and to map an executed step back to the steps it came from.
pub(crate) struct LevelPlan<'g, S: GraphStorage + 'g> {
    /// Provenance of each executed step, in physical order, modulators included.
    origins: Vec<StepOrigin>,
    /// What became of the logical steps a rule rewrote, by logical position.
    changed: Vec<(u32, Rewritten<'g, S>)>,
    /// Number of steps the level had before optimization.
    logical_len: usize,
}

/// How a logical step survives a rewrite, so it can still be shown in a diagnostic.
enum Rewritten<'g, S: GraphStorage + 'g> {
    /// Removed from the plan and kept as is; rendering it is deferred to the failure path.
    Detached(GraphStep<'g, S>),
    /// Rewritten in place or consumed by a rule, so it was rendered while still intact.
    Rendered(String),
}

impl<'g, S: GraphStorage + 'g> LevelPlan<'g, S> {
    /// Creates provenance for a pristine level, where every step stands for itself.
    fn identity(steps: &[GraphStep<'g, S>]) -> Self {
        Self {
            origins: (0..steps.len()).map(StepOrigin::new).collect(),
            changed: Vec::with_capacity(2),
            logical_len: steps.len(),
        }
    }

    /// Rebuilds the level as the user wrote it.
    ///
    /// Steps no rule touched are rendered from `current`, since a rewrite never changed them;
    /// everything else comes from what was captured at the moment it was rewritten. Returns the
    /// rendered steps and, for each logical position, its index among them.
    fn logical_level(&self, current: &[GraphStep<'g, S>]) -> (Vec<String>, Vec<Option<usize>>) {
        let mut slots: Vec<Option<(String, bool)>> = vec![None; self.logical_len];
        for (position, step) in &self.changed {
            let Some(slot) = slots.get_mut(*position as usize) else {
                continue;
            };
            *slot = match step {
                Rewritten::Detached(step) => {
                    Some((step.get_name(), matches!(step, GraphStep::Modulator(_, _))))
                }
                // A rule only rewrites or consumes real steps, never a modulator.
                Rewritten::Rendered(name) => Some((name.clone(), false)),
            };
        }
        for (index, origin) in self.origins.iter().enumerate() {
            match (slots.get_mut(origin.primary as usize), current.get(index)) {
                (Some(slot), Some(step)) if slot.is_none() => {
                    *slot = Some((step.get_name(), matches!(step, GraphStep::Modulator(_, _))));
                }
                _ => {}
            }
        }

        let mut names = Vec::with_capacity(slots.len());
        let mut positions = Vec::with_capacity(slots.len());
        for slot in slots {
            match slot {
                Some((name, false)) => {
                    positions.push(Some(names.len()));
                    names.push(name);
                }
                _ => positions.push(None),
            }
        }
        (names, positions)
    }

    /// Reconstructs the logical level together with the positions in it that `physical` came from.
    ///
    /// `physical` counts every executed step, modulators included, because that is how provenance
    /// is indexed.
    pub(crate) fn resolve(
        &self,
        current: &[GraphStep<'g, S>],
        physical: usize,
    ) -> (Vec<String>, Vec<usize>) {
        let (names, positions) = self.logical_level(current);
        let marked = match self.origins.get(physical) {
            Some(origin) => origin
                .positions()
                .into_iter()
                .filter_map(|logical| positions.get(logical).copied().flatten())
                .collect(),
            None => Vec::new(),
        };
        (names, marked)
    }
}

/// A traversal level being optimized: its steps plus the provenance of the rewrites applied so far.
pub(crate) struct Plan<'g, S: GraphStorage + 'g> {
    steps: Vec<GraphStep<'g, S>>,
    /// `None` until the first rewrite: until then every step still stands for itself.
    rewrite: Option<LevelPlan<'g, S>>,
    /// Whether rewrites are recorded at all; off makes every rewrite free of bookkeeping.
    tracking: bool,
}

impl<'g, S: GraphStorage + 'g> Deref for Plan<'g, S> {
    type Target = [GraphStep<'g, S>];

    fn deref(&self) -> &Self::Target {
        &self.steps
    }
}

impl<'g, S: GraphStorage + 'g> Plan<'g, S> {
    /// Wraps a logical plan. Tracking starts at the first rewrite, so this costs nothing.
    pub(crate) fn from_steps(steps: Vec<GraphStep<'g, S>>, tracking: bool) -> Self {
        Self {
            steps,
            rewrite: None,
            tracking,
        }
    }

    /// Splits the optimized plan into its steps and the provenance of the rewrites, which is
    /// `None` when no rule changed anything and the executed plan still is the user's query.
    pub(crate) fn into_parts(self) -> (Vec<GraphStep<'g, S>>, Option<LevelPlan<'g, S>>) {
        (self.steps, self.rewrite)
    }

    /// Returns the optimized steps, discarding provenance.
    pub(crate) fn into_steps(self) -> Vec<GraphStep<'g, S>> {
        self.steps
    }

    /// Starts tracking on the first rewrite of this level, unless diagnostics are disabled.
    ///
    /// Must be called before the steps are changed: provenance starts from the level as it stands.
    fn rewrite_mut(&mut self) -> Option<&mut LevelPlan<'g, S>> {
        if !self.tracking {
            return None;
        }
        if self.rewrite.is_none() {
            self.rewrite = Some(LevelPlan::identity(&self.steps));
        }
        self.rewrite.as_mut()
    }

    /// Captures how a step reads before a rule changes it, unless it is already known.
    ///
    /// Several rules fold into the same step, so the rendering is taken at most once per step,
    /// and never for a step that is already recorded.
    fn remember(&mut self, index: usize) {
        let Some(rewrite) = self.rewrite_mut() else {
            return;
        };
        let Some(position) = rewrite.origins.get(index).map(|origin| origin.primary) else {
            return;
        };
        if rewrite.changed.iter().any(|(known, _)| *known == position) {
            return;
        }
        let name = self.steps[index].get_name();
        if let Some(rewrite) = self.rewrite.as_mut() {
            rewrite.changed.push((position, Rewritten::Rendered(name)));
        }
    }

    /// Mutable access to a step that stays in place, e.g. to fold labels into a scan.
    ///
    /// Counts as a rewrite: folding a filter into a scan changes how that step reads.
    pub(crate) fn step_mut(&mut self, index: usize) -> &mut GraphStep<'g, S> {
        self.remember(index);
        &mut self.steps[index]
    }

    /// Mutable iteration over the steps, for recursion into nested traversals.
    ///
    /// Nested levels track their own provenance, so this is not a rewrite of this level.
    pub(crate) fn steps_mut(&mut self) -> impl Iterator<Item = &mut GraphStep<'g, S>> {
        self.steps.iter_mut()
    }

    /// Removes the step at `removed` after its work has been folded into the step at `into`,
    /// which inherits its provenance.
    pub(crate) fn absorb(&mut self, removed: usize, into: usize) {
        debug_assert!(removed != into, "a step cannot absorb itself");
        self.rewrite_mut();
        let step = self.steps.remove(removed);
        let Some(rewrite) = self.rewrite.as_mut() else {
            return;
        };
        let origin = rewrite.origins.remove(removed);
        rewrite
            .changed
            .push((origin.primary, Rewritten::Detached(step)));
        let target = if into > removed { into - 1 } else { into };
        if let Some(target) = rewrite.origins.get_mut(target) {
            target.merge(origin);
        }
    }

    /// Stably reorders the steps in `start..end` by the given cost key, keeping provenance
    /// attached to the steps as they move.
    pub(crate) fn sort_block_by_key<F>(&mut self, start: usize, end: usize, key: F)
    where
        F: Fn(&GraphStep<'g, S>) -> usize,
    {
        let mut order: Vec<usize> = (start..end).collect();
        order.sort_by_key(|index| key(&self.steps[*index]));

        let mut origins: Vec<Option<StepOrigin>> = match self.rewrite_mut() {
            Some(rewrite) => rewrite.origins.drain(start..end).map(Some).collect(),
            None => Vec::new(),
        };
        let mut steps: Vec<Option<GraphStep<'g, S>>> =
            self.steps.drain(start..end).map(Some).collect();

        for (offset, index) in order.into_iter().enumerate() {
            let slot = index - start;
            if let Some(step) = steps[slot].take() {
                self.steps.insert(start + offset, step);
            }
            if let (Some(origin), Some(rewrite)) = (
                origins.get_mut(slot).and_then(Option::take),
                self.rewrite.as_mut(),
            ) {
                rewrite.origins.insert(start + offset, origin);
            }
        }
    }

    /// Detaches a step together with its provenance, so a rule can replace it.
    ///
    /// The step is rendered here, because the rule consumes it.
    pub(crate) fn take_step(&mut self, index: usize) -> (GraphStep<'g, S>, StepOrigin) {
        // `remember` starts tracking, so provenance is in sync before the step disappears.
        self.remember(index);
        let step = self.steps.remove(index);
        let origin = match self.rewrite.as_mut() {
            Some(rewrite) => rewrite.origins.remove(index),
            None => StepOrigin::new(index),
        };
        (step, origin)
    }

    /// Inserts a step the user never wrote (e.g. a `lazy_barrier()`): it stands for no logical
    /// position, so diagnostics rebuilding the user's query skip it.
    pub(crate) fn insert_synthetic(&mut self, index: usize, step: GraphStep<'g, S>) {
        self.rewrite_mut();
        self.steps.insert(index, step);
        if let Some(rewrite) = self.rewrite.as_mut() {
            rewrite.origins.insert(
                index,
                StepOrigin {
                    primary: u32::MAX,
                    absorbed: Vec::new(),
                },
            );
        }
    }

    /// Inserts steps that all originate from the same logical step, e.g. the inner steps of an
    /// unnested `where()`.
    pub(crate) fn insert_steps(
        &mut self,
        index: usize,
        steps: Vec<GraphStep<'g, S>>,
        origin: &StepOrigin,
    ) {
        let count = steps.len();
        self.rewrite_mut();
        self.steps.splice(index..index, steps);
        if let Some(rewrite) = self.rewrite.as_mut() {
            rewrite
                .origins
                .splice(index..index, std::iter::repeat_n(origin.clone(), count));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphSource;
    use crate::graph::TraversalGraph;
    use crate::traversal::__;

    /// Builds a plan from a traversal without running any rule.
    fn plan_of<'g>(
        traversal: crate::GraphTraversalSource<'g, &'g TraversalGraph>,
    ) -> Plan<'g, &'g TraversalGraph> {
        Plan::from_steps(traversal.take_steps(), true)
    }

    #[test]
    fn an_untouched_plan_tracks_nothing() {
        let graph = GraphSource::tinkerpop_modern();
        let locked = graph.read();
        let plan = plan_of(locked.traversal().v(None).values("age").max());

        let (steps, level) = plan.into_parts();
        assert_eq!(steps.len(), 3);
        assert!(
            level.is_none(),
            "a level no rule rewrote must not pay for provenance"
        );
    }

    #[test]
    fn absorbing_steps_rebuilds_the_original_level() {
        let graph = GraphSource::tinkerpop_modern();
        let locked = graph.read();
        let mut plan = plan_of(locked.traversal().v(None).has_label("person").count());

        // Both `has_label` and `count` fold into the init step.
        plan.absorb(1, 0);
        plan.absorb(1, 0);

        let (steps, level) = plan.into_parts();
        let level = level.expect("a rewritten level is tracked");
        let (names, marked) = level.resolve(&steps, 0);

        assert_eq!(steps.len(), 1);
        assert_eq!(
            names,
            vec![
                "v()".to_string(),
                "has_label(\"person\")".to_string(),
                "count()".to_string()
            ]
        );
        assert_eq!(marked, vec![0, 1, 2]);
    }

    #[test]
    fn a_step_rewritten_in_place_keeps_its_original_rendering() {
        let graph = GraphSource::tinkerpop_modern();
        let locked = graph.read();
        let mut plan = plan_of(locked.traversal().v(None).has_label("person").values("age"));

        if let GraphStep::Init(_, init) = plan.step_mut(0) {
            init.set_labels(vec!["person".to_string()]);
        }
        plan.absorb(1, 0);

        let (steps, level) = plan.into_parts();
        let level = level.expect("a rewritten level is tracked");
        let (names, marked) = level.resolve(&steps, 0);

        assert_eq!(steps[0].get_name(), "v(labels: [\"person\"])");
        assert_eq!(
            names,
            vec![
                "v()".to_string(),
                "has_label(\"person\")".to_string(),
                "values(\"age\")".to_string()
            ],
            "the level must read as the user wrote it, not as it executes"
        );
        assert_eq!(marked, vec![0, 1]);
    }

    #[test]
    fn sorting_a_block_moves_provenance_with_the_steps() {
        let graph = GraphSource::tinkerpop_modern();
        let locked = graph.read();
        let mut plan = plan_of(
            locked
                .traversal()
                .v(None)
                .where_t(__::out(None))
                .has_id("1"),
        );

        plan.sort_block_by_key(1, 3, |step| match step {
            GraphStep::Filter("has_id", _) => 0,
            _ => 1,
        });

        let (steps, level) = plan.into_parts();
        let level = level.expect("a rewritten level is tracked");
        assert!(matches!(steps[1], GraphStep::Filter("has_id", _)));
        assert_eq!(level.resolve(&steps, 1).1, vec![2]);
        assert_eq!(level.resolve(&steps, 2).1, vec![1]);
    }

    #[test]
    fn inserted_steps_inherit_the_origin_of_the_step_they_replace() {
        let graph = GraphSource::tinkerpop_modern();
        let locked = graph.read();
        let mut plan = plan_of(locked.traversal().v(None).has_label("person").count());

        let (_, origin) = plan.take_step(1);
        let inner = crate::traversal::AnonymousTraversal::new()
            .has_label("person")
            .has_id("1");
        plan.insert_steps(1, inner.steps, &origin);

        let (steps, level) = plan.into_parts();
        let level = level.expect("a rewritten level is tracked");
        assert_eq!(steps.len(), 4);
        assert_eq!(level.resolve(&steps, 1).1, vec![1]);
        assert_eq!(level.resolve(&steps, 2).1, vec![1]);
        assert_eq!(level.resolve(&steps, 3).1, vec![2]);
    }
}
