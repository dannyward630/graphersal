//! `diff_schema`: the changes between two schemas and their backward compatibility (data and
//! clients written against the old schema keep working: a compatible change only adds or relaxes,
//! tightening a rule or removing a label or a closed object's key is incompatible, removing an open
//! object's key is compatible; conflicting existing data is
//! `validate_schema`'s business).
use graphersal::prelude::*;
use graphersal::schema::{SchemaChangeKind, SchemaDiff, VertexSchema};
use graphersal::storage::ElementKind;

use crate::schema_support::schema;

/// A one-label schema in `mode` whose `v` declares `properties` (JSON members) with `extra`
/// keywords on the label root.
fn label(mode: &str, properties: &str, extra: &str) -> GraphSchema {
    schema(&format!(
        r#"{{"mode": "{mode}", "vertices": {{"v": {{"schema": {{"type": "object",
            "properties": {{{properties}}}{extra}}}}}}}}}"#
    ))
}

fn diff(before: &GraphSchema, after: &GraphSchema) -> SchemaDiff {
    SchemaDiff::between(Some(before), after)
}

/// The single change between two schemas: its kind, path and compatibility.
fn single(before: &GraphSchema, after: &GraphSchema) -> (SchemaChangeKind, Option<String>, bool) {
    let diff = diff(before, after);
    assert_eq!(diff.changes.len(), 1, "{:#?}", diff.changes);
    let change = &diff.changes[0];
    assert_eq!(diff.compatible(), change.compatible);
    (change.kind, change.path.clone(), change.compatible)
}

#[test]
fn identical_schemas_have_no_changes() {
    let s = label("closed", r#""a": {"type": "integer"}"#, "");
    let diff = diff(&s, &s.clone());
    assert!(diff.is_empty() && diff.compatible());
    assert!(SchemaDiff::between(None, &GraphSchema::new()).is_empty());
}

#[test]
fn mode_changes_are_compatible_only_when_less_strict() {
    for (from, to, compatible) in [
        (SchemaMode::None, SchemaMode::Open, false),
        (SchemaMode::Open, SchemaMode::Closed, false),
        (SchemaMode::None, SchemaMode::Closed, false),
        (SchemaMode::Closed, SchemaMode::Open, true),
        (SchemaMode::Open, SchemaMode::None, true),
    ] {
        let (kind, _, ok) = single(
            &GraphSchema::new().with_mode(from),
            &GraphSchema::new().with_mode(to),
        );
        assert_eq!(kind, SchemaChangeKind::ModeChanged);
        assert_eq!(ok, compatible, "{from} -> {to}");
    }
}

#[test]
fn meta_and_annotations_are_always_compatible() {
    let base = label("closed", r#""a": {"type": "integer"}"#, "");
    let mut meta = base.clone();
    meta.meta = Some(ElementProperty::Object(Box::default()));
    assert_eq!(
        single(&base, &meta),
        (SchemaChangeKind::MetaChanged, None, true)
    );
    let annotated = label(
        "closed",
        r#""a": {"type": "integer", "description": "x", "deprecated": true}"#,
        "",
    );
    assert_eq!(
        single(&base, &annotated),
        (
            SchemaChangeKind::AnnotationChanged,
            Some("$.a".into()),
            true
        )
    );
    // A default is applied on create, so adding, changing or removing one is its own kind,
    // compatible too: no existing data or valid write is affected.
    let defaulted = label("closed", r#""a": {"type": "integer", "default": 3}"#, "");
    let redefaulted = label("closed", r#""a": {"type": "integer", "default": 4}"#, "");
    for (before, after) in [
        (&base, &defaulted),
        (&defaulted, &redefaulted),
        (&defaulted, &base),
    ] {
        assert_eq!(
            single(before, after),
            (SchemaChangeKind::DefaultChanged, Some("$.a".into()), true)
        );
    }
    let generated = label(
        "closed",
        r#""a": {"type": "string", "format": "uuid", "defaultFunction": "uuidv4"}"#,
        "",
    );
    let plain_uuid = label("closed", r#""a": {"type": "string", "format": "uuid"}"#, "");
    assert_eq!(
        single(&plain_uuid, &generated),
        (SchemaChangeKind::DefaultChanged, Some("$.a".into()), true)
    );
    // Both at once are two changes, still compatible.
    let both = label(
        "closed",
        r#""a": {"type": "integer", "description": "x", "default": 3, "deprecated": true}"#,
        "",
    );
    let diff = diff(&base, &both);
    let mut kinds: Vec<&str> = diff.changes.iter().map(|c| c.kind.as_str()).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, ["annotation_changed", "default_changed"]);
    assert!(diff.compatible());
}

#[test]
fn labels_added_and_removed() {
    let closed = schema(r#"{"mode": "closed", "vertices": {"a": {}}}"#);
    let closed_more = schema(r#"{"mode": "closed", "vertices": {"a": {}, "b": {}}}"#);
    // closed: no element could carry b before
    assert!(single(&closed, &closed_more).2);
    // ... removing it rejects its elements
    assert_eq!(
        single(&closed_more, &closed),
        (SchemaChangeKind::LabelRemoved, None, false)
    );
    // open: a new label with optional keys only is an addition
    let open = schema(r#"{"mode": "open", "vertices": {"a": {}}}"#);
    let open_more = schema(
        r#"{"mode": "open", "vertices": {"a": {}, "b": {"schema": {"type": "object",
            "properties": {"x": {"type": "integer"}}}}}}"#,
    );
    assert_eq!(
        single(&open, &open_more),
        (SchemaChangeKind::LabelAdded, None, true)
    );
    let open_free = schema(r#"{"mode": "open", "vertices": {"a": {}, "b": {}}}"#);
    assert!(single(&open, &open_free).2, "an empty label");
    // ... but one that requires a key, or rejects undeclared keys, breaks old writers of b
    let open_required = schema(
        r#"{"mode": "open", "vertices": {"a": {}, "b": {"schema": {"type": "object",
            "properties": {"x": {"type": "integer"}}, "required": ["x"]}}}}"#,
    );
    assert!(!single(&open, &open_required).2);
    let open_strict = schema(
        r#"{"mode": "open", "vertices": {"a": {}, "b": {"schema": {"type": "object",
            "additionalProperties": false}}}}"#,
    );
    assert!(!single(&open, &open_strict).2);
    // removing a label declaration is never compatible
    assert_eq!(
        single(&open_more, &open),
        (SchemaChangeKind::LabelRemoved, None, false)
    );
}

