//! The `Order` token (`Order.asc`/`Order::Asc`, ...).

use crate::script::*;

/// Scope marker that makes the vanilla dotted spelling `Order.asc`/`Order.desc` resolve. See
/// [`ScriptBy`]'s doc comment for the shared rationale — `Order` never had either half of this
/// dual registration before the `order()` comparison-infra spec (part 1 of the `order()` series)
/// added it, alongside the `::`-access `OrderModule` above, following the exact `By`/`CastPolicy`
/// precedent rather than inventing a new mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptOrder;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod OrderModule {
    use super::Order;
    use crate::value::literal::TokenLiteral;

    /// Ascending natural order.
    pub const Asc: Order = Order::Asc;
    /// Descending natural order.
    pub const Desc: Order = Order::Desc;
    /// Random order, drawn from the execution's random number generator.
    pub const Shuffle: Order = Order::Shuffle;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(order: &mut Order) -> String {
        order.token_literal()
    }
}
