// Schema diff; its public documentation (the compatibility rules) is the module docs of `crate::schema`.

use crate::graph::schema::nested::{compare_numbers, json_equal, number_text};
use crate::graph::schema::{ElementKind, GraphSchema, PropertySchema, SchemaMode};
use crate::value::temporal::Temporal;
use crate::value::{ElementProperty, Number, PropertyMap};
use std::cmp::Ordering;

/// What kind of change a [`SchemaChange`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaChangeKind {
    /// `mode` changed.
    ModeChanged,
    /// `meta` changed.
    MetaChanged,
    /// A label was declared.
    LabelAdded,
    /// A label declaration was removed.
    LabelRemoved,
    /// An edge connection was added.
    ConnectionAdded,
    /// An edge connection was removed.
    ConnectionRemoved,
    /// An object key was declared.
    PropertyAdded,
    /// An object key declaration was removed.
    PropertyRemoved,
    /// A key became required.
    RequiredAdded,
    /// A key is no longer required.
    RequiredRemoved,
    /// `type`, `format` or `anyOf` changed.
    TypeChanged,
    /// An assertion keyword (`minimum`, `pattern`, `enum`, ...) changed.
    ConstraintChanged,
    /// `additionalProperties` changed.
    AdditionalPropertiesChanged,
    /// An annotation (`title`, `description`, `default`, ...) changed.
    AnnotationChanged,
}

impl SchemaChangeKind {
    /// The snake_case name of the data shape (`label_added`, `type_changed`, ...).
    pub fn as_str(&self) -> &'static str {
        match self {
            SchemaChangeKind::ModeChanged => "mode_changed",
            SchemaChangeKind::MetaChanged => "meta_changed",
            SchemaChangeKind::LabelAdded => "label_added",
            SchemaChangeKind::LabelRemoved => "label_removed",
            SchemaChangeKind::ConnectionAdded => "connection_added",
            SchemaChangeKind::ConnectionRemoved => "connection_removed",
            SchemaChangeKind::PropertyAdded => "property_added",
            SchemaChangeKind::PropertyRemoved => "property_removed",
            SchemaChangeKind::RequiredAdded => "required_added",
            SchemaChangeKind::RequiredRemoved => "required_removed",
            SchemaChangeKind::TypeChanged => "type_changed",
            SchemaChangeKind::ConstraintChanged => "constraint_changed",
            SchemaChangeKind::AdditionalPropertiesChanged => "additional_properties_changed",
            SchemaChangeKind::AnnotationChanged => "annotation_changed",
        }
    }
}

/// One difference between two schemas.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SchemaChange {
    /// What changed.
    pub kind: SchemaChangeKind,
    /// The element kind of the label it concerns (`None` for `mode`/`meta`).
    pub element: Option<ElementKind>,
    /// The label it concerns.
    pub label: Option<String>,
    /// The location inside the label's schema: `$` for the label's property map, `$.address.zip`
    /// for a nested key, `$.tags[*]` for array items; a connection `(:a)-[:e]->(:b)`.
    pub path: Option<String>,
    /// The old value, short (a type name, a bound, ...).
    pub before: Option<String>,
    /// The new value, short.
    pub after: Option<String>,
    /// Whether the change is backward compatible (see the [module docs](crate::schema)).
    pub compatible: bool,
}

/// The differences between two schemas, with their compatibility.
///
/// Compatible = data and clients written against the old schema keep working (the rules are in
/// the [module docs](crate::schema)): e.g. removing a property declaration is compatible from an object
/// that admits undeclared keys (open) and incompatible from a closed one; it is listed as
/// [`SchemaChangeKind::PropertyRemoved`] either way.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct SchemaDiff {
    /// Every change, envelope first, then the labels in the new schema's order.
    pub changes: Vec<SchemaChange>,
}

impl SchemaDiff {
    /// The changes from `before` (`None`: no stored schema) to `after`.
    pub fn between(before: Option<&GraphSchema>, after: &GraphSchema) -> SchemaDiff {
        let empty = GraphSchema::new();
        let before = before.unwrap_or(&empty);
        let mut differ = Differ {
            old_mode: before.mode,
            new_mode: after.mode,
            changes: Vec::new(),
            element: None,
            label: None,
        };
        differ.envelope(before, after);
        if after.mode == SchemaMode::None {
            for change in &mut differ.changes {
                change.compatible = true;
            }
        }
        SchemaDiff {
            changes: differ.changes,
        }
    }

