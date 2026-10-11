//! The `Scope` token (`Scope.local`/`Scope::local`).

use crate::script::*;

/// Scope marker that makes the vanilla dotted spelling `Scope.local` / `Scope.global` resolve
/// (also the PascalCase `Scope.Local` / `Scope.Global`, following the `Order` precedent).
///
/// Mirrors [`ScriptColumn`]: the tokens are properties of this marker (dot form) and constants
/// of the static module `Scope::local` / `Scope::global`. There are deliberately no bare `local`
/// / `global` constants: `local()` is a step and `global` is reserved by Rhai. The token itself
/// is [`crate::value::Scope`], registered with Rhai as `Scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptScope;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod ScopeModule {
    use super::TraversalScope;
    use crate::value::literal::TokenLiteral;

    /// `Scope.local`: the step works on the collection held by each traverser.
    pub const local: TraversalScope = TraversalScope::Local;
    /// `Scope.global`: the step works on the whole stream.
    pub const global: TraversalScope = TraversalScope::Global;
    /// PascalCase alias of `Scope::local`.
    pub const Local: TraversalScope = TraversalScope::Local;
    /// PascalCase alias of `Scope::global`.
    pub const Global: TraversalScope = TraversalScope::Global;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(scope: &mut TraversalScope) -> String {
        scope.token_literal()
    }
}
