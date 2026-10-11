//! The `WithOptions` token (`WithOptions.tokens`, `WithOptions.indexer`, ...) of the step-level
//! `with()` modulator.

use crate::script::*;

/// Scope marker that makes the dotted spelling `WithOptions.tokens` resolve.
///
/// Mirrors [`ScriptBarrier`]; the static module gives `WithOptions::tokens`. No bare constants.
/// The token itself is [`crate::value::WithOptions`], registered with Rhai as `WithOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptWithOptions;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod WithOptionsModule {
    use crate::value::WithOptions;
    use crate::value::literal::TokenLiteral;

    /// `WithOptions.tokens`: the `valueMap()` tokens option.
    pub const tokens: WithOptions = WithOptions::Tokens;
    /// `WithOptions.none`: no token.
    pub const none: WithOptions = WithOptions::None;
    /// `WithOptions.ids`: the id token.
    pub const ids: WithOptions = WithOptions::Ids;
    /// `WithOptions.labels`: the label token.
    pub const labels: WithOptions = WithOptions::Labels;
    /// `WithOptions.keys`: the key token of a vertex property.
    pub const keys: WithOptions = WithOptions::Keys;
    /// `WithOptions.values`: the value token of a vertex property.
    pub const values: WithOptions = WithOptions::Values;
    /// `WithOptions.all`: every token.
    pub const all: WithOptions = WithOptions::All;
    /// `WithOptions.indexer`: the `index()` indexer option.
    pub const indexer: WithOptions = WithOptions::Indexer;
    /// `WithOptions.list`: the list indexer.
    pub const list: WithOptions = WithOptions::List;
    /// `WithOptions.map`: the map indexer.
    pub const map: WithOptions = WithOptions::Map;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(option: &mut WithOptions) -> String {
        option.token_literal()
    }

    /// Equality
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(left: &mut WithOptions, right: WithOptions) -> bool {
        *left == right
    }

    /// Inequality
    #[rhai_fn(global, name = "!=", pure)]
    pub fn ne(left: &mut WithOptions, right: WithOptions) -> bool {
        *left != right
    }
}
