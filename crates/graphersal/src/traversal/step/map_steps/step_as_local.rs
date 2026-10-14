//! `as_local()`: the wall clock of a date-time.

use crate::graph::GraphStorage;
use crate::traversal::step::map_steps::temporal::{
    ZoneArg, check_zone, computed, describe, incoming_temporal, input_error, replaced,
    temporal_error, zone_of,
};
use crate::value::temporal::Temporal;
use crate::*;

/// `as_local()`, `as_local(zone)` (a Graphersal extension; PostgreSQL `timestamptz AT TIME ZONE`
/// giving a `timestamp`, `java.time` `toLocalDateTime`): the `localdatetime` a `datetime` shows
/// on its own wall clock, or on the wall clock of `zone`. A `localdatetime` passes unchanged
/// (without a zone).
#[derive(Clone)]
struct StepAsLocal {
    zone: Option<ZoneArg>,
}

impl fmt::Display for StepAsLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.zone {
            Some(zone) => write!(f, "as_local({})", zone.literal()),
            None => f.write_str("as_local()"),
        }
    }
}

fn step<'g, S: GraphStorage + 'g>(zone: Option<&str>) -> GraphStep<'g, S> {
    GraphStep::Map(
        "as_local",
        Box::new(StepAsLocal {
            zone: zone.map(ZoneArg::parse),
        }),
    )
}

macro_rules! as_local_builders {
    () => {
        /// `as_local()`: the `localdatetime` a `datetime` shows on its own wall clock. Book
        /// part "Dates and Times".
        pub fn as_local(self) -> Self {
            self.push_step(step(None))
        }

        /// `as_local(zone)`: the `localdatetime` a `datetime` shows on the wall clock of
        /// `zone`.
        pub fn as_local_in(self, zone: &str) -> Self {
            self.push_step(step(Some(zone)))
        }
    };
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    as_local_builders!();
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    as_local_builders!();
}

impl __ {
    /// Anonymous `as_local()`; see [`GraphTraversalSource::as_local`].
    pub fn as_local<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().as_local()
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepAsLocal {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        const EXPECTED: &str = "a datetime";
        let value = incoming_temporal(context, &source.value, "as_local", EXPECTED)?;
        let zone = zone_of(&self.zone, "as_local")?;
        let result = match (value, zone) {
            (Temporal::DateTime(v), Some(zone)) => zone
                .at(v)
                .map(|v| v.local())
                .map_err(|error| temporal_error("as_local", error))?,
            (Temporal::DateTime(v), None) => v.local(),
            (Temporal::LocalDateTime(v), None) => v,
            (other, _) => {
                return Err(input_error(
                    "as_local",
                    "incoming value",
                    EXPECTED,
                    describe(&ElementProperty::Temporal(other)),
                ));
            }
        };
        Ok(replaced(source, computed(Temporal::LocalDateTime(result))))
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn plan_check(&self) -> TraverserResult<()> {
        check_zone(&self.zone, "as_local")
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
