//! One node of the supported JSON Schema 2020-12 subset: [`PropertySchema`] and its parts.
//!
//! A label's `schema` and every property inside it are `PropertySchema`s. The model stays
//! close to the JSON form (one field per keyword), so a schema round-trips through
//! [`GraphSchema::to_value`](crate::graph::schema::GraphSchema::to_value) unchanged, apart from
//! `$ref`/`$defs`, which are inlined when the schema is loaded.

use indexmap::IndexMap;

use crate::graph::schema::SchemaPattern;
use crate::value::{ElementProperty, Number};

/// A JSON Schema `type` name of the supported subset.
///
/// The engine's logical types map one to one, except `string`, which with
/// [`SchemaFormat::Uuid`] stands for `uuid`: `integer` = `int64` (a value outside `i64` cannot
/// be stored at all), `number` = `float64` (an `int64` written to it is coerced when exactly
/// representable), `string` = `string`, `boolean`, `null`, `array`, `object`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum JsonType {
    /// `null`.
    Null,
    /// `boolean`.
    Boolean,
    /// `integer`: an `int64`.
    Integer,
    /// `number`: a `float64`.
    Number,
    /// `string` (a `uuid` with `format: "uuid"`).
    String,
    /// `array`.
    Array,
    /// `object`.
    Object,
}

impl JsonType {
    /// Every type, in the order diagnostics list them.
    pub const ALL: [JsonType; 7] = [
        JsonType::Null,
        JsonType::Boolean,
        JsonType::Integer,
        JsonType::Number,
        JsonType::String,
        JsonType::Array,
        JsonType::Object,
    ];

    /// The JSON Schema spelling: `null`, `boolean`, `integer`, `number`, `string`, `array`,
    /// `object`.
    pub fn as_str(self) -> &'static str {
        match self {
            JsonType::Null => "null",
            JsonType::Boolean => "boolean",
            JsonType::Integer => "integer",
            JsonType::Number => "number",
            JsonType::String => "string",
            JsonType::Array => "array",
            JsonType::Object => "object",
        }
    }

    /// Parses a JSON Schema type name (case-sensitive, no aliases).
    pub fn from_name(name: &str) -> Option<Self> {
        JsonType::ALL.into_iter().find(|ty| ty.as_str() == name)
    }

    /// The engine's canonical name of this type (`int64`, `float64`, ...), as diagnostics,
    /// `Display` and `GType` spell it.
    pub(crate) fn engine_name(self, format: Option<SchemaFormat>) -> &'static str {
        match self {
            JsonType::Null => "null",
            JsonType::Boolean => "boolean",
            JsonType::Integer => "int64",
            JsonType::Number => "float64",
            JsonType::String => match format {
                Some(SchemaFormat::Uuid) => "uuid",
                None => "string",
            },
            JsonType::Array => "array",
            JsonType::Object => "object",
        }
    }
}

/// A `format` the engine understands. Only `uuid` exists in 0.1.0: every other `format` is an
/// error when a schema is loaded (a later date type must not silently change what an existing
/// schema accepts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaFormat {
    /// `format: "uuid"`: a `string` that is a UUID, stored as the logical `uuid` type.
    Uuid,
}

impl SchemaFormat {
    /// The keyword value: `uuid`.
    pub fn as_str(self) -> &'static str {
        match self {
            SchemaFormat::Uuid => "uuid",
        }
    }
}

/// The assertion keywords of a node besides `type`: each one restricts the values of the kind
/// it applies to and ignores the others (JSON Schema semantics): numbers for
/// `minimum`/`maximum`/`exclusive*`, strings for `minLength`/`maxLength`/`pattern`, arrays for
/// `minItems`/`maxItems`, every value for `enum`/`const`.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Constraints {
    /// `minimum`: inclusive lower bound of a number.
    pub minimum: Option<Number>,
    /// `maximum`: inclusive upper bound of a number.
    pub maximum: Option<Number>,
    /// `exclusiveMinimum`: exclusive lower bound of a number.
    pub exclusive_minimum: Option<Number>,
    /// `exclusiveMaximum`: exclusive upper bound of a number.
    pub exclusive_maximum: Option<Number>,
    /// `minLength`: minimum number of characters (code points) of a string.
    pub min_length: Option<u64>,
    /// `maxLength`: maximum number of characters (code points) of a string.
    pub max_length: Option<u64>,
    /// `pattern`: a regular expression a string must match (unanchored, see
    /// [`SchemaPattern`]).
    pub pattern: Option<SchemaPattern>,
    /// `minItems`: minimum number of items of an array.
    pub min_items: Option<u64>,
    /// `maxItems`: maximum number of items of an array.
    pub max_items: Option<u64>,
    /// `enum`: the value must equal one of these (JSON values; a `uuid` compares as its
    /// canonical string, `1` equals `1.0`).
    pub enum_values: Option<Vec<ElementProperty>>,
    /// `const`: the value must equal this one.
    pub const_value: Option<ElementProperty>,
}

