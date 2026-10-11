//! The `By` modulator token (`ArgBy`, `By.Label`/`By::Label`, ...).

use crate::script::*;

/// A script-level `by()` argument, before conversion to `By`.
#[derive(Debug, Clone)] // , Eq, PartialEq, Hash
#[non_exhaustive]
pub enum ArgBy {
    /// Reads a named property.
    Property(CompactString),
    /// Runs a child traversal and reads its result.
    Traversal(ScriptGraphTraversalSource),
    /// Reads the element's label.
    Label,
    /// Reads the element's ID.
    Id,
    /// Reads a map entry's/single-property's value.
    Value,
    /// Reads a property's key (`T.key`).
    Key,
    /// Reads a group's/bucket's size.
    Count,
    /// Reads a value at a JSON Path.
    JsonPath(crate::value::JsonPath),
    /// `.by(Column.keys)` / `.by(Column.values)`: a map entry's key/value, or a map's key/value
    /// list, as an `order()` sort key.
    Column(Column),
    /// `.by(Order.asc)`/`.by(Order.desc)` — a direction-only modulator, used by `order()`
    /// (part 2 of the `order()` series). Not accepted by `group()`/`group_count()`.
    Order(Order),
    /// `.by(["directory", "file"])` — a list of vertex labels, for steps that accept a label
    /// filter at a specific `by()` position; every other step rejects it when it executes.
    Labels(Vec<CompactString>),
}

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod ArgByModule {
    use super::ArgBy;
    use crate::value::literal::TokenLiteral;

    // The `///` texts of this module's functions are their DSL docs (completion tooltips, the
    // MCP DSL reference): `script/register/docs_tests.rs` checks them against `docs.rs`.

    /// A by() argument that reads the named property (the same as by("name")).
    ///
    /// Example: `g.v().has_label("person").order().by(By.Property("age")).values("name").to_list()` returns `["vadas", "marko", "josh", "peter"]`
    pub fn Property(prop: String) -> ArgBy {
        ArgBy::Property(CompactString::new(prop))
    }
    /// A by() argument that runs a child traversal (the same as by(__.x())).
    ///
    /// Example: `g.v().has_label("person").order().by(By.Traversal(__.out_e().count())).values("name").to_list()` returns `["vadas", "peter", "josh", "marko"]`
    pub fn Traversal(prop: ScriptGraphTraversalSource) -> ArgBy {
        ArgBy::Traversal(prop)
    }
    pub const Label: ArgBy = ArgBy::Label;
    pub const Id: ArgBy = ArgBy::Id;
    pub const Value: ArgBy = ArgBy::Value;
    pub const Count: ArgBy = ArgBy::Count;

    // Parses `path` eagerly: a malformed path (recursive descent, wildcards, a bad index) is a
    // script runtime error at the call site.

    /// A by() argument that reads a nested value by a JSONPath.
    ///
    /// Example: `g.inject(#{a: #{b: 2}}, #{a: #{b: 1}}).order().by(By.JsonPath("a.b")).to_list()` returns `[#{"a": #{"b": 1}}, #{"a": #{"b": 2}}]`
    #[rhai_fn(return_raw)]
    pub fn JsonPath(path: String) -> Result<ArgBy, Box<rhai::EvalAltResult>> {
        path.parse::<crate::value::JsonPath>()
            .map(ArgBy::JsonPath)
            .map_err(|err| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    rhai::Dynamic::from(crate::error::format_diagnostic(&err)),
                    rhai::Position::NONE,
                ))
            })
    }

    /// Return the current variant of `ArgBy`.
    #[rhai_fn(global, get = "enum_type", pure)]
    pub fn get_type(arg_by: &mut ArgBy) -> String {
        match arg_by {
            ArgBy::Property(_) => "Property",
            ArgBy::Label => "Label",
            ArgBy::Id => "Id",
            ArgBy::Value => "Value",
            ArgBy::Key => "Key",
            ArgBy::Count => "Count",
            ArgBy::Traversal(_) => "Traversal",
            ArgBy::JsonPath(_) => "JsonPath",
            ArgBy::Order(_) => "Order",
            ArgBy::Labels(_) => "Labels",
            ArgBy::Column(_) => "Column",
        }
        .to_string()
    }

    /// Return the inner value.
    #[rhai_fn(global, get = "value", pure)]
    pub fn get_value(arg_by: &mut ArgBy) -> Dynamic {
        match arg_by {
            ArgBy::Id => Dynamic::UNIT,
            ArgBy::Label => Dynamic::UNIT,
            ArgBy::Value => Dynamic::UNIT,
            ArgBy::Key => Dynamic::UNIT,
            ArgBy::Count => Dynamic::UNIT,
            ArgBy::Property(p) => Dynamic::from(p.to_string()),
            ArgBy::Traversal(p) => Dynamic::from(p.clone()),
            ArgBy::JsonPath(p) => Dynamic::from(p.to_string()),
            ArgBy::Order(o) => Dynamic::from(o.clone()),
            ArgBy::Column(column) => Dynamic::from(*column),
            ArgBy::Labels(labels) => Dynamic::from_array(
                labels
                    .iter()
                    .map(|label| Dynamic::from(label.to_string()))
                    .collect(),
            ),
        }
    }

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(arg_by: &mut ArgBy) -> String {
        arg_by.token_literal()
    }
}

/// Scope marker that makes the vanilla dotted spelling `By.Label` resolve, mirroring
/// [`ScriptGType`]'s treatment: the no-argument variants are registered as properties of this
/// marker (`register_get`), and the argument-taking constructors (`Property`/`Traversal`/
/// `JsonPath`) as dot-callable methods on it, so both `By.Label`/`By.Property("x")` (Java Gremlin
/// dot style) and `By::Label`/`By::Property("x")` (this crate's Rust-style static module,
/// `ArgByModule`, registered separately) work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptBy;

/// The DSL literal of a `by()` argument value: `T.id`, `T.label`, `T.key`, `T.value`,
/// `By.Count`, `By.Property("age")`, `By.Traversal(__.out())`, `By.JsonPath("$.a.b")`,
/// `Column.keys`, `Order.desc`, `["dir", "file"]`.
impl crate::value::literal::TokenLiteral for ArgBy {
    fn token_literal(&self) -> String {
        use crate::value::literal::{rhai_string, rhai_string_array};
        match self {
            ArgBy::Property(key) => format!("By.Property({})", rhai_string(key)),
            ArgBy::Traversal(traversal) => {
                format!("By.Traversal({})", traversal.token_literal())
            }
            ArgBy::Label => "T.label".to_string(),
            ArgBy::Id => "T.id".to_string(),
            ArgBy::Value => "T.value".to_string(),
            ArgBy::Key => "T.key".to_string(),
            ArgBy::Count => "By.Count".to_string(),
            ArgBy::JsonPath(path) => format!("By.JsonPath({})", rhai_string(&path.to_string())),
            ArgBy::Column(column) => column.token_literal(),
            ArgBy::Order(order) => order.token_literal(),
            ArgBy::Labels(labels) => rhai_string_array(labels),
        }
    }
}
