//! The `V` visualizer-format token (`V.Table`/`V::Table`, ...).

use crate::script::*;

/// Scope marker for the `V(...)`/`V::keys`-style vanilla-Gremlin `V` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptV;

#[export_module]
#[allow(non_upper_case_globals)]
pub(crate) mod script_v_module {
    use super::VisualizeFormat;
    use crate::value::literal::TokenLiteral;

    pub const Auto: VisualizeFormat = VisualizeFormat::Auto;
    pub const Table: VisualizeFormat = VisualizeFormat::Table;
    pub const Markdown: VisualizeFormat = VisualizeFormat::Markdown;
    pub const Json: VisualizeFormat = VisualizeFormat::Json;
    pub const JsonSchema: VisualizeFormat = VisualizeFormat::JsonSchema;
    pub const Mermaid: VisualizeFormat = VisualizeFormat::Mermaid;
    pub const PlantUML: VisualizeFormat = VisualizeFormat::PlantUML;
    pub const Tree: VisualizeFormat = VisualizeFormat::Tree;

    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(v: &mut VisualizeFormat) -> String {
        v.token_literal()
    }
}