impl Constraints {
    /// `true` when no keyword is set.
    pub fn is_empty(&self) -> bool {
        *self == Constraints::default()
    }
}

/// The annotation keywords of a node: stored, round-tripped, shown by the renderers, and
/// **never** used for validation.
///
/// # `default` is an annotation only
///
/// [`default`](Self::default) is information for clients and user interfaces (an editor may
/// prefill a form with it). The engine never applies it: it is not written when an element is
/// created without the property, not substituted when the property is read (`values()`,
/// `valueMap()`, display and export show nothing for an absent property), and changing it
/// changes no stored value. A required property without a value is still a violation, whatever
/// its `default`. This is JSON Schema's own meaning of `default`; it also keeps a pasted
/// Pydantic schema (which emits `default` on every optional field) from materialising values
/// nobody wrote. Should create-time defaults ever be supported, they get a keyword of their own.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Annotations {
    /// `title`.
    pub title: Option<String>,
    /// `description`.
    pub description: Option<String>,
    /// `$comment`.
    pub comment: Option<String>,
    /// `examples`.
    pub examples: Option<Vec<ElementProperty>>,
    /// `deprecated`.
    pub deprecated: Option<bool>,
    /// `default`: an annotation only, never applied (see the type docs).
    pub default: Option<ElementProperty>,
}

impl Annotations {
    /// `true` when no annotation is set.
    pub fn is_empty(&self) -> bool {
        *self == Annotations::default()
    }
}

/// One JSON Schema node of the supported subset: what a property (or a label's whole property
/// map) may hold.
///
/// Every keyword of the node must hold for a value to be valid, as in JSON Schema:
///
/// - [`types`](Self::types) (`type`): the value's kind must be one of them; `None` (no `type`
///   keyword) admits every kind. `["integer", "null"]` is a nullable integer.
/// - [`format`](Self::format): `uuid` turns `string` into the logical `uuid` type.
/// - [`items`](Self::items): every item of an array value; `None` admits any items.
/// - [`properties`](Self::properties), [`required`](Self::required),
///   [`additional_properties`](Self::additional_properties): the keys of an object value.
///   `required` is independent of nullability: a required key must be present (it may hold
///   `null` when its own schema admits `null`), an optional key may be absent.
///   `additional_properties` `None` takes the graph's default (`Open` mode: `true`, `Closed`:
///   `false`).
/// - [`any_of`](Self::any_of): at least one member must accept the value.
/// - [`constraints`](Self::constraints) and [`annotations`](Self::annotations).
///
/// Build one with the constructors ([`Self::integer`], [`Self::object`], ...) and the `with_*`
/// methods, or load it as part of a [`GraphSchema`](crate::graph::schema::GraphSchema).
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct PropertySchema {
    /// `type`: the admitted kinds; `None` admits every kind.
    pub types: Option<Vec<JsonType>>,
    /// `format` (only `uuid`).
    pub format: Option<SchemaFormat>,
    /// `items`: the schema of every array item.
    pub items: Option<Box<PropertySchema>>,
    /// `properties`: the declared keys of an object.
    pub properties: IndexMap<String, PropertySchema>,
    /// `required`: the keys an object must have.
    pub required: Vec<String>,
    /// `additionalProperties` (`true`/`false`); `None` = the mode's default.
    pub additional_properties: Option<bool>,
    /// `anyOf`: the value must be valid against at least one member.
    pub any_of: Option<Vec<PropertySchema>>,
    /// The assertion keywords besides `type`.
    pub constraints: Option<Box<Constraints>>,
    /// The annotation keywords (`title`, `description`, `default`, ...).
    pub annotations: Option<Box<Annotations>>,
}

impl PropertySchema {
    /// `{}`: admits every value.
    pub fn any() -> Self {
        Self::default()
    }

