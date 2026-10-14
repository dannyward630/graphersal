//! Create-time defaults: `default` and `defaultFunction` of a label's top-level properties (see
//! [`Annotations`](crate::graph::schema::Annotations) for the rules, the book page "Schema
//! Enforcement" for the guide).
//!
//! - [`fill_defaults`] adds the missing defaults to the properties of an element being created,
//!   before the write path coerces and validates them. Only the create paths call it
//!   ([`GraphStorage::create_vertex`](crate::graph::GraphStorage::create_vertex) /
//!   [`create_edge`](crate::graph::GraphStorage::create_edge)); imports, change sets, replay
//!   and loads store what they carry.
//! - [`check_defaults`] is the check of a schema being set: every applied default must be valid
//!   for its property, and `defaultFunction` may only sit on a top-level property.

use compact_str::CompactString;

use crate::error::SchemaError;
use crate::graph::GraphNodeType;
use crate::graph::schema::coerce::coerce_to_declared;
use crate::graph::schema::nested::{self, NestedCtx};
use crate::graph::schema::{GraphSchema, PropertySchema, SchemaMode};
use crate::value::PropertyMap;
use crate::value::json_path::JsonPathSegment;

/// Adds the create-time default of every top-level property the element's labels declare and
/// `properties` lacks (an explicit value, `null` included, is kept). The first label, in label
/// order, that declares a key decides (the coercion rule): its `default`/`defaultFunction`, or
/// nothing when it has none. Nothing in [`SchemaMode::None`] (nothing is enforced there).
///
/// `labels` is a vertex's label set, or an edge's single label (empty when it has none).
pub(crate) fn fill_defaults(
    schema: &GraphSchema,
    kind: GraphNodeType,
    labels: &[CompactString],
    properties: &mut PropertyMap,
) {
    if schema.mode == SchemaMode::None {
        return;
    }
    for (position, label) in labels.iter().enumerate() {
        let Some(root) = schema.label_root(kind, label.as_str()) else {
            continue;
        };
        for (key, node) in &root.properties {
            let Some(default) = node.create_default() else {
                continue;
            };
            if properties.contains_key(key) {
                continue;
            }
            let decided_before = labels[..position].iter().any(|earlier| {
                schema
                    .label_root(kind, earlier.as_str())
                    .is_some_and(|root| root.properties.contains_key(key))
            });
            if !decided_before {
                properties.insert(key.as_str(), default.value());
            }
        }
    }
}

/// The check of a schema being set: every top-level `default`/`defaultFunction` produces a value
/// valid for its property (after the write coercion: a canonical string default of a `uuid`
/// property is fine), and no `defaultFunction` sits anywhere else (a label's root, a nested
/// property, array items, an `anyOf` member), where nothing would apply it. A `default` below
/// the top level stays an annotation and is not checked. Fails with
/// [`SchemaError::InvalidSchema`] naming the keyword's location.
pub(crate) fn check_defaults(schema: &GraphSchema) -> Result<(), SchemaError> {
    let additional_default = schema.mode.default_additional_properties();
    let labels = schema
        .vertices
        .iter()
        .map(|(label, v)| ("vertex", "vertices", label, &v.schema))
        .chain(
            schema
                .edges
                .iter()
                .map(|(label, e)| ("edge", "edges", label, &e.schema)),
        );
    for (element_type, section, label, root) in labels {
        let root_at = format!("/{section}/{}/schema", escape(label));
        if let Some(at) = misplaced_function(root, &root_at, true) {
            return Err(misplaced(at));
        }
        for (key, node) in &root.properties {
            let at = format!("{root_at}/properties/{}", escape(key));
            for (name, child) in children(node) {
                if let Some(found) = misplaced_function(child, &format!("{at}/{name}"), false) {
                    return Err(misplaced(found));
                }
            }
            let Some(default) = node.create_default() else {
                continue;
            };
            let keyword = match default {
                crate::graph::schema::node::CreateDefault::Value(_) => "default",
                crate::graph::schema::node::CreateDefault::Function(_) => "defaultFunction",
            };
            let fail = |reason: String| SchemaError::InvalidSchema {
                path: format!("{at}/{keyword}"),
                reason: format!("the {keyword} does not fit the property: {reason}"),
                hint: "A default is stored when an element is created without the property, so \
                       it must be a valid value of the property's schema (defaultFunction uuidv4 \
                       and uuidv7 produce a uuid: declare {\"type\": \"string\", \"format\": \
                       \"uuid\"})."
                    .to_string(),
            };
            let value = coerce_to_declared(default.value(), node)
                .map_err(|failure| fail(failure.reason.to_string()))?;
            let ctx = NestedCtx {
                element_type,
                label,
                additional_default,
                full: true,
            };
            let mut path = vec![JsonPathSegment::Key(key.as_str().into())];
            nested::validate_nested(&ctx, &mut path, node, &value)
                .map_err(|err| fail(err.to_string()))?;
        }
    }
    Ok(())
}

/// The child nodes of `node` with the pointer segment(s) leading to them.
fn children(node: &PropertySchema) -> Vec<(String, &PropertySchema)> {
    let mut out = Vec::new();
    if let Some(items) = &node.items {
        out.push(("items".to_string(), items.as_ref()));
    }
    for (name, property) in &node.properties {
        out.push((format!("properties/{}", escape(name)), property));
    }
    if let Some(members) = &node.any_of {
        for (index, member) in members.iter().enumerate() {
            out.push((format!("anyOf/{index}"), member));
        }
    }
    out
}

/// The location of the first `defaultFunction` in `node` (itself, then its children, depth
/// first), as a JSON Pointer. For a label's `root`, its `properties` are the top level the
/// caller checks, so they are skipped.
fn misplaced_function(node: &PropertySchema, at: &str, root: bool) -> Option<String> {
    if node
        .annotations()
        .is_some_and(|a| a.default_function.is_some())
    {
        return Some(format!("{at}/defaultFunction"));
    }
    children(node)
        .into_iter()
        .filter(|(name, _)| !(root && name.starts_with("properties/")))
        .find_map(|(name, child)| misplaced_function(child, &format!("{at}/{name}"), false))
}

fn misplaced(at: String) -> SchemaError {
    SchemaError::InvalidSchema {
        path: at,
        reason: "defaultFunction applies only to a label's top-level properties".to_string(),
        hint: "Put it on a property directly under the label schema's \"properties\"; values \
               inside an object or array are not filled in."
            .to_string(),
    }
}

/// JSON Pointer escaping of one segment (`~` -> `~0`, `/` -> `~1`).
fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
