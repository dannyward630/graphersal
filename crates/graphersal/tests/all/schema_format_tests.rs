//! The schema format: the envelope, the supported JSON Schema 2020-12 subset (strict: unknown
//! and unsupported keywords are errors with their location), `$ref`/`$defs` inlining, the
//! canonical output, a pasted Pydantic `model_json_schema()`, required vs nullable,
//! `additionalProperties` x mode x multi-label, and the value-type names.
use graphersal::prelude::*;
use graphersal::schema::{EdgeSchema, PropertySchema, ValueType, type_of};

use crate::schema_support::{graph_with, props, schema};

fn load_err(json: &str) -> SchemaError {
    GraphSchema::from_json(json).expect_err(json)
}

/// The location and reason of a format error.
fn invalid(json: &str) -> (String, String, String) {
    match load_err(json) {
        SchemaError::InvalidSchema {
            path, reason, hint, ..
        } => (path, reason, hint),
        other => panic!("expected InvalidSchema, got {other:?}"),
    }
}

/// A label schema with one property `p` declared as `node`.
fn one(node: &str) -> String {
    format!(
        r#"{{"vertices": {{"v": {{"schema": {{"type": "object", "properties": {{"p": {node}}}}}}}}}}}"#
    )
}

// ---------------------------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------------------------

/// The canonical rendering of every `ValueType` variant is a stable contract: it is what a
/// diagnostic and a script-visible type name print.
#[test]
fn value_type_display_spellings_are_pinned() {
    assert_eq!(ValueType::Null.to_string(), "null");
    assert_eq!(ValueType::Boolean.to_string(), "boolean");
    assert_eq!(ValueType::Int64.to_string(), "int64");
    assert_eq!(ValueType::Float64.to_string(), "float64");
    assert_eq!(ValueType::String.to_string(), "string");
    assert_eq!(ValueType::Uuid.to_string(), "uuid");
    assert_eq!(
        ValueType::Array(Box::new(ValueType::Array(Box::new(ValueType::Int64)))).to_string(),
        "array<array<int64>>"
    );
    assert_eq!(ValueType::Object.to_string(), "object");
    assert_eq!(
        ValueType::Union(vec![ValueType::Int64, ValueType::String]).to_string(),
        "union<int64|string>"
    );
    assert_eq!(ValueType::Union(Vec::new()).to_string(), "union<>");
}

/// `type_of` reports the exact logical type and never guesses from string content (rule 7).
#[test]
fn type_of_reports_exact_logical_types() {
    assert_eq!(type_of(&ElementProperty::Null), Some(ValueType::Null));
    assert_eq!(type_of(&ElementProperty::from(7)), Some(ValueType::Int64));
    assert_eq!(
        type_of(&ElementProperty::from(7.5)),
        Some(ValueType::Float64)
    );
    for candidate in [
        "1e2e6a4f-2a4a-4c0b-9f3f-0d6b4f6f2a11",
        "2026-09-22",
        "192.168.0.1",
    ] {
        assert_eq!(
            type_of(&ElementProperty::from(candidate)),
            Some(ValueType::String),
            "'{candidate}' must stay a string"
        );
    }
    assert_eq!(
        type_of(&ElementProperty::Array(vec![1.into(), "x".into()])),
        Some(ValueType::Array(Box::new(ValueType::Union(vec![
            ValueType::Int64,
            ValueType::String
        ]))))
    );
    assert_eq!(
        type_of(&ElementProperty::Uuid(1)),
        Some(ValueType::Uuid),
        "a real uuid is uuid"
    );
    let element = || Box::new(graphersal::prelude::PropertyMap::default());
    assert_eq!(type_of(&ElementProperty::Vertex(element())), None);
    assert_eq!(
        type_of(&ElementProperty::Array(vec![ElementProperty::Edge(
            element()
        )])),
        None
    );
}