    /// `{"type": <ty>}`.
    pub fn of(ty: JsonType) -> Self {
        Self {
            types: Some(vec![ty]),
            ..Self::default()
        }
    }

    /// `{"type": [..]}`: a type array (`["integer", "null"]`).
    pub fn of_types(types: &[JsonType]) -> Self {
        Self {
            types: Some(types.to_vec()),
            ..Self::default()
        }
    }

    /// `{"type": "null"}`.
    pub fn null() -> Self {
        Self::of(JsonType::Null)
    }

    /// `{"type": "boolean"}`.
    pub fn boolean() -> Self {
        Self::of(JsonType::Boolean)
    }

    /// `{"type": "integer"}`: an `int64`.
    pub fn integer() -> Self {
        Self::of(JsonType::Integer)
    }

    /// `{"type": "number"}`: a `float64`.
    pub fn number() -> Self {
        Self::of(JsonType::Number)
    }

    /// `{"type": "string"}`.
    pub fn string() -> Self {
        Self::of(JsonType::String)
    }

    /// `{"type": "string", "format": "uuid"}`: a `uuid`.
    pub fn uuid() -> Self {
        Self {
            format: Some(SchemaFormat::Uuid),
            ..Self::of(JsonType::String)
        }
    }

    /// `{"type": "array", "items": <items>}`.
    pub fn array(items: PropertySchema) -> Self {
        Self {
            items: Some(Box::new(items)),
            ..Self::of(JsonType::Array)
        }
    }

    /// `{"type": "object"}` without declared properties.
    pub fn object() -> Self {
        Self::of(JsonType::Object)
    }

    /// `{"anyOf": [..]}`.
    pub fn any_of(members: Vec<PropertySchema>) -> Self {
        Self {
            any_of: Some(members),
            ..Self::default()
        }
    }

    /// Admits `null` too: adds `null` to `type`, or a `{"type": "null"}` member to `anyOf`
    /// (a node with neither admits `null` already).
    pub fn nullable(mut self) -> Self {
        if let Some(types) = &mut self.types {
            if !types.contains(&JsonType::Null) {
                types.push(JsonType::Null);
            }
        } else if let Some(members) = &mut self.any_of
            && !members.iter().any(PropertySchema::admits_null)
        {
            members.push(PropertySchema::null());
        }
        self
    }

    /// Declares an optional object key.
    pub fn with_property(mut self, name: impl Into<String>, schema: PropertySchema) -> Self {
        self.properties.insert(name.into(), schema);
        self
    }

    /// Declares a required object key.
    pub fn with_required_property(
        mut self,
        name: impl Into<String>,
        schema: PropertySchema,
    ) -> Self {
        let name = name.into();
        if !self.required.contains(&name) {
            self.required.push(name.clone());
        }
        self.properties.insert(name, schema);
        self
    }

    /// Sets `additionalProperties`.
    pub fn with_additional_properties(mut self, allowed: bool) -> Self {
        self.additional_properties = Some(allowed);
        self
    }

