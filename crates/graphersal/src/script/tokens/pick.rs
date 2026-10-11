//! The `Pick` token of `choose()`/`branch()` options (`Pick.any`/`Pick::Any`, ...).

use crate::script::*;

/// `choose()`/`branch()`'s `Pick.any`/`Pick.none`/`Pick.unproductive` tokens (TinkerPop `Pick`), the sentinel half of
/// a chained `.option(pick, traversal)`'s `pick` argument — the other half is a plain scalar
/// value, a predicate or a traversal key (see `OptionToken`). Dual-registered like `Order`/`By`/`GType`:
/// `Pick::Any`/`Pick::None` (`::`, `PickModule` below) and `Pick.any`/`Pick.none` (dot, via
/// `ScriptPick`), lowercase for the dot form to match real Gremlin-Groovy's own `Pick.any`/
/// `Pick.none` spelling, same convention `Order.asc`/`Order.desc` already established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pick {
    /// Any one of the matching branches.
    Any,
    /// None of the matching branches.
    None,
    /// The selector produced nothing for the traverser.
    Unproductive,
}

/// Scope marker that makes the vanilla dotted spelling `Pick.any`/`Pick.none` resolve. See
/// [`ScriptOrder`]'s doc comment for the shared rationale (lowercase dot access to match real
/// Gremlin-Groovy's own `Pick.any`/`Pick.none` spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptPick;

impl crate::value::literal::TokenLiteral for Pick {
    fn token_literal(&self) -> String {
        match self {
            Pick::Any => "Pick.any",
            Pick::None => "Pick.none",
            Pick::Unproductive => "Pick.unproductive",
        }
        .to_string()
    }
}

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod PickModule {
    use super::Pick;
    use crate::value::literal::TokenLiteral;

    /// `Pick.any`: `branch()`'s always-additional catch-all option.
    pub const Any: Pick = Pick::Any;
    /// `Pick.none`: the fallback option run when no explicit `.option()` matched.
    pub const None: Pick = Pick::None;
    /// `Pick.unproductive`: the option run when the selector produced nothing.
    pub const Unproductive: Pick = Pick::Unproductive;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(pick: &mut Pick) -> String {
        pick.token_literal()
    }
}
