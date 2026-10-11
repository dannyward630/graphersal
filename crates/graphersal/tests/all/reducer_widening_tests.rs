//! `min()`/`max()`/`sum()` (global and `Scope.local`) promote an int + float mix to a float, and
//! `min()`/`max()` compare strings, booleans and UUIDs (TinkerPop `NumberHelper` / `Comparable`).
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

fn floats(script: &str) -> Vec<f64> {
    ok(script)
        .iter()
        .map(|s| s.parse::<f64>().unwrap_or_else(|_| panic!("{script}: {s}")))
        .collect()
}

fn close(mut got: Vec<f64>, mut want: Vec<f64>) {
    got.sort_by(|a, b| a.total_cmp(b));
    want.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
    for (g, w) in got.iter().zip(&want) {
        assert!((g - w).abs() < 1e-9, "{got:?} vs {want:?}");
    }
}

const MIX: &str = r#"g.V().local(__.union(__.values("age"), __.outE().values("weight")).fold())"#;

#[test]
fn mixed_int_and_float_lists_promote_to_float() {
    // marko [29, 0.5, 1.0, 0.4], vadas [27], josh [32, 1.0, 0.4], peter [35, 0.2]
    let max = ok(&format!("{MIX}.max(Scope.local).toList()"));
    assert_eq!(max.len(), 4);
    assert!(max.contains(&"29.0".to_string()), "{max:?}");
    assert!(max.contains(&"27".to_string()), "{max:?}");
    assert!(max.contains(&"32.0".to_string()), "{max:?}");
    assert!(max.contains(&"35.0".to_string()), "{max:?}");
    let min = ok(&format!("{MIX}.min(Scope.local).toList()"));
    assert!(min.contains(&"27".to_string()), "{min:?}");
    assert_eq!(
        min.iter().filter(|s| s.starts_with("0.")).count(),
        3,
        "{min:?}"
    );
    close(
        floats(&format!("{MIX}.sum(Scope.local).toList()"))
            .into_iter()
            .filter(|v| *v != 27.0)
            .collect(),
        vec![30.9, 33.4, 35.2],
    );
    assert!(ok(&format!("{MIX}.sum(Scope.local).toList()")).contains(&"27".to_string()));
}

