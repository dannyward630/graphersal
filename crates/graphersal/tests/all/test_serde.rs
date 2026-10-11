//! Serde for values is plain JSON through the physical codec (`value/physical.rs`).
#![cfg(feature = "serde")]

use graphersal::prelude::Number;
use graphersal::prelude::*;
use graphersal::schema::PropertySchema;

const UUID_TEXT: &str = "123e4567-e89b-12d3-a456-426614174000";

fn object(fields: Vec<(&str, ElementProperty)>) -> ElementProperty {
    let mut map = PropertyMap::default();
    for (k, v) in fields {
        map.insert(k, v);
    }
    ElementProperty::Object(Box::new(map))
}

fn nested_with_uuid() -> ElementProperty {
    object(vec![
        (
            "id",
            ElementProperty::uuid_from_string(UUID_TEXT).expect("canonical UUID"),
        ),
        ("name", ElementProperty::String("marko".into())),
        ("age", ElementProperty::Number(29i64.into())),
        ("score", ElementProperty::Number(Number::from_f64(1.0))),
        ("active", ElementProperty::Boolean(true)),
        ("nick", ElementProperty::Null),
        (
            "tags",
            ElementProperty::Array(vec![
                ElementProperty::String("a".into()),
                object(vec![("k", ElementProperty::Number(2.5.into()))]),
            ]),
        ),
    ])
}

#[test]
fn element_property_serializes_as_plain_json() {
    let json = serde_json::to_string(&nested_with_uuid()).expect("serializes");
    assert_eq!(
        json,
        format!(
            r#"{{"id":"{UUID_TEXT}","name":"marko","age":29,"score":1.0,"active":true,"nick":null,"tags":["a",{{"k":2.5}}]}}"#
        )
    );
}

#[test]
fn element_property_deserializes_to_same_name_types() {
    let json = serde_json::to_string(&nested_with_uuid()).expect("serializes");
    let back: ElementProperty = serde_json::from_str(&json).expect("deserializes");
    let map = back.as_object().expect("object");
    // A UUID comes back as its string: plain JSON carries no logical type.
    assert_eq!(
        map.get("id"),
        Some(&ElementProperty::String(UUID_TEXT.into()))
    );
    assert_eq!(map.get("age"), Some(&ElementProperty::Number(29i64.into())));
    assert_eq!(
        map.get("score"),
        Some(&ElementProperty::Number(Number::from_f64(1.0)))
    );
    assert_eq!(map.get("nick"), Some(&ElementProperty::Null));
    let keys: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
    assert_eq!(
        keys,
        ["id", "name", "age", "score", "active", "nick", "tags"]
    );
}

#[test]
fn element_property_rejects_u64_overflow() {
    let err = serde_json::from_str::<ElementProperty>("18446744073709551615")
        .expect_err("u64 above i64::MAX must not be downcast");
    assert!(err.to_string().contains("18446744073709551615"), "{err}");
}

#[test]
fn nan_serialization_fails() {
    let nan = ElementProperty::Number(Number::from_f64(f64::NAN));
    let err = serde_json::to_string(&nan).expect_err("NaN has no JSON form");
    assert!(err.to_string().contains("non-finite"), "{err}");
}

#[test]
fn number_is_a_plain_json_number() {
    assert_eq!(serde_json::to_string(&Number::from_i64(-3)).unwrap(), "-3");
    assert_eq!(
        serde_json::to_string(&Number::from_f64(3.0)).unwrap(),
        "3.0"
    );
    assert_eq!(
        serde_json::from_str::<Number>("7").unwrap(),
        Number::from_i64(7)
    );
    assert_eq!(
        serde_json::from_str::<Number>("7.5").unwrap(),
        Number::from_f64(7.5)
    );
    assert!(serde_json::from_str::<Number>("\"7\"").is_err());
}

#[test]
fn schema_meta_is_plain_json_and_round_trips() {
    let mut metadata = graphersal::prelude::PropertyMap::default();
    metadata.insert("owner", ElementProperty::String("billing".into()));
    metadata.insert(
        "limits",
        object(vec![
            ("max", ElementProperty::Number(10i64.into())),
            ("ratio", ElementProperty::Number(0.5.into())),
            (
                "zones",
                ElementProperty::Array(vec![ElementProperty::String("eu".into())]),
            ),
        ]),
    );
    let schema = GraphSchema::new()
        .with_meta(ElementProperty::Object(Box::new(metadata)))
        .with_vertex(
            "item",
            PropertySchema::object().with_required_property("n", PropertySchema::integer()),
        );

    let json = serde_json::to_string(&schema).expect("serializes");
    assert!(
        json.contains(
            r#""meta":{"owner":"billing","limits":{"max":10,"ratio":0.5,"zones":["eu"]}}"#
        ),
        "{json}"
    );
    // serde writes exactly the canonical form.
    assert_eq!(json, schema.to_json());
    for wrapper in [
        "\"Object\"",
        "\"String\"",
        "\"Number\"",
        "\"NegInt\"",
        "\"Float\"",
    ] {
        assert!(!json.contains(wrapper), "{wrapper} in {json}");
    }
    let back: GraphSchema = serde_json::from_str(&json).expect("parses");
    assert_eq!(back, schema);
    // serde reads as strictly as the loader.
    assert!(serde_json::from_str::<GraphSchema>(r#"{"mdoe": "open"}"#).is_err());
}
