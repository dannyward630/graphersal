//! The `Column` token (`Column.keys`/`Column::keys`/`keys`).

use crate::script::*;

/// Scope marker that makes the vanilla dotted spelling `Column.keys` / `Column.values` resolve.
///
/// Mirrors [`ScriptGType`]: the tokens are properties of this marker (dot form) and constants of
/// a static module (`Column::keys`). The graph scope also binds the bare constants `keys` and
/// `values`, the Gremlin console's `import static Column.*`, so `select(keys)` works verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptColumn;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod ColumnModule {
    use super::Column;
    use crate::value::literal::TokenLiteral;

    /// `Column.keys`: the keys of a map, or the key of an entry.
    pub const keys: Column = Column::Keys;
    /// `Column.values`: the values of a map, or the value of an entry.
    pub const values: Column = Column::Values;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(column: &mut Column) -> String {
        column.token_literal()
    }
}
