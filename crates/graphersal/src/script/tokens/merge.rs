//! The `Merge` token of `mergeV()`/`mergeE()` options (`Merge.onCreate`, ...).

use crate::script::*;

/// Scope marker that makes the dotted spelling `Merge.onCreate` / `.onMatch` / `.outV` / `.inV`
/// resolve.
///
/// Mirrors [`ScriptScope`]; the static module gives `Merge::onCreate` .... No bare `onCreate`/
/// `outV` constants (`outV()` is a step). The token itself is [`crate::value::Merge`],
/// registered with Rhai as `Merge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptMerge;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod MergeModule {
    use super::Merge;
    use crate::value::literal::TokenLiteral;

    /// `Merge.onCreate`: the create-branch option of `mergeV()`/`mergeE()`.
    pub const onCreate: Merge = Merge::OnCreate;
    /// `Merge.onMatch`: the match-branch option of `mergeV()`/`mergeE()`.
    pub const onMatch: Merge = Merge::OnMatch;
    /// `Merge.outV`: the out-vertex option of `mergeE()`.
    pub const outV: Merge = Merge::OutV;
    /// `Merge.inV`: the in-vertex option of `mergeE()`.
    pub const inV: Merge = Merge::InV;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(merge: &mut Merge) -> String {
        merge.token_literal()
    }
}
