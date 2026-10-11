//! `asBool()` and `asNumber(GType.INT|LONG)` follow TinkerPop (any non-zero number is true, a float
//! truncates), while the schema coercion matrix and `cast()` stay strict.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run(script: &str) -> Result<Vec<String>, String> {
    let value = eval_value(modern(), script).map_err(|e| format!("{e}"))?;
    let items = value.into_array().map_err(|e| e.to_string())?;
    Ok(items.iter().map(|v| format!("{v:?}")).collect())
}

fn ok(script: &str) -> Vec<String> {
    run(script).unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn bool_of(literal: &str) -> Result<String, String> {
    let camel = run(&format!("g.inject({literal}).asBool().toList()")).map(|v| v.join(","));
    let snake = run(&format!("g.inject({literal}).as_bool().toList()")).map(|v| v.join(","));
    assert_eq!(camel.as_ref().ok(), snake.as_ref().ok(), "{literal}");
    assert_eq!(camel.is_err(), snake.is_err(), "{literal}");
    camel
}

fn int_of(literal: &str, token: &str) -> Result<String, String> {
    let camel = run(&format!(
        "g.inject({literal}).asNumber(GType.{token}).toList()"
    ))
    .map(|v| v.join(","));
    let snake = run(&format!(
        "g.inject({literal}).as_number(GType.{token}).toList()"
    ))
    .map(|v| v.join(","));
    assert_eq!(camel.as_ref().ok(), snake.as_ref().ok(), "{literal}");
    assert_eq!(camel.is_err(), snake.is_err(), "{literal}");
    camel
}

#[test]
fn as_bool_numbers() {
    for (literal, want) in [
        ("1", "true"),
        ("0", "false"),
        ("-1", "true"),
        ("2", "true"),
        ("3.14", "true"),
        ("-0.0", "false"),
        ("0.0", "false"),
        ("0.0 / 0.0", "false"),
        ("1.0 / 0.0", "true"),
        ("-1.0 / 0.0", "true"),
    ] {
        assert_eq!(bool_of(literal).as_deref(), Ok(want), "{literal}");
    }
}

#[test]
fn as_bool_texts() {
    for (literal, want) in [
        (r#""true""#, "true"),
        (r#""tRUe""#, "true"),
        (r#""FALSE""#, "false"),
        (r#""False""#, "false"),
        ("true", "true"),
        ("false", "false"),
    ] {
        assert_eq!(bool_of(literal).as_deref(), Ok(want), "{literal}");
    }
    for literal in [r#""hello""#, r#""1""#, r#""""#, r#"" true""#, "[1, 2]"] {
        assert!(bool_of(literal).is_err(), "{literal} must fail");
    }
}

#[test]
fn as_bool_of_an_out_degree() {
    assert_eq!(
        ok("g.V().local(__.outE().count()).asBool().toList()"),
        ["true", "false", "false", "true", "false", "true"]
    );
}

#[test]
fn as_bool_of_a_stored_text_names_its_origin() {
    let err = run(r#"g.V().hasLabel("person").values("name").asBool().toList()"#).unwrap_err();
    assert!(err.contains("marko") || err.contains("name"), "{err}");
}

#[test]
fn as_bool_policies_handle_a_bad_text() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();
    let skipped = locked
        .traversal()
        .inject(vec!["hello", "true"])
        .as_bool_with(CastPolicy::Skip)
        .to_list()
        .unwrap();
    assert_eq!(skipped, vec![ElementProperty::from(true)]);
    let defaulted = locked
        .traversal()
        .inject(vec!["hello"])
        .as_bool_with(CastPolicy::Default(ElementProperty::from(false)))
        .to_list()
        .unwrap();
    assert_eq!(defaulted, vec![ElementProperty::from(false)]);
    let rust = locked
        .traversal()
        .inject(vec![
            ElementProperty::from(-1i64),
            ElementProperty::from(0i64),
        ])
        .as_bool()
        .to_list()
        .unwrap();
    assert_eq!(
        rust,
        vec![ElementProperty::from(true), ElementProperty::from(false)]
    );
}

#[test]
fn as_number_int_truncates_a_float() {
    for token in ["INT", "LONG"] {
        for (literal, want) in [
            ("5.43", "5"),
            ("5.67", "5"),
            ("-5.67", "-5"),
            ("5", "5"),
            (r#""5""#, "5"),
            ("true", "1"),
            ("0.9", "0"),
        ] {
            assert_eq!(
                int_of(literal, token).as_deref(),
                Ok(want),
                "{literal} {token}"
            );
        }
        for literal in [
            r#""5.7""#,
            "0.0 / 0.0",
            "1.0 / 0.0",
            "-1.0 / 0.0",
            "1e300",
            "-1e300",
        ] {
            assert!(
                int_of(literal, token).is_err(),
                "{literal} {token} must fail"
            );
        }
    }
}

#[test]
fn as_number_other_targets_are_unchanged() {
    assert_eq!(ok("g.inject(5).asNumber(GType.DOUBLE).toList()"), ["5.0"]);
    assert_eq!(ok("g.inject(5.5).asNumber().toList()"), ["5.5"]);
    assert_eq!(ok("g.inject(5).asNumber().toList()"), ["5"]);
}

#[test]
fn as_number_int_keeps_null() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();
    let got = locked
        .traversal()
        .inject(vec![ElementProperty::Null])
        .as_number(GType::LONG)
        .to_list()
        .unwrap();
    assert_eq!(got, vec![ElementProperty::Null]);
}

#[test]
fn the_strict_cast_extension_and_matrix_stay_strict() {
    assert!(run("g.inject(2).cast(GType.BOOLEAN).toList()").is_err());
    assert!(run(r#"g.inject("TRUE").cast(GType.BOOLEAN).toList()"#).is_err());
    assert!(run("g.inject(5.43).cast(GType.INT).toList()").is_err());
    assert_eq!(ok("g.inject(5.0).cast(GType.INT).toList()"), ["5"]);
}

#[test]
fn profile_names_stay_as_written() {
    let text = |script: &str| {
        eval_value(modern(), script)
            .unwrap_or_else(|e| panic!("{script}: {e}"))
            .to_string()
    };
    let boolean = text("g.inject(1).asBool().profile().to_string()");
    assert!(boolean.contains("as_bool()"), "{boolean}");
    let int = text("g.inject(1.5).asNumber(GType.INT).profile().to_string()");
    assert!(int.contains("as_number(GType.LONG)"), "{int}");
}
