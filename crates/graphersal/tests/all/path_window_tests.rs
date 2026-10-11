//! `path()`, `simplePath()` and `cyclicPath()` take `from()`, `to()` and `by()` (TinkerPop's
//! `PathProcessor` steps): a window onto the path between two labelled positions, projected
//! through the `by()` ring before the check.
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

fn names(rows: &[&str]) -> Vec<String> {
    let mut rows: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
    rows.sort();
    rows
}

#[test]
fn path_from_to_by_scenario() {
    let got = both(
        r#"g.v().as_("a").out().as_("b").out().as_("c").path().from("b").to("c").by("name").to_list()"#,
        r#"g.V().as("a").out().as("b").out().as("c").path().from("b").to("c").by("name").toList()"#,
    );
    assert_eq!(got, names(&[r#"["josh", "lop"]"#, r#"["josh", "ripple"]"#]));
}

#[test]
fn simple_path_by_label_from_to_scenario() {
    let got = both(
        r#"g.v().as_("a").out().as_("b").out().as_("c").simple_path().by(T.label).from("b").to("c").path().by("name").to_list()"#,
        r#"g.V().as("a").out().as("b").out().as("c").simplePath().by(T.label).from("b").to("c").path().by("name").toList()"#,
    );
    assert_eq!(
        got,
        names(&[
            r#"["marko", "josh", "lop"]"#,
            r#"["marko", "josh", "ripple"]"#
        ])
    );
}

#[test]
fn simple_path_then_path_by_age_from_a_to_b_scenario() {
    // `by`, `from`, `to` follow `path()` in that order here.
    let got = both(
        r#"g.v().both().as_("a").both().as_("b").simple_path().path().by("age").from("a").to("b").to_list()"#,
        r#"g.V().both().as("a").both().as("b").simplePath().path().by("age").from("a").to("b").toList()"#,
    );
    assert_eq!(got.len(), 6, "{got:?}");
    assert!(got.contains(&"[29, 32]".to_string()), "{got:?}");
    assert!(got.contains(&"[29, 27]".to_string()), "{got:?}");
}

#[test]
fn cyclic_path_from_to_scenario_is_empty() {
    let got = both(
        r#"g.v("1").as_("a").out("created").as_("b").in_("created").as_("c").cyclic_path().from("a").to("b").path().to_list()"#,
        r#"g.V("1").as("a").out("created").as("b").in("created").as("c").cyclicPath().from("a").to("b").path().toList()"#,
    );
    assert!(got.is_empty(), "{got:?}");
}

#[test]
fn cyclic_path_by_age_scenario() {
    let got = both(
        r#"g.v("1").both().both().cyclic_path().by("age").to_list()"#,
        r#"g.V("1").both().both().cyclicPath().by("age").toList()"#,
    );
    // [marko, vadas, marko] -> [29, 27, 29] and [marko, josh, marko] -> [29, 32, 29] are cyclic;
    // every path through a software vertex has no age and is dropped.
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.iter().all(|row| row.contains("marko")), "{got:?}");
}

#[test]
fn a_one_sided_window_runs_to_the_end_or_from_the_start() {
    assert_eq!(
        ok(r#"g.V("1").as("a").out().as("b").out().path().from("b").by("name").to_list()"#),
        names(&[r#"["josh", "lop"]"#, r#"["josh", "ripple"]"#])
    );
    assert_eq!(
        ok(r#"g.V("1").out().as("b").out().path().to("b").by("name").to_list()"#),
        names(&[r#"["marko", "josh"]"#, r#"["marko", "josh"]"#])
    );
}

#[test]
fn from_equal_to_is_one_object_and_two_labels_may_share_a_position() {
    assert_eq!(
        ok(r#"g.V("1").as("a").out().as("b").out().path().from("b").to("b").by("name").to_list()"#),
        names(&[r#"["josh"]"#, r#"["josh"]"#])
    );
    let same = ok(r#"g.V("1").as("a","b").out().path().from("a").to("b").by("name").to_list()"#);
    assert_eq!(
        same,
        names(&[r#"["marko"]"#, r#"["marko"]"#, r#"["marko"]"#])
    );
}

/// TinkerPop's `Path.subPath` takes the LAST position carrying each label, independently.
#[test]
fn a_repeated_label_resolves_to_its_last_position_for_from_and_to() {
    assert_eq!(
        ok(
            r#"g.V("1").as("a").out().as("a").out().as("b").path().from("a").to("b").by("name").to_list()"#
        ),
        names(&[r#"["josh", "lop"]"#, r#"["josh", "ripple"]"#])
    );
    assert_eq!(
        ok(
            r#"g.V("1").as("a").out().as("b").out().as("b").path().from("a").to("b").by("name").to_list()"#
        ),
        names(&[
            r#"["marko", "josh", "lop"]"#,
            r#"["marko", "josh", "ripple"]"#
        ])
    );
}

#[test]
fn a_to_before_from_and_an_absent_label_are_errors_a_second_from_replaces_the_first() {
    let err = run(r#"g.V("1").as("a").out().as("b").out().path().from("b").to("a").to_list()"#)
        .expect_err("to before from");
    assert!(err.contains("comes after"), "{err}");
    let err = run(r#"g.V("1").as("a").out().path().from("zzz").to_list()"#).expect_err("absent");
    assert!(err.contains("zzz"), "{err}");
    let err = run(r#"g.V("1").as("a").out().path().to("zzz").to_list()"#).expect_err("absent");
    assert!(err.contains("zzz"), "{err}");
    assert_eq!(
        ok(r#"g.V("1").as("a").out().as("b").path().from("a").from("b").by("name").to_list()"#),
        names(&[r#"["josh"]"#, r#"["lop"]"#, r#"["vadas"]"#])
    );
}

#[test]
fn a_label_list_is_an_invalid_modulator() {
    for query in [
        r#"g.V("1").as("a").out().path().from(["a", "b"]).to_list()"#,
        r#"g.V("1").as("a").out().simplePath().to(["a", "b"]).to_list()"#,
    ] {
        let err = run(query).expect_err(query);
        assert!(
            err.contains("takes exactly one step label"),
            "{query}: {err}"
        );
    }
}

#[test]
fn the_by_ring_restarts_at_the_window_start() {
    // window [josh, lop]: by("name") on josh, by("lang") on lop; a third by cycles back.
    assert_eq!(
        ok(
            r#"g.V("1").as("a").out().as("b").out().path().from("b").by("name").by("lang").to_list()"#
        ),
        names(&[r#"["josh", "java"]"#, r#"["josh", "java"]"#])
    );
    // without a window the ring starts at the first object: marko has no lang -> dropped.
    assert!(
        ok(r#"g.V("1").as("a").out().out().path().by("lang").by("name").to_list()"#).is_empty()
    );
}

#[test]
fn a_by_without_a_value_drops_the_traverser_for_both_filters() {
    // v[1].out() = lop (no age), josh, vadas
    let simple = ok(r#"g.V("1").out().simplePath().by("age").values("name").to_list()"#);
    assert_eq!(simple, names(&[r#""josh""#, r#""vadas""#]));
    let cyclic = ok(r#"g.V("1").out().cyclicPath().by("age").values("name").to_list()"#);
    assert!(cyclic.is_empty(), "{cyclic:?}");
    // identical ages after the projection make a path cyclic that was simple by identity.
    assert_eq!(
        ok(r#"g.V("1").out().in("knows").cyclicPath().by(T.label).count().to_list()"#),
        vec!["2".to_string()]
    );
}

#[test]
fn a_nested_by_traversal_works_with_path_analysis_on_and_off() {
    let on = ok(r#"g.V("1").out().simplePath().by(__.out().count()).values("name").to_list()"#);
    let off = ok(
        r#"g.with("path.analysis", false).V("1").out().simplePath().by(__.out().count()).values("name").to_list()"#,
    );
    assert_eq!(on, off);
    assert_eq!(on.len(), 3);
    // by(__.path()) sees the one-position path of the object it projects.
    let child = ok(r#"g.V("1").out().simplePath().by(__.path()).values("name").to_list()"#);
    assert_eq!(child.len(), 3, "{child:?}");
}

#[test]
fn the_filters_work_inside_repeat_and_path_inside_local() {
    assert_eq!(
        ok(r#"g.V("1").repeat(__.out().simplePath()).times(2).values("name").to_list()"#),
        names(&[r#""lop""#, r#""ripple""#])
    );
    let windowed = ok(
        r#"g.V("1").as("a").repeat(__.out().simple_path().from("a")).times(2).path().by("name").to_list()"#,
    );
    assert_eq!(
        windowed,
        names(&[
            r#"["marko", "josh", "lop"]"#,
            r#"["marko", "josh", "ripple"]"#
        ])
    );
    let local = ok(r#"g.V().as("a").out().local(__.path().from("a")).count().to_list()"#);
    assert_eq!(local, vec!["6".to_string()]);
}

#[test]
fn the_rust_api_takes_the_modulators() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let rows = lock
        .traversal()
        .v("1")
        .as_("a")
        .out(None)
        .as_("b")
        .out(None)
        .path()
        .from("b")
        .by("name")
        .to_list()
        .unwrap();
    assert_eq!(rows.len(), 2);
    let simple = lock
        .traversal()
        .v("1")
        .both(None)
        .both(None)
        .simple_path()
        .by("name")
        .count()
        .next()
        .unwrap();
    assert_eq!(simple.and_then(|c| c.as_i64()), Some(4));
}

#[test]
fn the_plain_forms_are_unchanged_and_profile_shows_the_modulators_and_path_records() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let plain = lock
        .traversal()
        .v("1")
        .both(None)
        .both(None)
        .simple_path()
        .count()
        .next()
        .unwrap();
    assert_eq!(plain.and_then(|c| c.as_i64()), Some(4));
    let cyclic = lock
        .traversal()
        .v("1")
        .both(None)
        .both(None)
        .cyclic_path()
        .count()
        .next()
        .unwrap();
    assert_eq!(cyclic.and_then(|c| c.as_i64()), Some(3));

    let profile = lock
        .traversal()
        .v(None)
        .as_("a")
        .out(None)
        .as_("b")
        .out(None)
        .simple_path()
        .from("a")
        .to("b")
        .by("name")
        .path()
        .from("b")
        .profile()
        .unwrap()
        .to_string();
    assert!(
        profile.contains(r#"simple_path().from("a").to("b").by("#),
        "{profile}"
    );
    assert!(profile.contains("[path: full]"), "{profile}");
    assert!(profile.contains(r#"path().from("b")"#), "{profile}");
}

#[test]
fn a_rejected_modulator_inside_a_child_that_never_runs_still_fails_at_plan_time() {
    let err = run(r#"g.V().limit(0).where(__.out().path().from(["a","b"])).to_list()"#)
        .expect_err("plan-time rejection");
    assert!(err.contains("takes exactly one step label"), "{err}");
}