    /// Whether every change is compatible (also when there is none).
    pub fn compatible(&self) -> bool {
        self.changes.iter().all(|change| change.compatible)
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The changes as data: a list of `{kind, element, label, path, before, after,
    /// compatible}` (absent parts are `null`).
    pub fn changes_value(&self) -> ElementProperty {
        let text = |s: &Option<String>| {
            s.as_deref()
                .map_or(ElementProperty::Null, ElementProperty::from)
        };
        ElementProperty::Array(
            self.changes
                .iter()
                .map(|change| {
                    let mut map = PropertyMap::default();
                    map.insert("kind", ElementProperty::from(change.kind.as_str()));
                    map.insert(
                        "element",
                        change
                            .element
                            .map_or(ElementProperty::Null, |e| ElementProperty::from(e.as_str())),
                    );
                    map.insert("label", text(&change.label));
                    map.insert("path", text(&change.path));
                    map.insert("before", text(&change.before));
                    map.insert("after", text(&change.after));
                    map.insert("compatible", ElementProperty::Boolean(change.compatible));
                    ElementProperty::Object(Box::new(map))
                })
                .collect(),
        )
    }

    /// The diff as data: `{changes: [..], compatible}`.
    pub fn to_value(&self) -> ElementProperty {
        let mut map = PropertyMap::default();
        map.insert("changes", self.changes_value());
        map.insert("compatible", ElementProperty::Boolean(self.compatible()));
        ElementProperty::Object(Box::new(map))
    }
}

/// [`validate_schema`](crate::GraphTraversalSource::validate_schema) /
/// [`validate_schema_patch`](crate::GraphTraversalSource::validate_schema_patch): the
/// violations of the current data under a proposed schema, and the diff to it. Nothing is
/// applied.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SchemaValidation {
    /// The proposed schema (for a patch: the patched schema).
    pub schema: GraphSchema,
    /// Everything the current data violates (no groups: nothing).
    pub violations: crate::graph::schema::SchemaViolationReport,
    /// The changes from the stored schema to the proposed one.
    pub diff: SchemaDiff,
}

impl SchemaValidation {
    /// Whether the diff is backward compatible.
    pub fn compatible(&self) -> bool {
        self.diff.compatible()
    }

    /// Whether applying the schema would succeed now (no violations).
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }

    /// The result as data: `{violations: {..report..}, diff: [..changes..], compatible}`.
    pub fn to_value(&self) -> ElementProperty {
        let mut map = PropertyMap::default();
        map.insert("violations", self.violations.to_value());
        map.insert("diff", self.diff.changes_value());
        map.insert("compatible", ElementProperty::Boolean(self.compatible()));
        ElementProperty::Object(Box::new(map))
    }
}

fn strictness(mode: SchemaMode) -> u8 {
    match mode {
        SchemaMode::None => 0,
        SchemaMode::Open => 1,
        SchemaMode::Closed => 2,
    }
}

struct Differ {
    old_mode: SchemaMode,
    new_mode: SchemaMode,
    changes: Vec<SchemaChange>,
    element: Option<ElementKind>,
    label: Option<String>,
}

/// Whether a node admits every value (no type, no `anyOf`, no constraints, no keys).
fn accepts_any(node: &PropertySchema) -> bool {
    node.types.is_none()
        && node.any_of.is_none()
        && node.constraints.as_deref().is_none_or(|c| c.is_empty())
        && node.properties.is_empty()
        && node.required.is_empty()
        && node.items.is_none()
}

impl Differ {
    fn push(
        &mut self,
        kind: SchemaChangeKind,
        path: Option<String>,
        before: Option<String>,
        after: Option<String>,
        compatible: bool,
    ) {
        self.changes.push(SchemaChange {
            kind,
            element: self.element,
            label: self.label.clone(),
            path,
            before,
            after,
            compatible,
        });
    }

