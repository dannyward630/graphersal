//! The `Pop` token of `select()` (`Pop.first`/`Pop::first`/`first`, ...).

use crate::script::*;

/// Scope marker that makes the dotted spelling `Pop.first` / `Pop.last` / `Pop.all` / `Pop.mixed`
/// resolve (also the PascalCase `Pop.First` ... as for `Scope.Local`).
///
/// Mirrors [`ScriptScope`]: the tokens are properties of this marker (dot form) and constants of
/// the static module `Pop::first` ...; the graph scope also binds the bare constants `first`,
/// `last`, `all` and `mixed`, the Gremlin console's `import static Pop.*`, like `keys` / `values`
/// for [`Column`]. They coexist with the steps of the same name (`all(P)`), because a scope
/// constant and a method share no namespace; a script variable of the same name shadows the
/// constant for the rest of the scope. The token itself is [`crate::value::Pop`], registered with
/// Rhai as `Pop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptPop;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod PopModule {
    use super::Pop;
    use crate::value::literal::TokenLiteral;

    /// `Pop.first`: the oldest occurrence of a label.
    pub const first: Pop = Pop::First;
    /// `Pop.last`: the newest occurrence of a label (what `select("a")` reads).
    pub const last: Pop = Pop::Last;
    /// `Pop.all`: a list of every occurrence, in path order.
    pub const all: Pop = Pop::All;
    /// `Pop.mixed`: the value for one occurrence, a list for several.
    pub const mixed: Pop = Pop::Mixed;
    /// PascalCase alias of `Pop::first`.
    pub const First: Pop = Pop::First;
    /// PascalCase alias of `Pop::last`.
    pub const Last: Pop = Pop::Last;
    /// PascalCase alias of `Pop::all`.
    pub const All: Pop = Pop::All;
    /// PascalCase alias of `Pop::mixed`.
    pub const Mixed: Pop = Pop::Mixed;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(pop: &mut Pop) -> String {
        pop.token_literal()
    }

    /// Equality of two pop tokens.
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(left: &mut Pop, right: Pop) -> bool {
        *left == right
    }

    /// Inequality of two pop tokens.
    #[rhai_fn(global, name = "!=", pure)]
    pub fn ne(left: &mut Pop, right: Pop) -> bool {
        *left != right
    }
}