    /// Sets the assertion keywords.
    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = (!constraints.is_empty()).then(|| Box::new(constraints));
        self
    }

    /// Changes the assertion keywords in place.
    pub fn with_constraint(mut self, change: impl FnOnce(&mut Constraints)) -> Self {
        let mut constraints = self.constraints.take().map(|c| *c).unwrap_or_default();
        change(&mut constraints);
        self.with_constraints(constraints)
    }

    /// Changes the annotations in place.
    pub fn with_annotation(mut self, change: impl FnOnce(&mut Annotations)) -> Self {
        let mut annotations = self.annotations.take().map(|a| *a).unwrap_or_default();
        change(&mut annotations);
        self.annotations = (!annotations.is_empty()).then(|| Box::new(annotations));
        self
    }

    /// Sets `description`.
    pub fn with_description(self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.with_annotation(|a| a.description = Some(description))
    }

    /// Whether the node admits a `null` value by its type (`type` contains `null`, no `type`,
    /// or an `anyOf` member does).
    pub fn admits_null(&self) -> bool {
        self.admits_kind(JsonType::Null)
    }

    /// Whether a value of `kind` can pass this node's type checks (through `anyOf` too).
    pub(crate) fn admits_kind(&self, kind: JsonType) -> bool {
        let by_type = self
            .types
            .as_ref()
            .is_none_or(|types| types.contains(&kind));
        let by_any_of = self
            .any_of
            .as_ref()
            .is_none_or(|members| members.iter().any(|m| m.admits_kind(kind)));
        by_type && by_any_of
    }

    /// The kinds the node declares (`(kind, is_uuid)`, a `uuid` string apart from a plain
    /// string), through `anyOf`; `None` when it declares no type restriction (no `type` and no
    /// `anyOf`, or an `anyOf` member without one).
    pub(crate) fn declared_kinds(&self) -> Option<Vec<(JsonType, bool)>> {
        if let Some(types) = &self.types {
            return Some(
                types
                    .iter()
                    .map(|t| {
                        (
                            *t,
                            *t == JsonType::String && self.format == Some(SchemaFormat::Uuid),
                        )
                    })
                    .collect(),
            );
        }
        let members = self.any_of.as_ref()?;
        let mut kinds = Vec::new();
        for member in members {
            kinds.extend(member.declared_kinds()?);
        }
        Some(kinds)
    }

    /// Whether an object value may hold keys this node does not declare: the node's own
    /// `additionalProperties`, else `mode_default` (the mode's default) for a node typed
    /// `object`, else `true` (a node without `type: object`, such as an `anyOf` wrapper, puts no
    /// constraint on the keys; its members do).
    pub fn admits_additional_properties(&self, mode_default: bool) -> bool {
        match self.additional_properties {
            Some(explicit) => explicit,
            None if self
                .types
                .as_ref()
                .is_some_and(|types| types.contains(&JsonType::Object)) =>
            {
                mode_default
            }
            None => true,
        }
    }

    /// Whether the node is `string` with `format: "uuid"` in its own `type` (the logical `uuid`).
    pub(crate) fn is_uuid_string(&self) -> bool {
        self.format == Some(SchemaFormat::Uuid)
            && self
                .types
                .as_ref()
                .is_some_and(|types| types.contains(&JsonType::String))
    }

    /// The node's type in the engine's canonical spelling, for diagnostics: `int64`,
    /// `array<string>`, `object`, `union<int64|null>`, `any` (no type restriction).
    pub fn type_name(&self) -> String {
        let own: Option<Vec<String>> = self.types.as_ref().map(|types| {
            types
                .iter()
                .map(|ty| match ty {
                    JsonType::Array => match &self.items {
                        Some(items) => format!("array<{}>", items.type_name()),
                        None => "array".to_string(),
                    },
                    other => other.engine_name(self.format).to_string(),
                })
                .collect()
        });
        let members: Option<Vec<String>> = self
            .any_of
            .as_ref()
            .map(|members| members.iter().map(PropertySchema::type_name).collect());
        let names = match (own, members) {
            (Some(own), _) => own,
            (None, Some(members)) => members,
            (None, None) => return "any".to_string(),
        };
        if names.len() == 1 {
            names.into_iter().next().unwrap_or_default()
        } else {
            format!("union<{}>", names.join("|"))
        }
    }

    /// [`Self::type_name`] without the `null` alternative (`int64` for `["integer", "null"]`):
    /// what diagnostics name as the expected type, since whether `null` is admitted is not what
    /// a mismatch of another value is about.
    pub fn non_null_type_name(&self) -> String {
        if let Some(types) = &self.types
            && types.len() > 1
            && types.contains(&JsonType::Null)
        {
            let mut copy = self.clone();
            copy.types = Some(
                types
                    .iter()
                    .copied()
                    .filter(|t| *t != JsonType::Null)
                    .collect(),
            );
            return copy.type_name();
        }
        if self.types.is_none()
            && let Some(members) = &self.any_of
        {
            let kept: Vec<&PropertySchema> = members
                .iter()
                .filter(|m| m.types.as_deref() != Some(&[JsonType::Null]))
                .collect();
            if kept.len() < members.len() && !kept.is_empty() {
                return match kept.as_slice() {
                    [only] => only.type_name(),
                    many => format!(
                        "union<{}>",
                        many.iter()
                            .map(|m| m.type_name())
                            .collect::<Vec<_>>()
                            .join("|")
                    ),
                };
            }
        }
        self.type_name()
    }

    /// The annotations, if any.
    pub fn annotations(&self) -> Option<&Annotations> {
        self.annotations.as_deref()
    }

    /// The constraints, if any.
    pub fn constraints(&self) -> Option<&Constraints> {
        self.constraints.as_deref()
    }

    /// Whether `key` is in `required`.
    pub fn is_required(&self, key: &str) -> bool {
        self.required.iter().any(|name| name == key)
    }
}