    fn envelope(&mut self, before: &GraphSchema, after: &GraphSchema) {
        if before.mode != after.mode {
            self.push(
                SchemaChangeKind::ModeChanged,
                None,
                Some(before.mode.as_str().to_string()),
                Some(after.mode.as_str().to_string()),
                strictness(after.mode) <= strictness(before.mode),
            );
        }
        if before.meta != after.meta {
            self.push(SchemaChangeKind::MetaChanged, None, None, None, true);
        }
        let new_closed = self.new_mode == SchemaMode::Closed;
        let old_closed = self.old_mode == SchemaMode::Closed;
        // Vertex labels.
        self.element = Some(ElementKind::Vertex);
        for (label, vertex) in &after.vertices {
            self.label = Some(label.clone());
            match before.vertices.get(label) {
                Some(old) => self.object_root(&old.schema, &vertex.schema),
                None => {
                    let ok = old_closed || self.root_is_optional(&vertex.schema);
                    self.push(SchemaChangeKind::LabelAdded, None, None, None, ok);
                }
            }
        }
        for label in before.vertices.keys() {
            if !after.vertices.contains_key(label) {
                self.label = Some(label.clone());
                self.push(SchemaChangeKind::LabelRemoved, None, None, None, false);
            }
        }
        // Edge labels.
        self.element = Some(ElementKind::Edge);
        for (label, edge) in &after.edges {
            self.label = Some(label.clone());
            match before.edges.get(label) {
                Some(old) => {
                    let old_all = old.connections.is_empty();
                    let new_all = edge.connections.is_empty();
                    let covers = new_all
                        || (!old_all
                            && old.connections.iter().all(|c| edge.connections.contains(c)));
                    let ok = covers || !new_closed;
                    let render = |c: &crate::graph::schema::EdgeConnection| {
                        format!("(:{})-[:{label}]->(:{})", c.from, c.to)
                    };
                    for c in &edge.connections {
                        if !old.connections.contains(c) {
                            self.push(
                                SchemaChangeKind::ConnectionAdded,
                                Some(render(c)),
                                None,
                                None,
                                ok,
                            );
                        }
                    }
                    for c in &old.connections {
                        if !edge.connections.contains(c) {
                            self.push(
                                SchemaChangeKind::ConnectionRemoved,
                                Some(render(c)),
                                None,
                                None,
                                ok,
                            );
                        }
                    }
                    self.object_root(&old.schema, &edge.schema);
                }
                None => {
                    let ok = old_closed || self.root_is_optional(&edge.schema);
                    self.push(SchemaChangeKind::LabelAdded, None, None, None, ok);
                }
            }
        }
        for label in before.edges.keys() {
            if !after.edges.contains_key(label) {
                self.label = Some(label.clone());
                self.push(SchemaChangeKind::LabelRemoved, None, None, None, false);
            }
        }
        self.element = None;
        self.label = None;
    }

    /// Whether a new label's schema lets every write of an old client through: it requires no
    /// key and admits undeclared keys (its declared keys are optional additions).
    fn root_is_optional(&self, root: &PropertySchema) -> bool {
        root.required.is_empty()
            && root.admits_additional_properties(self.new_mode.default_additional_properties())
    }

    fn object_root(&mut self, old: &PropertySchema, new: &PropertySchema) {
        self.node("$".to_string(), old, new);
    }

