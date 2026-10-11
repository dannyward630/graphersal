//! `where(P).by()` and `select(Column.keys)` over a path.
//!
//! `where(P)` resolves a bare-string operand of every comparison (`eq neq lt lte gt gte`) as a
//! step label and applies its `by()` modulators as a ring (current object first, then each label
//! operand in textual order); `select(Column.keys)` of a path lists the labels of each position.
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
    let mut rendered: Vec<String> = items.iter().map(|v| format!("{v:?}")).collect();
    rendered.sort();
    Ok(rendered)
}

fn ok(script: &str) -> Vec<String> {
    run(script).unwrap_or_else(|e| panic!("{script}: {e}"))
}

/// Runs the snake_case and the camelCase spelling and asserts they agree.
fn both(snake: &str, camel: &str) -> Vec<String> {
    let a = ok(snake);
    let b = ok(camel);
    assert_eq!(a, b, "{snake}");
    a
}

fn rows(rows: &[&str]) -> Vec<String> {
    let mut rows: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
    rows.sort();
    rows
}

#[test]
fn where_gt_label_by_age_scenario() {
    let got = both(
        r#"g.v("1").as_("a").out().has("age").where(P.gt("a")).by("age").values("name").to_list()"#,
        r#"g.V("1").as("a").out().has("age").where(P.gt("a")).by("age").values("name").toList()"#,
    );
    assert_eq!(got, rows(&[r#""josh""#]));
}

#[test]
fn every_comparison_resolves_the_label_with_by() {
    // marko (29) -> vadas (27), josh (32)
    let q = |p: &str| {
        ok(&format!(
            r#"g.V("1").as("a").out().has("age").where({p}).by("age").values("name").toList()"#
        ))
    };
    assert_eq!(q(r#"P.lt("a")"#), rows(&[r#""vadas""#]));
    assert_eq!(q(r#"P.lte("a")"#), rows(&[r#""vadas""#]));
    assert_eq!(q(r#"P.gte("a")"#), rows(&[r#""josh""#]));
    assert_eq!(q(r#"P.eq("a")"#), rows(&[]));
    assert_eq!(q(r#"P.neq("a")"#), rows(&[r#""josh""#, r#""vadas""#]));
}

#[test]
fn ordering_resolves_a_label_without_by() {
    // 29 is greater than exactly one of the ages 29, 27, 32, 35 (27).
    let got = both(
        r#"g.v().values("age").as_("a").v("1").values("age").where(P.gt("a")).count().to_list()"#,
        r#"g.V().values("age").as("a").V("1").values("age").where(P.gt("a")).count().toList()"#,
    );
    assert_eq!(got, rows(&["1"]));
}

#[test]
fn a_vertex_without_the_property_is_dropped() {
    // `lop` has no age: an unproductive by() drops it (also under a not()).
    let got = ok(r#"g.V("1").as("a").out().where(P.gt("a")).by("age").values("name").toList()"#);
    assert_eq!(got, rows(&[r#""josh""#]));
    let got =
        ok(r#"g.V("1").as("a").out().where(P.not(P.gt("a"))).by("age").values("name").toList()"#);
    assert_eq!(got, rows(&[r#""vadas""#]));
}

#[test]
fn two_bys_split_the_current_object_and_the_operand() {
    let q = |bys: &str| {
        ok(&format!(
            r#"g.V("1").outE("knows").as("e").inV().where(P.gt("e")){bys}.values("name").toList()"#
        ))
    };
    // vertex age (27, 32) against edge weight (0.5, 1.0)
    assert_eq!(
        q(r#".by("age").by("weight")"#),
        rows(&[r#""josh""#, r#""vadas""#])
    );
    // the vertex has no weight
    assert_eq!(q(r#".by("weight").by("age")"#), rows(&[]));
    // one by() serves both sides: an edge has no age
    assert_eq!(q(r#".by("age")"#), rows(&[]));
}

#[test]
fn composite_predicates_take_ring_entries_in_textual_order() {
    let q = |bys: &str| {
        ok(&format!(
            r#"g.V().has("name", "vadas").as("a").V().has("name", "peter").as("b").V().has("age").where(P.gt("a").and(P.lt("b"))){bys}.values("name").toList()"#
        ))
    };
    // 27 < age < 35: marko, josh
    let both_ages = rows(&[r#""josh""#, r#""marko""#]);
    assert_eq!(q(r#".by("age")"#), both_ages);
    assert_eq!(q(r#".by("age").by("age").by("age")"#), both_ages);
    // operand a uses by #2, operand b uses by #3
    assert_eq!(q(r#".by("age").by("nosuch").by("age")"#), rows(&[]));
    assert_eq!(q(r#".by("age").by("age").by("nosuch")"#), rows(&[]));
}

#[test]
fn a_by_child_traversal_works_with_path_analysis_on_and_off() {
    let on = ok(
        r#"g.V("1").as("a").out("knows").where(P.lt("a")).by(__.out().count()).values("name").toList()"#,
    );
    let off = ok(
        r#"g.with("path.analysis", false).V("1").as("a").out("knows").where(P.lt("a")).by(__.out().count()).values("name").toList()"#,
    );
    // marko has 3 out edges; vadas 0, josh 2.
    assert_eq!(on, rows(&[r#""josh""#, r#""vadas""#]));
    assert_eq!(on, off);
    // a by(__.path()) child sees the projected object as a one-element path on either setting
    let q = |prefix: &str| {
        ok(&format!(
            r#"{prefix}V("1").as("a").out("knows").where(P.eq("a")).by(__.path().count(Scope.local)).values("name").toList()"#
        ))
    };
    assert_eq!(q("g."), rows(&[r#""josh""#, r#""vadas""#]));
    assert_eq!(q("g."), q(r#"g.with("path.analysis", false)."#));
}

#[test]
fn by_label_token() {
    let got = ok(r#"g.V("1").as("a").out().where(P.neq("a")).by(T.label).values("name").toList()"#);
    assert_eq!(got, rows(&[r#""lop""#]));
}

#[test]
fn unknown_labels_stay_literal() {
    // no label `x`, no side effect: the text "x" is compared with a number, incomparable
    assert_eq!(
        ok(r#"g.V().values("age").where(P.gt("x")).count().toList()"#),
        rows(&["0"])
    );
    // has() keeps literal strings even with a label of the same spelling
    let got = ok(r#"g.V().as("a").has("name", P.gt("a")).count().toList()"#);
    assert_eq!(got, rows(&["6"]));
}

#[test]
fn a_side_effect_scalar_still_resolves_after_the_label() {
    let got = ok(r#"g.withSideEffect("x", 29).V().values("age").where(P.gt("x")).toList()"#);
    assert_eq!(got, rows(&["32", "35"]));
    // the label wins over a side effect of the same name
    let got = ok(
        r#"g.withSideEffect("x", 100).V().values("age").as("x").V("1").values("age").where(P.gt("x")).count().toList()"#,
    );
    assert_eq!(got, rows(&["1"]));
}

#[test]
fn profile_shows_the_by_and_the_label_requirement() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let profile = lock
        .traversal()
        .v(None)
        .as_("a")
        .out(None)
        .where_p(P::Gt("a"))
        .by("age")
        .profile()
        .unwrap()
        .to_string();
    assert!(
        profile.contains(r#"where(P.gt("a")).by("age")"#),
        "{profile}"
    );
    assert!(profile.contains("[path: labels(a)]"), "{profile}");
}

#[test]
fn rust_api_where_by() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v("1")
        .as_("a")
        .out(None)
        .has_property("age")
        .where_p(P::Gt("a"))
        .by("age")
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(names, vec![ElementProperty::from("josh")]);
}

#[test]
fn select_keys_of_a_path_scenario() {
    let got = both(
        r#"g.v().as_("a", "b").out().as_("c").path().select(Column.keys).to_list()"#,
        r#"g.V().as("a", "b").out().as("c").path().select(Column.keys).toList()"#,
    );
    assert_eq!(got.len(), 6);
    assert!(
        got.iter().all(|row| row == r#"[["a", "b"], ["c"]]"#),
        "{got:?}"
    );
}

#[test]
fn select_keys_of_a_path_with_unlabelled_and_repeated_labels() {
    let got = ok(r#"g.V("1").out().path().select(Column.keys).toList()"#);
    assert_eq!(got.len(), 3);
    assert!(got.iter().all(|row| row == "[[], []]"), "{got:?}");
    let got = ok(r#"g.V("1").as("a").out().as("a").path().select(Column.keys).toList()"#);
    assert!(got.iter().all(|row| row == r#"[["a"], ["a"]]"#), "{got:?}");
    let got = ok(r#"g.V("1").as("a").out().out().as("b").path().select(Column.keys).toList()"#);
    assert!(
        got.iter().all(|row| row == r#"[["a"], [], ["b"]]"#),
        "{got:?}"
    );
}

#[test]
fn select_keys_spellings() {
    let expected = rows(&[
        r#"[["a"], ["b"]]"#,
        r#"[["a"], ["b"]]"#,
        r#"[["a"], ["b"]]"#,
    ]);
    for call in [
        "select(Column.keys)",
        "select(Column::keys)",
        "select(keys)",
    ] {
        let got = ok(&format!(
            r#"g.V("1").as("a").out().as("b").path().{call}.toList()"#
        ));
        assert_eq!(got, expected, "{call}");
    }
}

#[test]
fn select_keys_of_a_path_is_independent_of_path_analysis() {
    let on = ok(r#"g.V().as("a").out().path().select(Column.keys).toList()"#);
    let off = ok(
        r#"g.with("path.analysis", false).V().as("a").out().path().select(Column.keys).toList()"#,
    );
    assert_eq!(on.len(), 6);
    assert!(on.iter().all(|row| row == r#"[["a"], []]"#), "{on:?}");
    assert_eq!(on, off);
}

#[test]
fn select_values_of_a_path_is_its_objects() {
    let got = ok(r#"g.V("1").as("a").out().path().by("name").select(Column.values).toList()"#);
    assert_eq!(got.len(), 3);
    assert!(
        got.iter().all(|row| row.starts_with(r#"["marko", "#)),
        "{got:?}"
    );
}

#[test]
fn by_column_keys_of_a_path_in_order() {
    let got =
        ok(r#"g.V("1").as("a").out().as("b").path().order().by(Column.keys).count().toList()"#);
    assert_eq!(got, rows(&["3"]));
}
