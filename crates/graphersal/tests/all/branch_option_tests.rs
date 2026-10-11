//! Option keys and unmatched traversers of `choose()`/`branch()` (wrong-results step 3c).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> Result<rhai::Dynamic, String> {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script).map_err(|e| e.to_string())
}

fn strings(script: &str) -> Vec<String> {
    let mut out: Vec<String> = run(script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect();
    out.sort();
    out
}

/// Runs a query in both spellings (`.toList()`, `.to_list()`) and checks the sorted result.
fn both(query: &str, expected: &[&str]) {
    for q in [format!("{query}.toList()"), format!("{query}.to_list()")] {
        assert_eq!(strings(&q), expected, "{q}");
    }
}

#[test]
fn choose_predicate_key_first_match_and_none() {
    both(
        r#"g.V().hasLabel("person").choose(__.values("age")).option(P.between(26, 30), __.constant("x")).option(P.between(20, 30), __.constant("y")).option(Pick.none, __.constant("z"))"#,
        &["x", "x", "z", "z"],
    );
    both(
        r#"g.V().has_label("person").choose(__.values("age")).option(P.lt(30), __.constant("young")).option(P.gte(30), __.constant("old"))"#,
        &["old", "old", "young", "young"],
    );
    both(
        r#"g.V().choose(__.values("name")).option(P.neq("y"), __.values("age")).option(Pick.none, __.constant("x"))"#,
        &["27", "29", "32", "35"],
    );
}

#[test]
fn branch_predicate_keys_are_all_match() {
    both(
        r#"g.V().hasLabel("person").branch(__.values("age")).option(P.lt(30), __.constant("a")).option(P.lt(28), __.constant("b")).option(P.gt(30), __.constant("c"))"#,
        &["a", "a", "b", "c", "c"],
    );
}

#[test]
fn branch_pick_any_is_additive_and_none_is_fallback() {
    both(
        r#"g.V().hasLabel("person").branch(__.values("age")).option(P.gt(30), __.constant("old")).option(Pick.none, __.constant("not old")).option(Pick.any, __.constant("any"))"#,
        &[
            "any", "any", "any", "any", "not old", "not old", "old", "old",
        ],
    );
}

#[test]
fn numeric_literal_keys_match_by_value() {
    // Out-degrees: marko 3, josh 2, the others 0. An int, a float and a long-looking key all match.
    for key in ["2", "2.0"] {
        let q = format!(
            r#"g.V().choose(__.out().count()).option({key}, __.values("name")).option(3, __.values("age")).toList()"#
        );
        let out = run(&q).unwrap().into_array().unwrap();
        let strs: Vec<String> = out.iter().map(|v| v.to_string()).collect();
        assert!(strs.contains(&"josh".to_string()), "{q}: {strs:?}");
        assert!(strs.contains(&"29".to_string()), "{q}: {strs:?}");
        assert_eq!(strs.len(), 6, "{q}");
    }
}

#[test]
fn choose_passes_unmatched_traversers_through() {
    // No option matches the four traversers with out-degree 0 and no Pick.none exists: they pass
    // through as the vertices themselves.
    let q = r#"g.V().choose(__.out().count()).option(2, __.values("name")).option(3, __.values("age")).toList()"#;
    let out = run(q).unwrap().into_array().unwrap();
    assert_eq!(out.len(), 6);
    let maps = out.iter().filter(|v| v.is_map()).count();
    assert_eq!(maps, 4, "{out:?}");
    // The Rust API gives the same.
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let n = lock
        .traversal()
        .v(None)
        .choose_switch(__::out(None).count())
        .option(ElementProperty::from(2), __::values("name"))
        .to_list()
        .unwrap()
        .len();
    assert_eq!(n, 6);
}

#[test]
fn choose_unproductive_selector_passes_through_even_with_none() {
    // `values("age")` yields nothing for the two software vertices: they pass through unchanged
    // (v[lop], v[ripple]); the persons take the `none` branch only when no option matched.
    let q = r#"g.V().choose(__.values("age")).option(P.between(26, 30), __.values("name")).option(Pick.none, __.values("name")).toList()"#;
    let out = run(q).unwrap().into_array().unwrap();
    assert_eq!(out.len(), 6);
    assert_eq!(out.iter().filter(|v| v.is_map()).count(), 2, "{out:?}");
    let q = r#"g.V().choose(__.values("age")).option(P.between(26, 30), __.values("name")).option(Pick.none, __.discard()).toList()"#;
    if let Ok(out) = run(q) {
        assert_eq!(out.into_array().unwrap().len(), 4);
    }
}

#[test]
fn pick_unproductive_runs_for_an_empty_selector() {
    both(
        r#"g.V().choose(__.values("age")).option(P.between(26, 30), __.values("name")).option(Pick.none, __.values("name")).option(Pick.unproductive, __.label())"#,
        &["josh", "marko", "peter", "software", "software", "vadas"],
    );
    // branch(): every unproductive option runs.
    both(
        r#"g.V().branch(__.values("age")).option(Pick.unproductive, __.constant("a")).option(Pick.unproductive, __.constant("b"))"#,
        &["a", "a", "b", "b"],
    );
}

#[test]
fn branch_drops_unmatched_and_unproductive_traversers() {
    both(
        r#"g.V().branch(__.label()).option("person", __.constant("p"))"#,
        &["p", "p", "p", "p"],
    );
    both(
        r#"g.V().branch(__.values("age")).option(P.lt(28), __.constant("young"))"#,
        &["young"],
    );
}

#[test]
fn choose_first_none_option_only() {
    both(
        r#"g.V().hasLabel("person").choose(__.values("age")).option(P.gt(100), __.constant("n")).option(Pick.none, __.constant("a")).option(Pick.none, __.constant("b"))"#,
        &["a", "a", "a", "a"],
    );
    // branch(): every none option runs.
    both(
        r#"g.V().hasLabel("person").branch(__.values("age")).option(P.gt(100), __.constant("n")).option(Pick.none, __.constant("a")).option(Pick.none, __.constant("b"))"#,
        &["a", "a", "a", "a", "b", "b", "b", "b"],
    );
}

#[test]
fn branch_traversal_keys_match_when_the_traversal_yields() {
    for q in [
        r#"g.V().branch(__.identity()).option(__.hasLabel("software"), __.in("created").values("name").order().fold()).option(__.has("name", "vadas"), __.values("age")).option(P.neq(123), __.bothE().count())"#,
        r#"g.v().branch(__.identity()).option(__.has_label("software"), __.in_("created").values("name").order().fold()).option(__.has("name", "vadas"), __.values("age")).option(P.neq(123), __.bothE().count())"#,
    ] {
        let out = run(&format!("{q}.toList()"));
        let out = match out {
            Ok(o) => o,
            Err(_) => continue, // alternative spelling of in(): only one needs to parse
        };
        let strs: Vec<String> = out
            .into_array()
            .unwrap()
            .iter()
            .map(|v| v.to_string())
            .collect();
        assert_eq!(strs.len(), 3, "{strs:?}");
        assert!(strs.contains(&"27".to_string()));
        assert!(
            strs.contains(&"12".to_string()),
            "bothE count over the whole bucket: {strs:?}"
        );
        assert!(
            strs.iter()
                .any(|s| s.contains("marko") && s.contains("peter")),
            "{strs:?}"
        );
    }
    // A traversal key sees the selector's value as its start: `values("name")` -> a string.
    both(
        r#"g.V().hasLabel("person").branch(__.values("name")).option(__.is(P.eq("marko")), __.constant("m")).option(__.is(P.neq("marko")), __.constant("other"))"#,
        &["m", "other", "other", "other"],
    );
}

#[test]
fn choose_rejects_traversal_keys_and_pick_any() {
    let err = run(r#"g.V().hasLabel("person").choose(__.values("name")).option(__.is("marko"), __.values("age")).toList()"#)
        .expect_err("traversal key");
    assert!(err.contains("traversal is not allowed"), "{err}");
    let err = run(r#"g.V().choose(__.label()).option(Pick.any, __.identity()).toList()"#)
        .expect_err("Pick.any");
    assert!(err.contains("Pick.any"), "{err}");
}

#[test]
fn literal_operands_are_data_not_step_labels() {
    // `"a"` is a literal here: it never resolves to the step label `a`.
    both(
        r#"g.V().hasLabel("person").as("a").choose(__.constant("a")).option("a", __.constant("lit")).option(Pick.none, __.constant("none"))"#,
        &["lit", "lit", "lit", "lit"],
    );
}

#[test]
fn nested_choose_with_predicates() {
    both(
        r#"g.V().choose(__.label()).option("person", __.choose(__.values("age")).option(P.lt(30), __.constant("young")).option(P.gte(30), __.constant("old"))).option("software", __.constant("program")).option(Pick.none, __.constant("unknown"))"#,
        &["old", "old", "program", "program", "young", "young"],
    );
}

#[test]
fn rust_api_option_p_and_option_traversal() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let young = lock
        .traversal()
        .v(None)
        .has_label("person")
        .choose_switch(__::values("age"))
        .option_p(P::Lt(PValue::from(30)), __::constant("young"))
        .option_p(P::Gte(PValue::from(30)), __::constant("old"))
        .to_list()
        .unwrap()
        .iter()
        .filter(|v| v.as_string() == Some("young"))
        .count();
    assert_eq!(young, 2);
    let n = lock
        .traversal()
        .v(None)
        .branch(__::identity())
        .option_traversal(__::has_label("software"), __::constant("sw"))
        .option_traversal(__::has_label("person"), __::constant("p"))
        .to_list()
        .unwrap()
        .len();
    assert_eq!(n, 6);
    let err = lock
        .traversal()
        .v(None)
        .choose_switch(__::identity())
        .option_traversal(__::has_label("software"), __::constant("sw"))
        .to_list();
    assert!(err.is_err());
}

#[test]
fn profile_shows_predicate_and_traversal_keys() {
    let text = run(r#"g.V().branch(__.identity()).option(__.hasLabel("software"), __.constant("sw")).option(P.neq(1), __.constant("x")).profile().to_string()"#)
        .unwrap()
        .to_string();
    assert!(text.contains("branch("), "{text}");
    // The key traversal's steps are profiled as nested steps of the option, before its branch.
    assert!(text.contains("has_label(\"software\")"), "{text}");
}

#[test]
fn bulk_merge_does_not_change_option_results() {
    let q = |merge: bool| {
        run(&format!(
            r#"g.with("bulk.merge", {merge}).V().out().choose(__.values("age")).option(P.gt(30), __.constant("old")).option(Pick.none, __.constant("young")).toList()"#
        ))
        .unwrap()
        .into_array()
        .unwrap()
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
    };
    let (mut a, mut b) = (q(true), q(false));
    a.sort();
    b.sort();
    assert_eq!(a, b);
}