    fn node(&mut self, path: String, old: &PropertySchema, new: &PropertySchema) {
        if old == new {
            return;
        }
        let same_type = old.types == new.types && old.format == new.format;
        let same_any_of = old.any_of == new.any_of;
        if !same_type || !same_any_of {
            match (&old.any_of, &new.any_of) {
                (Some(a), Some(b)) if a.len() == b.len() && same_type => {
                    for (index, (a, b)) in a.iter().zip(b).enumerate() {
                        self.node(format!("{path}.anyOf[{index}]"), a, b);
                    }
                }
                _ => {
                    let ok = match (old.declared_kinds(), new.declared_kinds()) {
                        (_, None) => true,
                        (None, Some(_)) => false,
                        (Some(old_kinds), Some(new_kinds)) => {
                            old_kinds.iter().all(|kind| new_kinds.contains(kind))
                        }
                    };
                    self.push(
                        SchemaChangeKind::TypeChanged,
                        Some(path.clone()),
                        Some(old.type_name()),
                        Some(new.type_name()),
                        ok,
                    );
                }
            }
        }
        self.constraints(&path, old, new);
        if old.annotations != new.annotations {
            self.push(
                SchemaChangeKind::AnnotationChanged,
                Some(path.clone()),
                None,
                None,
                true,
            );
        }
        // Array items.
        match (&old.items, &new.items) {
            (Some(a), Some(b)) => self.node(format!("{path}[*]"), a, b),
            (None, Some(b)) => {
                let ok = accepts_any(b);
                self.push(
                    SchemaChangeKind::TypeChanged,
                    Some(format!("{path}[*]")),
                    Some("any".to_string()),
                    Some(b.type_name()),
                    ok,
                );
            }
            (Some(a), None) => self.push(
                SchemaChangeKind::TypeChanged,
                Some(format!("{path}[*]")),
                Some(a.type_name()),
                Some("any".to_string()),
                true,
            ),
            (None, None) => {}
        }
        // Object keys.
        let old_additional =
            old.admits_additional_properties(self.old_mode.default_additional_properties());
        let new_additional =
            new.admits_additional_properties(self.new_mode.default_additional_properties());
        let child = |key: &str| {
            if path == "$" {
                format!("$.{key}")
            } else {
                format!("{path}.{key}")
            }
        };
        for (key, node) in &new.properties {
            match old.properties.get(key) {
                Some(old_node) => self.node(child(key), old_node, node),
                None => {
                    // Optional: compatible, also in an open object (data that already holds the
                    // key with another type is validate_schema's business). Required: the
                    // RequiredAdded change below is incompatible, and so is this one. A key the
                    // old schema required without declaring it was written by every old client,
                    // with any value: a declaration may only keep accepting everything.
                    let ok = if old.is_required(key) {
                        accepts_any(node)
                    } else {
                        !new.is_required(key)
                    };
                    self.push(
                        SchemaChangeKind::PropertyAdded,
                        Some(child(key)),
                        None,
                        Some(node.type_name()),
                        ok,
                    );
                }
            }
        }
        for (key, node) in &old.properties {
            if !new.properties.contains_key(key) {
                // Compatible when the new object admits undeclared keys: old data stays valid and
                // old writers are still accepted (the key is simply no longer declared). From a
                // closed object the key would be rejected: incompatible.
                self.push(
                    SchemaChangeKind::PropertyRemoved,
                    Some(child(key)),
                    Some(node.type_name()),
                    None,
                    new_additional,
                );
            }
        }
        for key in &new.required {
            if !old.is_required(key) {
                self.push(
                    SchemaChangeKind::RequiredAdded,
                    Some(child(key)),
                    None,
                    None,
                    false,
                );
            }
        }
        for key in &old.required {
            if !new.is_required(key) {
                self.push(
                    SchemaChangeKind::RequiredRemoved,
                    Some(child(key)),
                    None,
                    None,
                    true,
                );
            }
        }
        if old.additional_properties != new.additional_properties {
            self.push(
                SchemaChangeKind::AdditionalPropertiesChanged,
                Some(path.clone()),
                Some(old_additional.to_string()),
                Some(new_additional.to_string()),
                !old_additional || new_additional,
            );
        }
    }