#[test]
fn snake_case_spellings_agree() {
    let camel = ok(r#"g.V().values("name").max().toList()"#);
    let snake = ok(r#"g.v().values("name").max().to_list()"#);
    assert_eq!(camel, snake);
    assert_eq!(
        ok(r#"g.V().values("name").fold().min(Scope.local).toList()"#),
        ok(r#"g.v().values("name").fold().min(Scope::local).to_list()"#)
    );
}

#[test]
fn promotion_is_order_independent() {
    assert_eq!(ok("g.inject(3, 2.5, 1).min().toList()"), vec!["1.0"]);
    assert_eq!(ok("g.inject(1, 2.5, 3).min().toList()"), vec!["1.0"]);
    assert_eq!(ok("g.inject(2.5, 3).max().toList()"), vec!["3.0"]);
    assert_eq!(ok("g.inject(3, 2.5).max().toList()"), vec!["3.0"]);
    assert_eq!(ok("g.inject(1, 2).max().toList()"), vec!["2"]);
    assert_eq!(ok("g.inject(1, 2.5).sum().toList()"), vec!["3.5"]);
    assert_eq!(ok("g.inject(2.5, 1).sum().toList()"), vec!["3.5"]);
    assert!(run("g.inject(9223372036854775807, 1.0).sum().toList()").is_ok());
    assert!(run("g.inject(9223372036854775807, 1).sum().toList()").is_err());
}

#[test]
fn sum_over_bulked_traversers_is_merge_independent() {
    let on = ok(r#"g.V().out().values("age").sum().toList()"#);
    let off = ok(r#"g.with("bulk.merge", false).V().out().values("age").sum().toList()"#);
    assert_eq!(on, off);
    let on = ok(r#"g.V().out().values("weight").sum().toList()"#);
    let off = ok(r#"g.with("bulk.merge", false).V().out().values("weight").sum().toList()"#);
    assert_eq!(on, off);
}

#[test]
fn strings_booleans_and_uuids_are_comparable() {
    assert_eq!(
        ok(r#"g.V().values("name").max().toList()"#),
        vec![r#""vadas""#]
    );
    assert_eq!(
        ok(r#"g.V().values("name").min().toList()"#),
        vec![r#""josh""#]
    );
    assert_eq!(
        ok(r#"g.V().values("name").fold().max(Scope.local).toList()"#),
        vec![r#""vadas""#]
    );
    assert_eq!(
        ok(r#"g.V().values("name").fold().min(Scope.local).toList()"#),
        vec![r#""josh""#]
    );
    assert_eq!(ok(r#"g.inject("b", "a").max().toList()"#), vec![r#""b""#]);
    assert_eq!(ok(r#"g.inject("b", "a").min().toList()"#), vec![r#""a""#]);
    assert_eq!(ok("g.inject(false, true).max().toList()"), vec!["true"]);
    assert_eq!(ok("g.inject(false, true).min().toList()"), vec!["false"]);
    assert_eq!(ok(r#"g.inject("a", ()).max().toList()"#), vec![r#""a""#]);
    let a = r#"UUID.fromString("00000000-0000-0000-0000-000000000001")"#;
    let b = r#"UUID.fromString("00000000-0000-0000-0000-000000000002")"#;
    assert_eq!(ok(&format!("g.inject({a}, {b}).max().toList()")).len(), 1);
    assert_eq!(
        ok(&format!("g.inject({b}, {a}).max().toList()")),
        ok(&format!("g.inject({b}).toList()"))
    );
    assert_eq!(
        ok(&format!("g.inject({b}, {a}).min().toList()")),
        ok(&format!("g.inject({a}).toList()"))
    );
}

#[test]
fn empty_and_all_null_streams_emit_nothing() {
    assert!(ok(r#"g.V().hasLabel("nope").values("name").max().toList()"#).is_empty());
    assert!(ok("g.inject((), ()).min().toList()").is_empty());
    assert!(ok(r#"g.V().values("nope").fold().max(Scope.local).toList()"#).is_empty());
}

#[test]
fn incomparable_kinds_fail() {
    for script in [
        r#"g.inject(1, "a").max().toList()"#,
        r#"g.inject("a", 1).min().toList()"#,
        "g.inject(true, 1).max().toList()",
        r#"g.inject(1, true).max().toList()"#,
        "g.V().max().toList()",
        "g.V().fold().min(Scope.local).toList()",
        r#"g.V().values("name").fold().sum(Scope.local).toList()"#,
        r#"g.V().values("name").sum().toList()"#,
        r#"g.V().values("name").mean().toList()"#,
    ] {
        assert!(run(script).is_err(), "{script} must fail");
    }
}

#[test]
fn the_error_names_both_kinds() {
    let err = run(r#"g.inject(1, "a").max().toList()"#).expect_err("fails");
    assert!(err.contains("number") && err.contains("string"), "{err}");
}

#[test]
fn profile_keeps_the_step_names() {
    let text = format!(
        "{:?}",
        eval_value(
            modern(),
            r#"g.V().values("name").max().profile().to_string()"#
        )
        .unwrap()
    );
    assert!(text.contains("max()"), "{text}");
    let text = format!(
        "{:?}",
        eval_value(
            modern(),
            r#"g.V().values("name").fold().max(Scope.local).profile().to_string()"#
        )
        .unwrap()
    );
    assert!(text.contains("max(Scope.local)"), "{text}");
}

#[test]
fn path_analysis_does_not_change_results() {
    assert_eq!(
        ok(r#"g.with("path.analysis", false).V().values("name").max().toList()"#),
        vec![r#""vadas""#]
    );
}

#[test]
fn rust_api_matches() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let max = lock
        .traversal()
        .v(None)
        .values("name")
        .max()
        .to_list()
        .unwrap();
    assert_eq!(max, vec![ElementProperty::String("vadas".into())]);
    let min = lock
        .traversal()
        .v(None)
        .values("name")
        .fold()
        .min_scoped(Scope::Local)
        .to_list()
        .unwrap();
    assert_eq!(min, vec![ElementProperty::String("josh".into())]);
}
