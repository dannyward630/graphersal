use crate::graph::GraphStorage;
use crate::*;

/// The step-level `with(WithOptions.key[, WithOptions.value])` modulator (TinkerPop's
/// `Configuring`): sets an option of the step before it. `valueMap()` takes
/// `WithOptions.tokens`, `index()` takes `WithOptions.indexer`; any other step, or an option the
/// step does not know, fails with [`TraverserError::InvalidModulator`] when the traversal runs.
///
/// Not to be confused with `g.with(key, value)`, which sets an execution option of the whole
/// traversal (`book/src/users_guide/options.md`).
#[derive(Debug, Clone)]
struct StepWith {
    option: Option<(WithOptions, Option<WithOptions>)>,
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `with(WithOptions.key)` / `with(WithOptions.key, WithOptions.value)`: configures the step
    /// right before it, see [`WithOptions`]. `value` is `None` for the one-argument form
    /// (`with(WithOptions.tokens)` means all tokens).
    pub fn with_option(self, key: WithOptions, value: Option<WithOptions>) -> Self {
        self.push_step(with_step(key, value))
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `with(WithOptions..)` in a child traversal; see [`GraphTraversalSource::with_option`].
    pub fn with_option(self, key: WithOptions, value: Option<WithOptions>) -> Self {
        self.push_step(with_step(key, value))
    }
}

fn with_step<'g, S: GraphStorage + 'g>(
    key: WithOptions,
    value: Option<WithOptions>,
) -> GraphStep<'g, S> {
    GraphStep::Modulator(
        "with",
        Box::new(StepWith {
            option: Some((key, value)),
        }),
    )
}

impl std::fmt::Display for StepWith {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.option {
            Some((key, Some(value))) => write!(f, "with({key}, {value})"),
            Some((key, None)) => write!(f, "with({key})"),
            None => f.write_str("with()"),
        }
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepModulator<'g, S> for StepWith {
    fn get_with_option(&mut self) -> Option<(WithOptions, Option<WithOptions>)> {
        self.option.take()
    }

    fn clone_box(&self) -> Box<dyn GraphStepModulator<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