    fn constraints(&mut self, path: &str, old: &PropertySchema, new: &PropertySchema) {
        let empty = crate::graph::schema::Constraints::default();
        let a = old.constraints.as_deref().unwrap_or(&empty);
        let b = new.constraints.as_deref().unwrap_or(&empty);
        if a == b {
            return;
        }
        // Lower bounds: relaxed when the new one is absent or not greater.
        let number_bounds: [(&str, &Option<Number>, &Option<Number>, Ordering); 4] = [
            ("minimum", &a.minimum, &b.minimum, Ordering::Less),
            (
                "exclusiveMinimum",
                &a.exclusive_minimum,
                &b.exclusive_minimum,
                Ordering::Less,
            ),
            ("maximum", &a.maximum, &b.maximum, Ordering::Greater),
            (
                "exclusiveMaximum",
                &a.exclusive_maximum,
                &b.exclusive_maximum,
                Ordering::Greater,
            ),
        ];
        for (name, x, y, relaxed) in number_bounds {
            if x == y {
                continue;
            }
            let ok = match (x, y) {
                (_, None) => true,
                (None, Some(_)) => false,
                (Some(x), Some(y)) => {
                    compare_numbers(y, x).is_some_and(|o| o == relaxed || o == Ordering::Equal)
                }
            };
            self.bound_change(
                path,
                name,
                x.as_ref().map(number_text),
                y.as_ref().map(number_text),
                ok,
            );
        }
        let temporal_bounds: [(&str, &Option<Temporal>, &Option<Temporal>, Ordering); 4] = [
            (
                "formatMinimum",
                &a.format_minimum,
                &b.format_minimum,
                Ordering::Less,
            ),
            (
                "formatExclusiveMinimum",
                &a.format_exclusive_minimum,
                &b.format_exclusive_minimum,
                Ordering::Less,
            ),
            (
                "formatMaximum",
                &a.format_maximum,
                &b.format_maximum,
                Ordering::Greater,
            ),
            (
                "formatExclusiveMaximum",
                &a.format_exclusive_maximum,
                &b.format_exclusive_maximum,
                Ordering::Greater,
            ),
        ];
        for (name, x, y, relaxed) in temporal_bounds {
            if x == y {
                continue;
            }
            // Relaxed: removed, or moved outward within the same type (a bound of another type
            // comes with a format change, itself incompatible).
            let ok = match (x, y) {
                (_, None) => true,
                (None, Some(_)) => false,
                (Some(x), Some(y)) => {
                    x.kind() == y.kind()
                        && matches!(y.cmp(x), o if o == relaxed || o == Ordering::Equal)
                }
            };
            self.bound_change(
                path,
                name,
                x.as_ref().map(ToString::to_string),
                y.as_ref().map(ToString::to_string),
                ok,
            );
        }
        let count_bounds: [(&str, Option<u64>, Option<u64>, bool); 4] = [
            ("minLength", a.min_length, b.min_length, true),
            ("maxLength", a.max_length, b.max_length, false),
            ("minItems", a.min_items, b.min_items, true),
            ("maxItems", a.max_items, b.max_items, false),
        ];
        for (name, x, y, lower) in count_bounds {
            if x == y {
                continue;
            }
            let ok = match (x, y) {
                (_, None) => true,
                (None, Some(_)) => false,
                (Some(x), Some(y)) => {
                    if lower {
                        y <= x
                    } else {
                        y >= x
                    }
                }
            };
            self.bound_change(
                path,
                name,
                x.map(|n| n.to_string()),
                y.map(|n| n.to_string()),
                ok,
            );
        }
        if a.pattern != b.pattern {
            self.bound_change(
                path,
                "pattern",
                a.pattern.as_ref().map(|p| p.as_str().to_string()),
                b.pattern.as_ref().map(|p| p.as_str().to_string()),
                b.pattern.is_none(),
            );
        }
        if a.enum_values != b.enum_values {
            let ok = match (&a.enum_values, &b.enum_values) {
                (_, None) => true,
                (None, Some(_)) => false,
                (Some(x), Some(y)) => x.iter().all(|v| y.iter().any(|w| json_equal(v, w))),
            };
            let show = |values: &Option<Vec<ElementProperty>>| {
                values.as_ref().map(|values| {
                    crate::traversal::step::barrier_steps::numeric::preview(
                        &ElementProperty::Array(values.clone()),
                    )
                })
            };
            self.bound_change(path, "enum", show(&a.enum_values), show(&b.enum_values), ok);
        }
        if a.const_value != b.const_value {
            let show = |value: &Option<ElementProperty>| {
                value
                    .as_ref()
                    .map(crate::traversal::step::barrier_steps::numeric::preview)
            };
            let ok = b.const_value.is_none()
                || matches!((&a.const_value, &b.const_value), (Some(x), Some(y)) if json_equal(x, y));
            self.bound_change(
                path,
                "const",
                show(&a.const_value),
                show(&b.const_value),
                ok,
            );
        }
    }

    fn bound_change(
        &mut self,
        path: &str,
        keyword: &str,
        before: Option<String>,
        after: Option<String>,
        compatible: bool,
    ) {
        self.push(
            SchemaChangeKind::ConstraintChanged,
            Some(path.to_string()),
            Some(format!("{keyword} {}", before.as_deref().unwrap_or("-"))),
            Some(format!("{keyword} {}", after.as_deref().unwrap_or("-"))),
            compatible,
        );
    }
}
