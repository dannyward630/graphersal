//! The `Operator` token of the sack and reducer steps (`Operator.sum`, ...).

use crate::script::*;

/// Scope marker that makes the vanilla dotted spelling `Operator.sum` / `Operator.addAll` resolve.
///
/// Mirrors [`ScriptMerge`]; the static module gives `Operator::sum` .... No bare constants (`sum()`
/// and `max()` are steps). The token itself is [`crate::value::Operator`], registered with Rhai as
/// `Operator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptOperator;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod OperatorModule {
    use super::Operator;
    use crate::value::literal::TokenLiteral;

    /// `Operator.sum`: addition.
    pub const sum: Operator = Operator::Sum;
    /// `Operator.minus`: subtraction.
    pub const minus: Operator = Operator::Minus;
    /// `Operator.mult`: multiplication.
    pub const mult: Operator = Operator::Mult;
    /// `Operator.div`: division.
    pub const div: Operator = Operator::Div;
    /// `Operator.max`: the larger value.
    pub const max: Operator = Operator::Max;
    /// `Operator.min`: the smaller value.
    pub const min: Operator = Operator::Min;
    /// `Operator.assign`: the right operand.
    pub const assign: Operator = Operator::Assign;
    /// `Operator.and`: logical and.
    pub const and: Operator = Operator::And;
    /// `Operator.or`: logical or.
    pub const or: Operator = Operator::Or;
    /// `Operator.addAll`: list concatenation or map merge.
    pub const addAll: Operator = Operator::AddAll;
    /// `Operator.sumLong`: int64 addition.
    pub const sumLong: Operator = Operator::SumLong;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(operator: &mut Operator) -> String {
        operator.token_literal()
    }

    /// Equality of two operator tokens.
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(left: &mut Operator, right: Operator) -> bool {
        *left == right
    }

    /// Inequality of two operator tokens.
    #[rhai_fn(global, name = "!=", pure)]
    pub fn ne(left: &mut Operator, right: Operator) -> bool {
        *left != right
    }
}
