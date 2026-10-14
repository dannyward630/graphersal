//! `select(T.id)` / `select(T.label)` / `select(Direction.OUT)` / `select(Direction.IN)`: reads a
//! token entry of a map, as `elementMap()` and `valueMap(true)` build them (a Graphersal
//! extension: TinkerPop's `select()` takes no `T` token).
//!
//! A map built by `elementMap()` holds the element's id and label under the tokens `T.id` and
//! `T.label`, beside the string keys of its properties, so `select("label")` reads a property
//! named `label` and `select(T.label)` the element's label. A value without that token entry
//! (a map without it, any other value) is filtered, like `select("key")` on a missing key.
//! The step never reads `Traverser.path`, so it keeps the default `path_requirement()`.

use crate::graph::GraphStorage;
use crate::*;

#[derive(Clone)]
struct StepSelectToken {
    key: RecordKey,
}

impl fmt::Display for StepSelectToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token = match &self.key {
            RecordKey::Id => "T.id",
            RecordKey::Label => "T.label",
            RecordKey::Out => "Direction.OUT",
            RecordKey::In => "Direction.IN",
            RecordKey::Name(name) => return write!(f, "select({})", literal::rhai_string(name)),
        };
        write!(f, "select({token})")
    }
}

/// The physical `select(token)` step, shared by the source and anonymous builders.
pub(super) fn select_token_step<'g, S: GraphStorage + 'g>(key: RecordKey) -> GraphStep<'g, S> {
    GraphStep::Map("select", Box::new(StepSelectToken { key }))
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepSelectToken {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        // Only reached by a caller that ignores `may_filter()`; a miss is then an error rather
        // than a silent drop.
        self.process_filter(context, source)?.ok_or_else(|| {
            Box::new(TraverserError::QueryExecution {
                query: self.to_string(),
                reason: "the value holds no such token entry".to_string(),
            })
        })
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn process_filter(
        &self,
        _context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Option<Traverser<S>>> {
        let found = match &source.value {
            TraverserValue::Record { keys, values } => keys
                .iter()
                .position(|key| *key == self.key)
                .and_then(|index| values.get(index).cloned()),
            _ => None,
        };
        Ok(found.map(|value| Traverser::with_parts(value, source.path, source.bulk, source.sack)))
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