/// A node's type name uses the engine's spelling: `integer` is `int64`, `number` `float64`,
/// `string` + `format: uuid` is `uuid`.
#[test]
fn node_type_names_use_the_engine_spelling() {
    let names = [
        (r#"{"type": "integer"}"#, "int64"),
        (r#"{"type": "number"}"#, "float64"),
        (r#"{"type": "string", "format": "uuid"}"#, "uuid"),
        (r#"{"type": ["integer", "null"]}"#, "union<int64|null>"),
        (
            r#"{"type": "array", "items": {"type": "string"}}"#,
            "array<string>",
        ),
        (r#"{"type": "array"}"#, "array"),
        (
            r#"{"anyOf": [{"type": "integer"}, {"type": "string"}]}"#,
            "union<int64|string>",
        ),
        (r#"{}"#, "any"),
    ];
    for (node, name) in names {
        let schema = schema(&one(node));
        assert_eq!(
            schema.vertices["v"].schema.properties["p"].type_name(),
            name,
            "{node}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------------------------

#[test]
fn the_envelope_has_defaults_and_rejects_typos() {
    let empty = schema("{}");
    assert!(empty.is_empty());
    assert_eq!(empty.mode, SchemaMode::None);
    let full = schema(
        r#"{"$schema": "https://graphersal.dev/schema/v1", "meta": {"version": "2.3", "x": [1]},
            "mode": "open", "vertices": {}, "edges": {}}"#,
    );
    assert_eq!(full.mode, SchemaMode::Open);
    assert!(!full.is_empty(), "meta alone is a schema");

    // F3: a typo is never ignored, at any level of the envelope.
    for (json, path) in [
        (r#"{"mdoe": "closed"}"#, "/mdoe"),
        (
            r#"{"vertices": {"a": {"propertes": {}}}}"#,
            "/vertices/a/propertes",
        ),
        (
            r#"{"edges": {"e": {"conections": []}}}"#,
            "/edges/e/conections",
        ),
        (
            r#"{"edges": {"e": {"connections": [{"from": "a", "to": "b", "label": "e"}]}}}"#,
            "/edges/e/connections/0/label",
        ),
    ] {
        let (at, reason, _) = invalid(json);
        assert_eq!(at, path, "{json}");
        assert!(reason.starts_with("unknown key"), "{reason}");
    }
    let (_, _, hint) = invalid(r#"{"version": 1}"#);
    assert!(hint.contains("meta"), "{hint}");
    let (_, _, hint) = invalid(r#"{"vertices": {"a": {"properties": {}}}}"#);
    assert!(hint.contains("\"schema\""), "{hint}");
    let (at, reason, _) =
        invalid(r#"{"$schema": "https://graphersal.dev/schemas/graphersal-schema-v2.json"}"#);
    assert_eq!(at, "/$schema");
    assert!(reason.contains("unsupported schema format"), "{reason}");
    let (_, reason, _) = invalid(r#"{"mode": "Closed"}"#);
    assert!(reason.contains("unknown mode 'Closed'"), "{reason}");
    let (_, reason, _) = invalid(r#"{"meta": 3}"#);
    assert!(reason.contains("meta must be an object"), "{reason}");
    let (_, reason, _) = invalid(r#"{"vertices": {"": {}}}"#);
    assert!(reason.contains("must not be empty"), "{reason}");
    let (_, reason, _) = invalid(r#"{"edges": {"e": {"connections": [{"from": "a"}]}}}"#);
    assert!(reason.contains("\"to\""), "{reason}");
    let (_, reason, _) = invalid(r#"{"mode": "open", "mode": "closed"}"#);
    assert!(reason.contains("duplicate key"), "{reason}");
}

#[test]
fn a_label_schema_must_be_an_object_node() {
    for node in [
        r#"{"type": "string"}"#,
        r#"{"type": ["object", "null"]}"#,
        r#"{"anyOf": [{"type": "object"}]}"#,
        r#"{"type": "object", "enum": [{}]}"#,
        r#"{}"#,
    ] {
        let json = format!(r#"{{"vertices": {{"v": {{"schema": {node}}}}}}}"#);
        let (at, reason, _) = invalid(&json);
        assert_eq!(at, "/vertices/v/schema", "{node}");
        assert!(reason.contains("object node"), "{node}: {reason}");
    }
    // A label without a schema is an object that declares nothing.
    let schema = schema(r#"{"vertices": {"v": {}}, "edges": {"e": {}}}"#);
    assert_eq!(schema.vertices["v"].schema, PropertySchema::object());
    assert!(schema.edges["e"].connections.is_empty());
}

// ---------------------------------------------------------------------------------------------
// Keywords
// ---------------------------------------------------------------------------------------------

#[test]
fn every_supported_keyword_round_trips_in_canonical_form() {
    let json = r#"{"$schema":"https://graphersal.dev/schema/v1","meta":{"v":"1"},"mode":"closed","vertices":{"v":{"schema":{"title":"V","description":"a vertex","$comment":"c","type":"object","properties":{"i":{"type":["integer","null"],"minimum":0,"exclusiveMaximum":10.5},"s":{"type":"string","enum":["a","b"],"minLength":1,"maxLength":3,"pattern":"^[ab]$"},"c":{"type":"string","const":"x","default":"x"},"u":{"type":"string","format":"uuid"},"a":{"type":"array","minItems":1,"maxItems":2,"items":{"type":"object","properties":{"k":{"type":"boolean"}},"required":["k"],"additionalProperties":false}},"o":{"type":"object","additionalProperties":true},"x":{"anyOf":[{"type":"integer"},{"type":"string"}],"examples":[1,"a"],"deprecated":true},"n":{"exclusiveMinimum":0,"maximum":5}},"required":["i","s"],"additionalProperties":false}}},"edges":{"e":{"connections":[{"from":"v","to":"*"}],"schema":{"type":"object"}}}}"#;
    let loaded = schema(json);
    // The canonical writer orders keywords by kind; loading it back gives the same schema.
    let written = loaded.to_json();
    assert_eq!(schema(&written), loaded);
    assert_eq!(schema(&loaded.to_json_pretty()), loaded);
    // Integers stay integers, floats stay floats.
    assert!(written.contains(r#""minimum":0,"#), "{written}");
    assert!(written.contains(r#""exclusiveMaximum":10.5"#), "{written}");
    // `$schema`, `mode`, `vertices`, `edges` are always written.
    let minimal = schema("{}").to_json();
    assert_eq!(
        minimal,
        r#"{"$schema":"https://graphersal.dev/schema/v1","mode":"none","vertices":{},"edges":{}}"#
    );
}

#[test]
fn unknown_keywords_are_errors_with_their_location() {
    let (at, reason, hint) = invalid(&one(r#"{"type": "integer", "minimun": 0}"#));
    assert_eq!(at, "/vertices/v/schema/properties/p/minimun");
    assert_eq!(reason, "unknown keyword 'minimun'");
    assert!(hint.contains("minimum"), "{hint}");
    // deep inside items and anyOf
    let (at, _, _) = invalid(&one(
        r#"{"type": "array", "items": {"anyOf": [{"type": "string"}, {"type": "integer", "bogus": 1}]}}"#,
    ));
    assert_eq!(at, "/vertices/v/schema/properties/p/items/anyOf/1/bogus");
}

#[test]
fn unsupported_json_schema_keywords_are_errors_with_a_reason() {
    for (keyword, node, fragment) in [
        ("oneOf", r#"{"oneOf": [{"type": "string"}]}"#, "anyOf"),
        ("allOf", r#"{"allOf": [{"type": "string"}]}"#, "allOf"),
        ("not", r#"{"not": {"type": "string"}}"#, "allOf"),
        ("if", r#"{"if": {"type": "string"}}"#, "allOf"),
        (
            "prefixItems",
            r#"{"type": "array", "prefixItems": []}"#,
            "tuples",
        ),
        (
            "uniqueItems",
            r#"{"type": "array", "uniqueItems": true}"#,
            "not supported yet",
        ),
        (
            "multipleOf",
            r#"{"type": "integer", "multipleOf": 2}"#,
            "not supported yet",
        ),
        (
            "patternProperties",
            r#"{"type": "object", "patternProperties": {}}"#,
            "properties",
        ),
        (
            "nullable",
            r#"{"type": "string", "nullable": true}"#,
            "\"null\"",
        ),
        ("variants", r#"{"variants": []}"#, "anyOf"),
        (
            "min_length",
            r#"{"type": "string", "min_length": 1}"#,
            "minLength",
        ),
        (
            "readOnly",
            r#"{"type": "string", "readOnly": true}"#,
            "annotations",
        ),
    ] {
        let (at, reason, hint) = invalid(&one(node));
        assert!(at.ends_with(keyword), "{keyword}: {at}");
        assert_eq!(reason, format!("unsupported keyword '{keyword}'"));
        assert!(hint.contains(fragment), "{keyword}: {hint}");
    }
}

#[test]
fn only_the_uuid_format_exists_and_dates_are_errors() {
    for format in ["date-time", "date", "time", "email", "uri"] {
        let (at, reason, hint) = invalid(&one(&format!(
            r#"{{"type": "string", "format": "{format}"}}"#
        )));
        assert!(at.ends_with("/format"), "{at}");
        assert!(reason.contains(format), "{reason}");
        assert!(hint.contains("0.2.0"), "{hint}");
    }
    let (_, reason, _) = invalid(&one(r#"{"format": "uuid"}"#));
    assert!(reason.contains("format needs"), "{reason}");
    let (_, reason, _) = invalid(&one(r#"{"type": "integer", "format": "uuid"}"#));
    assert!(reason.contains("format applies to string"), "{reason}");
}

#[test]
fn type_names_follow_json_schema_and_old_names_get_a_hint() {
    for name in [
        "null", "boolean", "integer", "number", "string", "array", "object",
    ] {
        schema(&one(&format!(r#"{{"type": "{name}"}}"#)));
    }
    for (name, fragment) in [
        ("int64", "\"integer\""),
        ("float64", "\"number\""),
        ("uuid", "\"format\": \"uuid\""),
        ("union", "anyOf"),
        ("Integer", "integer (int64)"),
    ] {
        let (_, reason, hint) = invalid(&one(&format!(r#"{{"type": "{name}"}}"#)));
        assert_eq!(reason, format!("unknown type '{name}'"));
        assert!(hint.contains(fragment), "{name}: {hint}");
    }
    let (_, reason, _) = invalid(&one(r#"{"type": ["string", "string"]}"#));
    assert!(reason.contains("listed twice"), "{reason}");
    let (_, reason, _) = invalid(&one(r#"{"type": []}"#));
    assert!(reason.contains("non-empty"), "{reason}");
}

#[test]
fn keywords_must_fit_the_declared_type() {
    for (node, keyword) in [
        (r#"{"type": "integer", "minLength": 1}"#, "minLength"),
        (r#"{"type": "string", "minimum": 1}"#, "minimum"),
        (r#"{"type": "string", "items": {}}"#, "items"),
        (r#"{"type": "integer", "properties": {}}"#, "properties"),
        (r#"{"type": "string", "maxItems": 1}"#, "maxItems"),
    ] {
        let (_, reason, _) = invalid(&one(node));
        assert!(reason.starts_with(keyword), "{node}: {reason}");
    }
    // Without `type` a kind keyword applies to values of its kind (JSON Schema semantics).
    schema(&one(r#"{"minimum": 1, "minLength": 2}"#));
    // `minimum` fits `integer` too.
    schema(&one(r#"{"type": "integer", "minimum": 1}"#));
}

#[test]
fn malformed_keyword_values_are_errors() {
    for (node, fragment) in [
        (
            r#"{"type": "string", "minLength": -1}"#,
            "non-negative integer",
        ),
        (
            r#"{"type": "string", "minLength": 1.5}"#,
            "non-negative integer",
        ),
        (r#"{"type": "integer", "minimum": "0"}"#, "must be a number"),
        (r#"{"enum": []}"#, "must not be empty"),
        (r#"{"anyOf": []}"#, "must not be empty"),
        (r#"{"type": "array", "items": [{}]}"#, "tuples"),
        (
            r#"{"type": "object", "required": ["a", "a"]}"#,
            "listed twice",
        ),
        (
            r#"{"type": "object", "additionalProperties": {"type": "integer"}}"#,
            "map of T",
        ),
        (r#"true"#, "boolean schema"),
    ] {
        let (_, reason, _) = invalid(&one(node));
        assert!(reason.contains(fragment), "{node}: {reason}");
    }
    let (_, _, hint) = invalid(&one(
        r#"{"type": "object", "additionalProperties": {"type": "integer"}}"#,
    ));
    assert!(hint.contains("dict[str, T]"), "{hint}");
}

// ---------------------------------------------------------------------------------------------
// $ref / $defs
// ---------------------------------------------------------------------------------------------

#[test]
fn local_references_are_inlined_with_their_sibling_annotations() {
    let loaded = schema(
        r##"{"vertices": {"v": {"schema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": "urn:v",
            "$defs": {
                "Zip": {"type": "string", "pattern": "^\\d{5}$"},
                "Address": {"type": "object", "properties": {"zip": {"$ref": "#/$defs/Zip"}},
                            "required": ["zip"]}
            },
            "type": "object",
            "properties": {
                "home": {"$ref": "#/$defs/Address", "description": "home address"},
                "work": {"anyOf": [{"$ref": "#/$defs/Address"}, {"type": "null"}], "default": null}
            }}}}}"##,
    );
    let root = &loaded.vertices["v"].schema;
    let home = &root.properties["home"];
    assert_eq!(home.type_name(), "object");
    assert_eq!(
        home.annotations().unwrap().description.as_deref(),
        Some("home address")
    );
    assert_eq!(
        home.properties["zip"]
            .constraints()
            .unwrap()
            .pattern
            .as_ref()
            .unwrap()
            .as_str(),
        r"^\d{5}$"
    );
    // Written back without $ref/$defs.
    let text = loaded.to_json();
    assert!(!text.contains("$ref") && !text.contains("$defs"), "{text}");
}

#[test]
fn bad_references_are_errors() {
    let wrap = |defs: &str, node: &str| {
        format!(
            r#"{{"vertices": {{"v": {{"schema": {{"$defs": {defs}, "type": "object",
               "properties": {{"p": {node}}}}}}}}}}}"#
        )
    };
    let (at, reason, _) = invalid(&wrap(r##"{"A": {"$ref": "#/$defs/A"}}"##, "{}"));
    assert!(reason.contains("refers to itself"), "{at}: {reason}");
    let (_, reason, _) = invalid(&wrap(
        r##"{"A": {"type": "object", "properties": {"b": {"$ref": "#/$defs/B"}}},
            "B": {"type": "array", "items": {"$ref": "#/$defs/A"}}}"##,
        r##"{"$ref": "#/$defs/A"}"##,
    ));
    assert!(reason.contains("cycle"), "{reason}");
    let (at, reason, _) = invalid(&wrap("{}", r##"{"$ref": "#/$defs/Missing"}"##));
    assert_eq!(at, "/vertices/v/schema/properties/p/$ref");
    assert!(reason.contains("is not defined"), "{reason}");
    let (_, reason, _) = invalid(&wrap("{}", r#"{"$ref": "https://example.org/s.json"}"#));
    assert!(reason.contains("unsupported reference"), "{reason}");
    let (_, reason, _) = invalid(&wrap(
        r#"{"A": {"type": "string"}}"#,
        r##"{"$ref": "#/$defs/A", "minLength": 1}"##,
    ));
    assert!(reason.contains("next to $ref"), "{reason}");
    let (_, reason, _) = invalid(&one(r#"{"$defs": {}}"#));
    assert!(reason.contains("only allowed at the root"), "{reason}");
    // An unused but invalid definition is an error too.
    let (at, _, _) = invalid(&wrap(r#"{"Bad": {"type": "int64"}}"#, "{}"));
    assert_eq!(at, "/vertices/v/schema/$defs/Bad/type");
}

// ---------------------------------------------------------------------------------------------
// Pydantic
// ---------------------------------------------------------------------------------------------

/// Pydantic 2.13 `Person.model_json_schema()` (INPUT.md 3.2) without the parts that are errors
/// by decision: `created` (`format: date-time`) and `labels` (`dict[str, int]`, a map of T).
const PYDANTIC: &str = r##"{
  "$defs": {
    "Address": {"additionalProperties": false, "type": "object", "required": ["street", "zip"],
                "properties": {"street": {"title": "Street", "type": "string"},
                               "zip": {"title": "Zip", "type": "string", "pattern": "^\\d{5}$"}},
                "title": "Address"},
    "Role": {"enum": ["admin", "user"], "title": "Role", "type": "string"}
  },
  "title": "Person", "description": "A person vertex.", "type": "object",
  "required": ["id", "name", "score", "level", "any_"],
  "properties": {
    "id": {"title": "Id", "type": "string", "format": "uuid"},
    "name": {"title": "Name", "type": "string", "minLength": 1, "maxLength": 80,
             "description": "Full name"},
    "age": {"anyOf": [{"type": "integer", "minimum": 0, "maximum": 150}, {"type": "null"}],
            "default": null, "title": "Age"},
    "score": {"title": "Score", "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 1},
    "tags": {"title": "Tags", "type": "array", "items": {"type": "string"}, "maxItems": 10},
    "address": {"anyOf": [{"$ref": "#/$defs/Address"}, {"type": "null"}], "default": null},
    "kind": {"const": "human", "default": "human", "title": "Kind", "type": "string"},
    "role": {"$ref": "#/$defs/Role", "default": "user"},
    "level": {"enum": ["a", "b"], "title": "Level", "type": "string"},
    "any_": {"anyOf": [{"type": "integer"}, {"type": "string"}], "title": "Any"}
  }
}"##;

fn pydantic_schema() -> GraphSchema {
    schema(&format!(
        r#"{{"mode": "closed", "vertices": {{"person": {{"schema": {PYDANTIC}}}}}}}"#
    ))
}

#[test]
fn a_pasted_pydantic_model_schema_loads_and_validates() {
    let loaded = pydantic_schema();
    let root = &loaded.vertices["person"].schema;
    assert_eq!(root.properties["id"].type_name(), "uuid");
    assert_eq!(root.properties["role"].type_name(), "string");
    assert_eq!(
        root.properties["role"].annotations().unwrap().default,
        Some(ElementProperty::from("user"))
    );
    let mut graph = graph_with(loaded);
    let id = "550e8400-e29b-41d4-a716-446655440000";
    let mut add = |extra: &[(&str, ElementProperty)]| {
        let mut entries = vec![
            ("id", ElementProperty::from(id)),
            ("name", "Ann".into()),
            ("score", 0.5.into()),
            ("level", "a".into()),
            ("any_", 1.into()),
        ];
        entries.extend_from_slice(extra);
        graph.add_vertex(None, vec!["person".into()], props(&entries))
    };
    add(&[]).expect("the required keys suffice");
    add(&[
        ("age", ElementProperty::Null),
        ("address", ElementProperty::Null),
        ("tags", ElementProperty::Array(vec!["x".into()])),
        ("kind", "human".into()),
        ("role", "admin".into()),
    ])
    .expect("optional keys, null where admitted");
    let mut address = graphersal::prelude::PropertyMap::default();
    address.insert("street", "Main".into());
    address.insert("zip", "12345".into());
    add(&[("address", ElementProperty::Object(Box::new(address)))]).expect("a nested model");
    for (key, bad) in [
        ("score", ElementProperty::from(1.0)),
        ("kind", "robot".into()),
        ("role", "root".into()),
        ("age", 151.into()),
        ("tags", ElementProperty::Null),
        ("any_", true.into()),
        ("extra", 1.into()),
    ] {
        assert!(add(&[(key, bad.clone())]).is_err(), "{key} = {bad:?}");
    }
}

#[test]
fn the_unedited_pydantic_output_names_what_to_remove() {
    let with_date = PYDANTIC.replace(
        r#""level":"#,
        r#""created": {"format": "date-time", "type": "string"}, "level":"#,
    );
    let json = format!(r#"{{"vertices": {{"person": {{"schema": {with_date}}}}}}}"#);
    let (at, _, hint) = invalid(&json);
    assert_eq!(at, "/vertices/person/schema/properties/created/format");
    assert!(hint.contains("0.2.0"), "{hint}");
    let with_map = PYDANTIC.replace(
        r#""level":"#,
        r#""labels": {"additionalProperties": {"type": "integer"}, "type": "object"}, "level":"#,
    );
    let json = format!(r#"{{"vertices": {{"person": {{"schema": {with_map}}}}}}}"#);
    let (at, _, _) = invalid(&json);
    assert_eq!(
        at,
        "/vertices/person/schema/properties/labels/additionalProperties"
    );
}

// ---------------------------------------------------------------------------------------------
// required vs nullable
// ---------------------------------------------------------------------------------------------

/// The four combinations of JSON Schema: required or optional x null admitted or not.
#[test]
fn required_and_nullable_are_independent() {
    let s = schema(
        r#"{"mode": "open", "vertices": {"v": {"schema": {"type": "object", "properties": {
            "rq": {"type": "string"}, "rn": {"type": ["string", "null"]},
            "oq": {"type": "string"}, "on": {"anyOf": [{"type": "string"}, {"type": "null"}]}},
            "required": ["rq", "rn"]}}}}"#,
    );
    let mut graph = graph_with(s);
    let base = || {
        vec![
            ("rq", ElementProperty::from("x")),
            ("rn", ElementProperty::Null),
        ]
    };
    let mut add = |entries: Vec<(&str, ElementProperty)>| {
        graph.add_vertex(None, vec!["v".into()], props(&entries))
    };
    add(base()).expect("required present (one of them null where admitted), optional absent");
    let with = |key: &'static str, value: ElementProperty| {
        let mut entries = base();
        entries.retain(|(k, _)| *k != key);
        entries.push((key, value));
        entries
    };
    assert!(
        add(with("rq", ElementProperty::Null)).is_err(),
        "required, null not admitted"
    );
    assert!(
        add(with("oq", ElementProperty::Null)).is_err(),
        "optional, null not admitted"
    );
    assert!(
        add(with("on", ElementProperty::Null)).is_ok(),
        "optional, null admitted"
    );
    assert!(add(with("oq", "y".into())).is_ok());
    // absent required keys
    assert!(matches!(
        add(vec![("rn", ElementProperty::Null)]),
        Err(GraphError::Schema(
            SchemaError::MissingRequiredProperty { .. }
        ))
    ));
    assert!(
        matches!(
            add(vec![("rq", ElementProperty::from("x"))]),
            Err(GraphError::Schema(
                SchemaError::MissingRequiredProperty { .. }
            ))
        ),
        "a nullable key that is required must be present"
    );
}

// ---------------------------------------------------------------------------------------------
// additionalProperties x mode x multi-label
// ---------------------------------------------------------------------------------------------

fn additional_schema(mode: &str, a: &str, b: &str) -> GraphSchema {
    let field = |value: &str| {
        if value.is_empty() {
            String::new()
        } else {
            format!(r#", "additionalProperties": {value}"#)
        }
    };
    schema(&format!(
        r#"{{"mode": "{mode}", "vertices": {{
            "A": {{"schema": {{"type": "object", "properties": {{"a": {{"type": "integer"}}}}{}}}}},
            "B": {{"schema": {{"type": "object", "properties": {{"b": {{"type": "integer"}}}}{}}}}}}}}}"#,
        field(a),
        field(b)
    ))
}

#[test]
fn additional_properties_follows_the_mode_unless_stated_and_any_label_may_admit() {
    // (mode, A's additionalProperties, B's, labels, admitted?)
    let cases = [
        ("open", "", "", vec!["A"], true),
        ("closed", "", "", vec!["A"], false),
        ("closed", "true", "", vec!["A"], true),
        ("open", "false", "", vec!["A"], false),
        // multi-label: allowed if ANY label allows an undeclared key
        ("closed", "", "true", vec!["A", "B"], true),
        ("closed", "", "", vec!["A", "B"], false),
        ("open", "false", "false", vec!["A", "B"], false),
        ("open", "false", "", vec!["A", "B"], true),
    ];
    for (mode, a, b, labels, admitted) in cases {
        let mut graph = graph_with(additional_schema(mode, a, b));
        let result = graph.add_vertex(
            None,
            labels.iter().map(|l| (*l).into()).collect(),
            props(&[("free", 1.into())]),
        );
        assert_eq!(
            result.is_ok(),
            admitted,
            "{mode} A={a} B={b} {labels:?}: {result:?}"
        );
    }
    // A key one label declares is checked against that label, whatever the other says.
    let mut graph = graph_with(additional_schema("closed", "", "true"));
    assert!(
        graph
            .add_vertex(
                None,
                vec!["A".into(), "B".into()],
                props(&[("a", "x".into())])
            )
            .is_err()
    );
}

/// F4: a free-form object in a closed graph is `additionalProperties: true` on that object.
#[test]
fn a_free_form_object_in_a_closed_graph() {
    let s = schema(
        r#"{"mode": "closed", "vertices": {"v": {"schema": {"type": "object", "properties": {
            "_meta": {"type": "object", "additionalProperties": true},
            "strict": {"type": "object", "properties": {"k": {"type": "integer"}}}}}}}}"#,
    );
    let mut graph = graph_with(s);
    let mut meta = graphersal::prelude::PropertyMap::default();
    meta.insert("anything", 1.into());
    assert!(
        graph
            .add_vertex(
                None,
                vec!["v".into()],
                props(&[("_meta", ElementProperty::Object(Box::new(meta.clone())))])
            )
            .is_ok()
    );
    // The closed default still applies to an object that does not state it.
    assert!(matches!(
        graph.add_vertex(None, vec!["v".into()], props(&[("strict", ElementProperty::Object(Box::new(meta)))])),
        Err(GraphError::Schema(SchemaError::UndeclaredProperty { ref property_name, .. })) if property_name == "$.strict.anything"
    ));
}

#[test]
fn a_rust_built_schema_equals_its_json() {
    let built = GraphSchema::new()
        .with_mode(SchemaMode::Closed)
        .with_vertex(
            "person",
            PropertySchema::object()
                .with_required_property("name", PropertySchema::string())
                .with_property("age", PropertySchema::integer().nullable()),
        )
        .with_edge(
            "knows",
            EdgeSchema::new(PropertySchema::object()).with_connection("person", "person"),
        );
    let loaded = schema(
        r#"{"mode": "closed", "vertices": {"person": {"schema": {"type": "object", "properties": {
            "name": {"type": "string"}, "age": {"type": ["integer", "null"]}},
            "required": ["name"]}}},
           "edges": {"knows": {"connections": [{"from": "person", "to": "person"}],
                               "schema": {"type": "object"}}}}"#,
    );
    assert_eq!(built, loaded);
}
