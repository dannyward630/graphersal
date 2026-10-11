//! JSON Merge Patch (RFC 7396) of a schema: `patch_schema` and `validate_schema_patch`.

use crate::error::SchemaError;
use crate::graph::schema::GraphSchema;
use crate::graph::schema::parse::MAX_SCHEMA_DEPTH;
use crate::value::physical::{lift_untyped, read_json};
use crate::value::{ElementProperty, PropertyMap};

/// A JSON Merge Patch document (RFC 7396) for a schema.
///
/// Applied to the canonical form of the stored schema ([`GraphSchema::to_value`], `{}` when no
/// schema is stored): an object member of the patch merges into the same member of the schema,
/// `null` removes it, and any other value replaces it (an array is replaced as a whole). The
/// result must be a valid schema.
///
/// ```
/// use graphersal::schema::{GraphSchema, SchemaMode, SchemaPatch};
/// let schema = GraphSchema::from_json(r#"{"mode": "open", "vertices": {"person": {}}}"#).unwrap();
/// let patch = SchemaPatch::from_json(r#"{"mode": "closed", "vertices": {"person": null}}"#).unwrap();
/// let patched = patch.apply(Some(&schema)).unwrap();
/// assert_eq!(patched.mode, SchemaMode::Closed);
/// assert!(patched.vertices.is_empty());
/// ```
///
/// RFC 7396 cannot set a member *to* `null` (null means "remove"): a `"default": null` or a
/// `null` in `meta` is written with [`GraphSchema`] replacement (`set_schema`) instead.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaPatch {
    document: ElementProperty,
}

impl SchemaPatch {
    /// The patch from its JSON text.
    pub fn from_json(text: &str) -> Result<SchemaPatch, SchemaError> {
        let value =
            read_json(text, MAX_SCHEMA_DEPTH).map_err(|err| SchemaError::InvalidSchema {
                path: "/".to_string(),
                reason: format!("the patch is not valid JSON: {err}"),
                hint: String::new(),
            })?;
        Ok(SchemaPatch {
            document: lift_untyped(value),
        })
    }

    /// The patch from a value (a Rhai map or a Python dict).
    pub fn from_value(value: ElementProperty) -> SchemaPatch {
        SchemaPatch { document: value }
    }

    /// The patch document.
    pub fn document(&self) -> &ElementProperty {
        &self.document
    }

    /// Applies the patch to `schema` (`None`: no stored schema, the patch applies to `{}`) and
    /// loads the result.
    pub fn apply(&self, schema: Option<&GraphSchema>) -> Result<GraphSchema, SchemaError> {
        let target = match schema {
            Some(schema) => schema.to_value(),
            None => ElementProperty::Object(Box::default()),
        };
        GraphSchema::from_value(&merge_patch(target, &self.document))
    }
}

/// RFC 7396 `MergePatch(Target, Patch)`.
pub(crate) fn merge_patch(target: ElementProperty, patch: &ElementProperty) -> ElementProperty {
    let ElementProperty::Object(patch_members) = patch else {
        return patch.clone();
    };
    let mut target_members: PropertyMap = match target {
        ElementProperty::Object(members) => *members,
        _ => PropertyMap::default(),
    };
    for (key, value) in patch_members.iter() {
        if matches!(value, ElementProperty::Null) {
            target_members.shift_remove(key);
        } else if let Some(slot) = target_members.get_mut(key) {
            // In place: a patched member keeps its position (property order is meaningful).
            let current = std::mem::replace(slot, ElementProperty::Null);
            *slot = merge_patch(current, value);
        } else {
            target_members.insert(key.clone(), merge_patch(ElementProperty::Null, value));
        }
    }
    ElementProperty::Object(Box::new(target_members))
}
