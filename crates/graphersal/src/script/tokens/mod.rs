//! The DSL token classes, one file per class: the scope marker types behind the dotted spelling
//! (`Order.asc`), the `#[export_module]` static modules behind the `::` spelling (`Order::Asc`),
//! and the script-side value types (`ArgP`, `ArgBy`, `ScriptUuid`, `ScriptSet`, `ScriptMap`, ...).
//! The registration with the engine happens in `engine_with_limits`.

mod barrier;
mod by;
mod cardinality;
mod cast_policy;
mod column;
mod direction;
mod gtype;
mod map;
mod merge;
mod operator;
mod order;
mod pick;
mod pop;
mod predicate;
mod profile_type;
mod schema_mode;
mod scope;
mod set;
mod t;
mod uuid;
mod visualizer;
mod with_options;

pub use barrier::*;
pub use by::*;
pub use cardinality::*;
pub use cast_policy::*;
pub use column::*;
pub use direction::*;
pub use gtype::*;
pub use map::*;
pub use merge::*;
pub use operator::*;
pub use order::*;
pub use pick::*;
pub use pop::*;
pub use predicate::*;
pub use profile_type::*;
pub use schema_mode::*;
pub use scope::*;
pub use set::*;
pub use t::*;
pub use uuid::*;
pub use visualizer::*;
pub use with_options::*;

/// The scope markers render as their class name (`Scope`, `P`, `T`, ...): the global that
/// evaluates to them. This list is the one place that names every marker; the session renderer
/// and the `to_string`/`to_debug` registrations go through `for_each_marker!`.
macro_rules! marker_literals {
    ($($marker:ident => $name:literal),* $(,)?) => {
        $(
            impl crate::value::literal::TokenLiteral for $marker {
                fn token_literal(&self) -> String {
                    $name.to_string()
                }
            }
        )*

        /// Calls `$m!` with every scope marker type.
        macro_rules! for_each_marker {
            ($m:ident) => { $m!($($marker),*) };
        }
        pub(crate) use for_each_marker;
    };
}

marker_literals!(
    ScriptP => "P",
    ScriptTextP => "TextP",
    ScriptGType => "GType",
    ScriptUuidTokens => "UUID",
    ScriptSetTokens => "Set",
    ScriptColumn => "Column",
    ScriptV => "V",
    ScriptBy => "By",
    ScriptCastPolicy => "CastPolicy",
    ScriptProfileType => "ProfileType",
    ScriptOrder => "Order",
    ScriptPick => "Pick",
    ScriptT => "T",
    ScriptScope => "Scope",
    ScriptCardinality => "Cardinality",
    ScriptDirection => "Direction",
    ScriptMerge => "Merge",
    ScriptOperator => "Operator",
    ScriptBarrier => "Barrier",
    ScriptWithOptions => "WithOptions",
    ScriptPop => "Pop",
    ScriptSchemaMode => "SchemaMode",
);

/// `Set.of(1, "a")`: the seed literal with its members.
impl crate::value::literal::TokenLiteral for ScriptSet {
    fn token_literal(&self) -> String {
        format!("Set.of({})", crate::value::literal::rhai_literals(&self.0))
    }
}
