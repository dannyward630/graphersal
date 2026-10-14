//! `date_diff()` (TinkerPop `dateDiff()`): the difference of two dates in milliseconds, or in
//! whole units of time.

use crate::graph::GraphStorage;
use crate::traversal::optimizer::path_analysis::{PathAnalysis, PathRequirement};
use crate::traversal::step::map_steps::temporal::{
    Operand, ZoneArg, check_zone, epoch_of, incoming_temporal, replaced, temporal_error, zone_of,
};
use crate::value::dt::DT;
use crate::value::literal::TokenLiteral;
use crate::value::temporal::arith;
use crate::*;

/// `date_diff(other)`, `date_diff(other, DT)`, `date_diff(other, DT, zone)`; `other` is a
/// constant or a child traversal whose first result is the operand.
///
/// `date_diff(other)` is TinkerPop's `dateDiff()`: the incoming date minus `other`, in
/// milliseconds (`int64`; each operand's epoch milliseconds rounded down); a `null` operand is
/// the epoch. Graphersal extends it to the other date and time types (both operands of the same
/// type: a `date` counts from midnight, a `time` within the day; `null` is 1970-01-01 or
/// midnight) and adds `date_diff(other, DT.unit[, zone])`: whole units, truncated toward zero
/// (units below a day from the elapsed time, days and weeks from the wall-clock dates and times,
/// months and larger as `java.time` `ChronoUnit.between` counts them; a `datetime` on the wall
/// clock of `other`'s offset, or of `zone`).
struct StepDateDiff<'g, S: GraphStorage + 'g> {
    other: Operand<'g, S>,
    unit: Option<DT>,
    zone: Option<ZoneArg>,
}

impl<'g, S: GraphStorage + 'g> Clone for StepDateDiff<'g, S> {
    fn clone(&self) -> Self {
        Self {
            other: self.other.clone(),
            unit: self.unit,
            zone: self.zone.clone(),
        }
    }
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepDateDiff<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "date_diff({}", self.other)?;
        if let Some(unit) = self.unit {
            write!(f, ", {}", unit.token_literal())?;
        }
        if let Some(zone) = &self.zone {
            write!(f, ", {}", zone.literal())?;
        }
        f.write_str(")")
    }
}

fn step<'g, S: GraphStorage + 'g>(
    other: Operand<'g, S>,
    unit: Option<DT>,
    zone: Option<&str>,
) -> GraphStep<'g, S> {
    GraphStep::Map(
        "date_diff",
        Box::new(StepDateDiff {
            other,
            unit,
            zone: zone.map(ZoneArg::parse),
        }),
    )
}

macro_rules! date_diff_builders {
    () => {
        /// `date_diff(other)` (TinkerPop `dateDiff()`): the incoming date minus `other` in
        /// milliseconds (`int64`); `other` is a date or time of the same type, or `null` (the
        /// epoch). Book part "Dates and Times".
        pub fn date_diff(self, other: impl Into<ElementProperty>) -> Self {
            self.push_step(step(Operand::Value(other.into()), None, None))
        }

        /// `date_diff(traversal)`: like [`date_diff`](Self::date_diff), the operand being the
        /// first result of `traversal`, run on the incoming traverser.
        pub fn date_diff_traversal(self, other: AnonymousTraversal<'g, S>) -> Self {
            self.push_step(step(Operand::Traversal(other), None, None))
        }

        /// `date_diff(other, DT, zone)`: the difference in whole `unit`s, truncated toward zero
        /// (a Graphersal extension); `zone` (optional) is the zone on whose wall clock two
        /// `datetime`s are compared for calendar units.
        pub fn date_diff_in(
            self,
            other: impl Into<ElementProperty>,
            unit: DT,
            zone: Option<&str>,
        ) -> Self {
            self.push_step(step(Operand::Value(other.into()), Some(unit), zone))
        }

        /// `date_diff(traversal, DT, zone)`; see [`date_diff_in`](Self::date_diff_in).
        pub fn date_diff_traversal_in(
            self,
            other: AnonymousTraversal<'g, S>,
            unit: DT,
            zone: Option<&str>,
        ) -> Self {
            self.push_step(step(Operand::Traversal(other), Some(unit), zone))
        }
    };
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    date_diff_builders!();
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    date_diff_builders!();
}

impl __ {
    /// Anonymous `date_diff(other)`; see [`GraphTraversalSource::date_diff`].
    pub fn date_diff<'g, S: GraphStorage + 'g>(
        other: impl Into<ElementProperty>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().date_diff(other)
    }

    /// Anonymous `date_diff(traversal)`; see [`GraphTraversalSource::date_diff_traversal`].
    pub fn date_diff_traversal<'g, S: GraphStorage + 'g>(
        other: AnonymousTraversal<'g, S>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().date_diff_traversal(other)
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepDateDiff<'g, S> {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        let value = incoming_temporal(context, &source.value, "date_diff", "a date or time")?;
        let other = match self.other.resolve(context, &source, "date_diff")? {
            Some(other) => other,
            // TinkerPop: a `null` operand is the epoch.
            None => epoch_of(value.kind()).ok_or_else(|| {
                temporal_error(
                    "date_diff",
                    crate::value::temporal::TemporalError::unsupported(
                        "the difference",
                        value.kind(),
                    ),
                )
            })?,
        };
        let zone = zone_of(&self.zone, "date_diff")?;
        let result = match self.unit {
            None => arith::diff_millis(value, other),
            Some(unit) => arith::diff(value, other, unit, zone),
        }
        .map_err(|error| temporal_error("date_diff", error))?;
        Ok(replaced(
            source,
            TraverserValue::Computed(ElementProperty::Number(Number::from_i64(result)), None),
        ))
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn plan_check(&self) -> TraverserResult<()> {
        check_zone(&self.zone, "date_diff")?;
        self.other.check("date_diff")
    }

    fn analyze_nested_paths(
        &mut self,
        analysis: &PathAnalysis,
        _downstream: &PathRequirement,
    ) -> PathRequirement {
        // The child traversal runs on the incoming traverser, path included; its output does
        // not continue the path.
        self.other.analyze_paths(analysis)
    }

    fn child_traversals_mut(&mut self) -> Vec<&mut AnonymousTraversal<'g, S>> {
        self.other.children()
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
