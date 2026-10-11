//! The `Barrier` token (`Barrier.normSack`).

use crate::script::*;

/// Scope marker that makes the dotted spelling `Barrier.normSack` resolve.
///
/// Mirrors [`ScriptOperator`]; the static module gives `Barrier::normSack`. No bare `normSack`
/// constant. The token itself is [`crate::value::Barrier`], registered with Rhai as `Barrier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptBarrier;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod BarrierModule {
    use super::Barrier;
    use crate::value::literal::TokenLiteral;

    /// `Barrier.normSack`: normalise the numeric sacks of the stream to sum to 1.
    pub const normSack: Barrier = Barrier::NormSack;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(barrier: &mut Barrier) -> String {
        barrier.token_literal()
    }

    /// Equality
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(left: &mut Barrier, right: Barrier) -> bool {
        *left == right
    }

    /// Inequality
    #[rhai_fn(global, name = "!=", pure)]
    pub fn ne(left: &mut Barrier, right: Barrier) -> bool {
        *left != right
    }
}
