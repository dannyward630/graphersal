//! The `SchemaMode` token of `set_schema(schema, mode)` (`SchemaMode.closed`/`SchemaMode::closed`).

use crate::script::*;

/// Scope marker that makes the dotted spelling `SchemaMode.none` / `.open` / `.closed` resolve.
///
/// Mirrors [`ScriptScope`]: the tokens are properties of this marker (dot form) and constants of
/// the static module `SchemaMode::none` ... No bare constants (`none`, `open` and `closed` are
/// too generic to reserve). The token itself is [`SchemaMode`], registered with Rhai as
/// `SchemaMode`; wherever it is accepted, the strings `"none"`, `"open"` and `"closed"` are
/// accepted too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptSchemaMode;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod SchemaModeModule {
    use super::SchemaMode;
    use crate::value::literal::TokenLiteral;

    /// `SchemaMode.none`: the schema describes, nothing is enforced.
    pub const none: SchemaMode = SchemaMode::None;
    /// `SchemaMode.open`: declared labels and properties are enforced, undeclared ones allowed.
    pub const open: SchemaMode = SchemaMode::Open;
    /// `SchemaMode.closed`: only declared labels, properties and connections are admitted.
    pub const closed: SchemaMode = SchemaMode::Closed;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(mode: &mut SchemaMode) -> String {
        mode.token_literal()
    }
}
