//! `age()`: the difference of two dates as a `duration` (PostgreSQL `age`).

use crate::graph::GraphStorage;
use crate::traversal::optimizer::path_analysis::{PathAnalysis, PathRequirement};
use crate::traversal::step::map_steps::temporal::{
    Operand, ZoneArg, check_zone, computed, incoming_temporal, input_error, replaced,
    temporal_error, zone_of,
};
use crate::value::temporal::{Temporal, arith};
use crate::*;

/// `age(other)`, `age(other, zone)` (a Graphersal extension): the incoming date minus `other` as
/// PostgreSQL's `age` computes it, a `duration` of whole years and months, days and a time part,
/// all with the same sign (`date("2001-04-10")` with `age(date("1957-06-13"))` is
/// `P43Y9M27D`, the reverse its negation). Both operands have the same type; two `datetime`s are compared on the wall clock
/// of `other`'s offset, or of `zone`. `other` is a constant or the first result of a child
/// traversal; `null` is an error.
struct StepAge<'g, S: GraphStorage + 'g> {
    other: Operand<'g, S>,
    zone: Option<ZoneArg>,
}

impl<'g, S: GraphStorage + 'g> Clone for StepAge<'g, S> {
    fn clone(&self) -> Self {
        Self {
            other: self.other.clone(),
            zone: self.zone.clone(),
        }
    }
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepAge<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "age({}", self.other)?;
        if let Some(zone) = &self.zone {
            write!(f, ", {}", zone.literal())?;
        }
        f.write_str(")")
    }
}

fn step<'g, S: GraphStorage + 'g>(other: Operand<'g, S>, zone: Option<&str>) -> GraphStep<'g, S> {
    GraphStep::Map(
        "age",
        Box::new(StepAge {
            other,
            zone: zone.map(ZoneArg::parse),
        }),
    )
}

macro_rules! age_builders {
    () => {
        /// `age(other)` / `age(other, zone)`: the incoming date minus `other` as a `duration`
        /// of years and months, days and time, every part with the same sign (PostgreSQL
        /// `age`). Book part "Dates and Times".
        pub fn age(self, other: impl Into<ElementProperty>, zone: Option<&str>) -> Self {
            self.push_step(step(Operand::Value(other.into()), zone))
        }

        /// `age(traversal)` / `age(traversal, zone)`: like [`age`](Self::age), the operand
        /// being the first result of `traversal`, run on the incoming traverser.
        pub fn age_traversal(self, other: AnonymousTraversal<'g, S>, zone: Option<&str>) -> Self {
            self.push_step(step(Operand::Traversal(other), zone))
        }
    };
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    age_builders!();
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    age_builders!();
}

impl __ {
    /// Anonymous `age(other)`; see [`GraphTraversalSource::age`].
    pub fn age<'g, S: GraphStorage + 'g>(
        other: impl Into<ElementProperty>,
        zone: Option<&str>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().age(other, zone)
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepAge<'g, S> {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        let value = incoming_temporal(context, &source.value, "age", "a date or time")?;
        let Some(other) = self.other.resolve(context, &source, "age")? else {
            return Err(input_error(
                "age",
                "argument",
                "a date or time",
                "null".to_string(),
            ));
        };
        let zone = zone_of(&self.zone, "age")?;
        let result =
            arith::age(value, other, zone).map_err(|error| temporal_error("age", error))?;
        Ok(replaced(source, computed(Temporal::Duration(result))))
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn plan_check(&self) -> TraverserResult<()> {
        check_zone(&self.zone, "age")?;
        self.other.check("age")
    }

    fn analyze_nested_paths(
        &mut self,
        analysis: &PathAnalysis,
        _downstream: &PathRequirement,
    ) -> PathRequirement {
        self.other.analyze_paths(analysis)
    }

    fn child_traversals_mut(&mut self) -> Vec<&mut AnonymousTraversal<'g, S>> {
        self.other.children()
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