#[test]
fn edge_labels_follow_the_same_rules() {
    let open = schema(r#"{"mode": "open", "vertices": {"a": {}}}"#);
    let with_edge = schema(
        r#"{"mode": "open", "vertices": {"a": {}}, "edges": {"e": {"schema": {"type": "object",
            "properties": {"w": {"type": "number"}}}}}}"#,
    );
    let diff_added = diff(&open, &with_edge);
    assert_eq!(diff_added.changes.len(), 1, "{:#?}", diff_added.changes);
    assert_eq!(diff_added.changes[0].kind, SchemaChangeKind::LabelAdded);
    assert_eq!(diff_added.changes[0].element, Some(ElementKind::Edge));
    assert!(diff_added.compatible());
    assert!(!diff(&with_edge, &open).compatible());
}

#[test]
fn connections_change_compatibility_by_what_they_allow() {
    let edge = |mode: &str, connections: &str| {
        schema(&format!(
            r#"{{"mode": "{mode}", "vertices": {{"a": {{}}, "b": {{}}}},
                "edges": {{"e": {{"connections": [{connections}]}}}}}}"#
        ))
    };
    let ab = r#"{"from": "a", "to": "b"}"#;
    let ba = r#"{"from": "b", "to": "a"}"#;
    let both = format!("{ab}, {ba}");
    // An empty list allows every pair: the first connection restricts.
    let (kind, path, ok) = single(&edge("closed", ""), &edge("closed", ab));
    assert_eq!(kind, SchemaChangeKind::ConnectionAdded);
    assert_eq!(path.as_deref(), Some("(:a)-[:e]->(:b)"));
    assert!(!ok);
    assert!(diff(&edge("closed", ab), &edge("closed", &both)).compatible());
    assert!(!diff(&edge("closed", &both), &edge("closed", ab)).compatible());
    assert!(
        diff(&edge("closed", ab), &edge("closed", "")).compatible(),
        "all allowed again"
    );
    // Topology is checked in closed mode only.
    assert!(diff(&edge("open", &both), &edge("open", ab)).compatible());
}

