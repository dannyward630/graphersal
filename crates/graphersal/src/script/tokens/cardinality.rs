//! The `Cardinality` token and the `Cardinality.single(v)` value wrapper.

use crate::script::*;

/// Scope marker that makes the dotted spelling `Cardinality.single` / `.list` / `.set` resolve,
/// and the value-wrapping calls `Cardinality.single(v)` / `.list(v)` / `.set(v)`.
///
/// Mirrors [`ScriptScope`]: the tokens are properties of this marker (dot form) and constants of
/// the static module `Cardinality::single` ...; `Cardinality::single(v)` is a function of the
/// same module. No bare `single`/`list`/`set` constants (explicit class prefixes only). The token
/// itself is [`crate::value::Cardinality`], registered with Rhai as `Cardinality`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptCardinality;

/// A value tagged with a property cardinality: what `Cardinality.single(v)` (Gremlin
/// `single(v)`) evaluates to in the DSL, for `mergeV()` option maps. Script-only, registered
/// with Rhai as `CardinalityValue`.
#[derive(Debug, Clone)]
pub(crate) struct CardinalityValue {
    /// The requested cardinality.
    pub(crate) cardinality: Cardinality,
    /// The wrapped script value.
    pub(crate) value: Dynamic,
}

impl CardinalityValue {
    pub(crate) fn new(cardinality: Cardinality, value: Dynamic) -> Self {
        Self { cardinality, value }
    }
}

/// `Cardinality.single(5)`: the token and the wrapped value as a literal.
impl crate::value::literal::TokenLiteral for CardinalityValue {
    fn token_literal(&self) -> String {
        format!(
            "{}({})",
            crate::value::literal::TokenLiteral::token_literal(&self.cardinality),
            crate::script::render::value_literal(&self.value)
        )
    }
}

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod CardinalityModule {
    use super::{Cardinality, CardinalityValue};
    use crate::value::literal::TokenLiteral;

    /// `Cardinality.single`: one value per key.
    pub const single: Cardinality = Cardinality::Single;
    /// `Cardinality.list`: several values per key.
    pub const list: Cardinality = Cardinality::List;
    /// `Cardinality.set`: several distinct values per key.
    pub const set: Cardinality = Cardinality::Set;

    /// `Cardinality::single(v)`: `v` tagged with `Cardinality.single`.
    #[rhai_fn(name = "single")]
    pub fn single_value(value: Dynamic) -> CardinalityValue {
        CardinalityValue::new(Cardinality::Single, value)
    }
    /// `Cardinality::list(v)`: `v` tagged with `Cardinality.list`.
    #[rhai_fn(name = "list")]
    pub fn list_value(value: Dynamic) -> CardinalityValue {
        CardinalityValue::new(Cardinality::List, value)
    }
    /// `Cardinality::set(v)`: `v` tagged with `Cardinality.set`.
    #[rhai_fn(name = "set")]
    pub fn set_value(value: Dynamic) -> CardinalityValue {
        CardinalityValue::new(Cardinality::Set, value)
    }

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(cardinality: &mut Cardinality) -> String {
        cardinality.token_literal()
    }

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn value_to_string(value: &mut CardinalityValue) -> String {
        value.token_literal()
    }
}
