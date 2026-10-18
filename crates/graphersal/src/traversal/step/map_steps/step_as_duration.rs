//! `as_duration()`: converts ISO 8601 duration text to a `duration`.

use crate::graph::GraphStorage;
use crate::traversal::step::map_steps::step_as_date::parse_error;
use crate::traversal::step::map_steps::temporal::{computed, describe, held_value, replaced};
use crate::value::temporal::{Duration, Temporal};
use crate::*;

/// `as_duration()` (a Graphersal extension; TinkerPop has no duration conversion step): a
/// `duration` passes unchanged, a string is parsed as an ISO 8601 duration (`P1Y2M3DT4H5M6S`,
/// `P2W`, `PT1.5S`, a sign per part), anything else fails with "Can't parse".
#[derive(Clone, Default)]
struct StepAsDuration;

impl fmt::Display for StepAsDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("as_duration()")
    }
}

fn step<'g, S: GraphStorage + 'g>() -> GraphStep<'g, S> {
    GraphStep::Map("as_duration", Box::new(StepAsDuration))
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `as_duration()`: converts ISO 8601 duration text (`"P1DT2H"`) to a `duration`; a
    /// `duration` passes, anything else fails ("Can't parse"). Book part "Dates and Times".
    pub fn as_duration(self) -> Self {
        self.push_step(step())
    }
}

impl __ {
    /// Anonymous `as_duration()`.
    pub fn as_duration<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().as_duration()
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `as_duration()`; see [`GraphTraversalSource::as_duration`].
    pub fn as_duration(self) -> Self {
        self.push_step(step())
    }
}

/// Converts one value to a duration.
fn convert(value: &ElementProperty) -> Result<Duration, String> {
    let parse = |text: &str| {
        text.parse::<Duration>().map_err(|err| match err {
            crate::value::temporal::TemporalError::Parse { reason, .. } => reason.to_string(),
            other => other.to_string(),
        })
    };
    match value {
        ElementProperty::Temporal(Temporal::Duration(duration)) => Ok(*duration),
        ElementProperty::String(text) => parse(text),
        ElementProperty::Compressed(text) => text.with_text(parse),
        ElementProperty::Deduped(text) => parse(text.as_str()),
        ElementProperty::Null => Err("null is not a duration".to_string()),
        other => Err(format!(
            "a {} is not a duration",
            crate::graph::schema::value_type_name(other)
        )),
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepAsDuration {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        let converted = match held_value(context, &source.value)? {
            Ok(held) => convert(held.as_ref()).map_err(|reason| {
                parse_error("as_duration", "duration", describe(&held), reason)
            })?,
            Err(kind) => {
                return Err(parse_error(
                    "as_duration",
                    "duration",
                    format!("a {kind}"),
                    format!("a {kind} is not a duration"),
                ));
            }
        };
        Ok(replaced(source, computed(Temporal::Duration(converted))))
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