#[test]
fn optional_properties_are_additions() {
    let closed = label("closed", r#""a": {"type": "integer"}"#, "");
    let closed_more = label(
        "closed",
        r#""a": {"type": "integer"}, "b": {"type": "string"}"#,
        "",
    );
    assert_eq!(
        single(&closed, &closed_more),
        (SchemaChangeKind::PropertyAdded, Some("$.b".into()), true)
    );
    assert_eq!(
        single(&closed_more, &closed),
        (SchemaChangeKind::PropertyRemoved, Some("$.b".into()), false)
    );
    let open = label("open", r#""a": {"type": "integer"}"#, "");
    let open_more = label(
        "open",
        r#""a": {"type": "integer"}, "b": {"type": "string"}"#,
        "",
    );
    // An optional key added to an open object is compatible (user decision, the usual
    // schema-registry convention); data that already holds b with another type is found by
    // validate_schema, not by the diff.
    assert_eq!(
        single(&open, &open_more),
        (SchemaChangeKind::PropertyAdded, Some("$.b".into()), true)
    );
    // Removing a declaration from an open object is compatible: old data and old writers stay
    // valid, the key is just undeclared now.
    assert_eq!(
        single(&open_more, &open),
        (SchemaChangeKind::PropertyRemoved, Some("$.b".into()), true)
    );
    let open_any = label("open", r#""a": {"type": "integer"}, "b": {}"#, "");
    assert!(single(&open, &open_any).2, "a key that admits anything");
    // nested open objects follow the same rule
    let nested = |inner: &str| {
        label(
            "open",
            &format!(r#""address": {{"type": "object", "properties": {{{inner}}}}}"#),
            "",
        )
    };
    let (kind, path, ok) = single(
        &nested(r#""zip": {"type": "string"}"#),
        &nested(r#""zip": {"type": "string"}, "city": {"type": "string"}"#),
    );
    assert_eq!(
        (kind, path.as_deref(), ok),
        (
            SchemaChangeKind::PropertyAdded,
            Some("$.address.city"),
            true
        )
    );
    // A required key added is incompatible: both changes say so.
    let open_required = label(
        "open",
        r#""a": {"type": "integer"}, "b": {"type": "string"}"#,
        r#", "required": ["b"]"#,
    );
    let required = diff(&open, &open_required);
    let kinds: Vec<(SchemaChangeKind, bool)> = required
        .changes
        .iter()
        .map(|c| (c.kind, c.compatible))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (SchemaChangeKind::PropertyAdded, false),
            (SchemaChangeKind::RequiredAdded, false)
        ]
    );
    // A key the old schema required without declaring it: every old client wrote it with any
    // value, so a declaration may only keep accepting everything.
    let undeclared = label(
        "open",
        r#""a": {"type": "integer"}"#,
        r#", "required": ["b"]"#,
    );
    let declared = label(
        "open",
        r#""a": {"type": "integer"}, "b": {"type": "string"}"#,
        r#", "required": ["b"]"#,
    );
    assert!(!single(&undeclared, &declared).2);
    let declared_any = label(
        "open",
        r#""a": {"type": "integer"}, "b": {}"#,
        r#", "required": ["b"]"#,
    );
    assert!(single(&undeclared, &declared_any).2);
    // explicit additionalProperties wins over the mode
    let strict = label(
        "open",
        r#""a": {"type": "integer"}"#,
        r#", "additionalProperties": false"#,
    );
    let strict_more = label(
        "open",
        r#""a": {"type": "integer"}, "b": {"type": "string"}"#,
        r#", "additionalProperties": false"#,
    );
    assert!(single(&strict, &strict_more).2);
    assert_eq!(
        single(&open, &strict),
        (
            SchemaChangeKind::AdditionalPropertiesChanged,
            Some("$".into()),
            false
        )
    );
    assert!(single(&strict, &open).2);
}

#[test]
fn required_changes() {
    let optional = label("closed", r#""a": {"type": "integer"}"#, "");
    let required = label(
        "closed",
        r#""a": {"type": "integer"}"#,
        r#", "required": ["a"]"#,
    );
    assert_eq!(
        single(&optional, &required),
        (SchemaChangeKind::RequiredAdded, Some("$.a".into()), false)
    );
    assert_eq!(
        single(&required, &optional),
        (SchemaChangeKind::RequiredRemoved, Some("$.a".into()), true)
    );
}

#[test]
fn type_changes_are_compatible_when_widening() {
    let with = |node: &str| label("closed", &format!(r#""a": {node}"#), "");
    for (from, to, compatible) in [
        (
            r#"{"type": "integer"}"#,
            r#"{"type": ["integer", "null"]}"#,
            true,
        ),
        (
            r#"{"type": ["integer", "null"]}"#,
            r#"{"type": "integer"}"#,
            false,
        ),
        (r#"{"type": "integer"}"#, r#"{"type": "number"}"#, false),
        (
            r#"{"type": "string"}"#,
            r#"{"type": "string", "format": "uuid"}"#,
            false,
        ),
        (
            r#"{"type": "string", "format": "uuid"}"#,
            r#"{"type": "string"}"#,
            false,
        ),
        (r#"{"type": "integer"}"#, r#"{}"#, true),
        (r#"{}"#, r#"{"type": "integer"}"#, false),
        (
            r#"{"type": "integer"}"#,
            r#"{"anyOf": [{"type": "integer"}, {"type": "string"}]}"#,
            true,
        ),
    ] {
        let (kind, path, ok) = single(&with(from), &with(to));
        assert_eq!(kind, SchemaChangeKind::TypeChanged, "{from} -> {to}");
        assert_eq!(path.as_deref(), Some("$.a"));
        assert_eq!(ok, compatible, "{from} -> {to}");
    }
}

#[test]
fn constraint_changes_are_compatible_when_relaxing() {
    let with = |keywords: &str| {
        label(
            "closed",
            &format!(r#""a": {{"type": ["integer", "string", "array"]{keywords}}}"#),
            "",
        )
    };
    for (from, to, compatible) in [
        (r#", "minimum": 0"#, r#", "minimum": -1"#, true),
        (r#", "minimum": 0"#, r#", "minimum": 1"#, false),
        ("", r#", "minimum": 0"#, false),
        (r#", "minimum": 0"#, "", true),
        (r#", "maximum": 5"#, r#", "maximum": 6.5"#, true),
        (
            r#", "exclusiveMaximum": 5"#,
            r#", "exclusiveMaximum": 4"#,
            false,
        ),
        (r#", "maxLength": 10"#, r#", "maxLength": 20"#, true),
        (r#", "minLength": 1"#, r#", "minLength": 2"#, false),
        (r#", "minItems": 2"#, r#", "minItems": 1"#, true),
        (r#", "pattern": "^a""#, "", true),
        (r#", "enum": [1, 2]"#, r#", "enum": [1, 2, 3]"#, true),
        (r#", "enum": [1, 2]"#, r#", "enum": [1.0]"#, false),
        (r#", "const": 1"#, "", true),
        ("", r#", "const": 1"#, false),
    ] {
        let (kind, _, ok) = single(&with(from), &with(to));
        assert_eq!(kind, SchemaChangeKind::ConstraintChanged, "{from} -> {to}");
        assert_eq!(ok, compatible, "{from} -> {to}");
    }
    // the pattern change itself
    let (_, _, ok) = single(&with(r#", "pattern": "^a""#), &with(r#", "pattern": "^b""#));
    assert!(!ok);
}

#[test]
fn nested_changes_name_their_path() {
    let before = label(
        "closed",
        r#""address": {"type": "object", "properties": {"zip": {"type": "string"}}},
           "tags": {"type": "array", "items": {"type": "string", "maxLength": 5}}"#,
        "",
    );
    let after = label(
        "closed",
        r#""address": {"type": "object", "properties": {"zip": {"type": "string"}},
                       "required": ["zip"]},
           "tags": {"type": "array", "items": {"type": "string", "maxLength": 8}}"#,
        "",
    );
    let diff = diff(&before, &after);
    let paths: Vec<(SchemaChangeKind, Option<String>, bool)> = diff
        .changes
        .iter()
        .map(|c| (c.kind, c.path.clone(), c.compatible))
        .collect();
    assert!(paths.contains(&(
        SchemaChangeKind::RequiredAdded,
        Some("$.address.zip".into()),
        false
    )));
    assert!(paths.contains(&(
        SchemaChangeKind::ConstraintChanged,
        Some("$.tags[*]".into()),
        true
    )));
    assert!(!diff.compatible());
    for change in &diff.changes {
        assert_eq!(change.element, Some(ElementKind::Vertex));
        assert_eq!(change.label.as_deref(), Some("v"));
    }
}

/// With a new mode `none` nothing is enforced, so every change is compatible.
#[test]
fn a_new_mode_none_makes_everything_compatible() {
    let closed = label(
        "closed",
        r#""a": {"type": "integer"}"#,
        r#", "required": ["a"]"#,
    );
    let mut none = label("none", r#""a": {"type": "string"}"#, "");
    none.vertices.insert("b".into(), VertexSchema::default());
    assert!(diff(&closed, &none).compatible());
}

#[test]
fn the_diff_as_data() {
    let before = label("closed", r#""a": {"type": "integer"}"#, "");
    let after = label("closed", r#""a": {"type": ["integer", "null"]}"#, "");
    let value = diff(&before, &after).to_value();
    let map = value.as_object().unwrap();
    assert_eq!(map["compatible"], ElementProperty::Boolean(true));
    let changes = map["changes"].as_array().unwrap();
    let change = changes[0].as_object().unwrap();
    assert_eq!(change["kind"], ElementProperty::from("type_changed"));
    assert_eq!(change["element"], ElementProperty::from("vertex"));
    assert_eq!(change["label"], ElementProperty::from("v"));
    assert_eq!(change["path"], ElementProperty::from("$.a"));
    assert_eq!(change["before"], ElementProperty::from("int64"));
    assert_eq!(change["after"], ElementProperty::from("union<int64|null>"));
}

/// Removing a property declaration is compatible exactly when the (new) object admits undeclared
/// keys: old data stays valid and old writers are still accepted. It is listed either way.
#[test]
fn property_removal_is_compatible_from_an_open_object() {
    let two = r#""a": {"type": "integer"}, "nick": {"type": "string"}"#;
    let one = r#""a": {"type": "integer"}"#;
    // open, optional key removed
    assert_eq!(
        single(&label("open", two, ""), &label("open", one, "")),
        (
            SchemaChangeKind::PropertyRemoved,
            Some("$.nick".into()),
            true
        )
    );
    // open, required key removed: both changes are compatible
    let required = r#", "required": ["nick"]"#;
    let removed = diff(&label("open", two, required), &label("open", one, ""));
    let kinds: Vec<(SchemaChangeKind, bool)> = removed
        .changes
        .iter()
        .map(|c| (c.kind, c.compatible))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (SchemaChangeKind::PropertyRemoved, true),
            (SchemaChangeKind::RequiredRemoved, true)
        ]
    );
    assert!(removed.compatible());
    // closed: incompatible (the key would now be rejected), also with an explicit
    // additionalProperties: false in an open graph
    assert_eq!(
        single(&label("closed", two, ""), &label("closed", one, "")),
        (
            SchemaChangeKind::PropertyRemoved,
            Some("$.nick".into()),
            false
        )
    );
    let strict = r#", "additionalProperties": false"#;
    assert!(!single(&label("open", two, strict), &label("open", one, strict)).2);
    // and an explicit additionalProperties: true in a closed graph admits it
    let loose = r#", "additionalProperties": true"#;
    assert!(single(&label("closed", two, loose), &label("closed", one, loose)).2);
    // nested open object
    let nested = |mode: &str, inner: &str| {
        label(
            mode,
            &format!(r#""address": {{"type": "object", "properties": {{{inner}}}}}"#),
            "",
        )
    };
    let zip_city = r#""zip": {"type": "string"}, "city": {"type": "string"}"#;
    let zip = r#""zip": {"type": "string"}"#;
    assert_eq!(
        single(&nested("open", zip_city), &nested("open", zip)),
        (
            SchemaChangeKind::PropertyRemoved,
            Some("$.address.city".into()),
            true
        )
    );
    assert!(!single(&nested("closed", zip_city), &nested("closed", zip)).2);
}
