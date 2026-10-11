//! `hasId(P)`, `hasLabel(P)`, `hasKey(P)` evaluate the predicate (never stringify it), and
//! `where(P.eq/neq("key"))` resolves a bare string through path labels, then side effects.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> rhai::Dynamic {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn count(script: &str) -> i64 {
    run(script)
        .as_int()
        .unwrap_or_else(|_| panic!("{script}: not an int"))
}

#[test]
fn has_id_takes_a_predicate() {
    assert_eq!(count(r#"g.v().has_id(P.neq("1")).count().next()"#), 5);
    assert_eq!(
        count(r#"g.v().has_id(P.within(["1", "2"])).count().next()"#),
        2
    );
    assert_eq!(
        count(r#"g.v().has_id(P.without(["1", "2"])).count().next()"#),
        4
    );
    assert_eq!(count("g.v().has_id(P.without([])).count().next()"), 6);
    assert_eq!(count(r#"g.v().has_id(P.gt("4")).count().next()"#), 2);
    assert_eq!(count(r#"g.v().has_id(P.neq("1")).count().next()"#), 5);
    assert_eq!(count(r#"g.v().in().has_id(P.neq("1")).count().next()"#), 3);
    // `has(T.id, P)` routes to the same step.
    assert_eq!(count(r#"g.v().has(T.id, P.neq("1")).count().next()"#), 5);
    assert_eq!(count(r#"g.v().has(T.id, P.eq("1")).count().next()"#), 1);
}

#[test]
fn has_id_predicate_on_edges() {
    assert_eq!(count(r#"g.e().has_id(P.neq("1")).count().next()"#), 5);
}

#[test]
fn a_step_label_never_shadows_an_id_operand() {
    // The label "1" does not make `neq("1")` compare against the labelled vertex.
    assert_eq!(
        count(r#"g.v("1").as("1").out().has_id(P.neq("1")).count().next()"#),
        3
    );
}

#[test]
fn has_label_takes_a_predicate() {
    assert_eq!(
        count(r#"g.v().has_label(P.neq("person")).count().next()"#),
        2
    );
    assert_eq!(
        count(r#"g.v().has_label(P.within("person", "software")).count().next()"#),
        6
    );
    assert_eq!(
        count(r#"g.v().has_label(P.without(["person"])).count().next()"#),
        2
    );
    assert_eq!(
        count(r#"g.v().has_label(P.starting_with("per")).count().next()"#),
        4
    );
    assert_eq!(
        count(r#"g.e().has_label(P.neq("knows")).count().next()"#),
        4
    );
    assert_eq!(
        count(r#"g.v().has(T.label, P.neq("person")).count().next()"#),
        2
    );
}

#[test]
fn has_key_takes_a_predicate() {
    assert_eq!(
        count(r#"g.v().properties().has_key(P.neq("name")).count().next()"#),
        6
    );
    assert_eq!(
        count(r#"g.v().properties().has_key(P.eq("name")).count().next()"#),
        6
    );
    assert_eq!(
        count(r#"g.v().properties().has_key(P.within("name", "age")).count().next()"#),
        10
    );
    assert_eq!(
        count(r#"g.v().properties().has_key(P.starting_with("a")).count().next()"#),
        4
    );
}

#[test]
fn a_predicate_mixed_with_values_is_an_argument_error() {
    for script in [
        r#"g.V().hasId(P.neq("1"), "2")"#,
        r#"g.V().hasLabel(P.neq("a"), "person")"#,
        r#"g.V().properties().hasKey("name", P.neq("age"))"#,
    ] {
        let err = eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
            .expect_err(script)
            .to_string();
        assert!(
            err.contains("a predicate mixed with other arguments"),
            "{err}"
        );
        assert!(
            err.contains("has_id(), has_label() and has_key() take either plain values"),
            "{err}"
        );
    }
}

#[test]
fn a_predicate_is_never_stringified_into_a_literal() {
    // Before, every one of these matched nothing because the predicate became the string
    // "crate::script_host::ArgP".
    for script in [
        r#"g.V().hasId(P.lt("zzz")).count().next()"#,
        r#"g.V().hasLabel(P.lt("zzz")).count().next()"#,
        r#"g.V().properties().hasKey(P.lt("zzz")).count().next()"#,
    ] {
        assert!(count(script) > 0, "{script}");
    }
}

#[test]
fn profile_shows_the_predicate() {
    let profile = run(r#"g.V().hasId(P.neq("1")).count().profile().to_string()"#).to_string();
    assert!(profile.contains(r#"has_id(P.neq("1"))"#), "{profile}");
}

#[test]
fn equality_predicates_still_fold_into_the_source() {
    let profile = run(r#"g.V().hasId(P.eq("1")).count().profile().to_string()"#).to_string();
    assert!(!profile.contains("has_id("), "{profile}");
}

#[test]
fn rust_api_token_predicates() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();
    let ids = locked
        .traversal()
        .v(None)
        .has_id_p(P::Neq("1"))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(ids[0].as_i64(), Some(5));
    let labels = locked
        .traversal()
        .v(None)
        .has_label_p(P::Neq("person"))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(labels[0].as_i64(), Some(2));
    let keys = locked
        .traversal()
        .v(None)
        .properties(None)
        .has_key_p(P::Neq("name"))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(keys[0].as_i64(), Some(6));
    let nested = locked
        .traversal()
        .v(None)
        .where_t(__::has_label_p(P::Neq("person")))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(nested[0].as_i64(), Some(2));
}

#[test]
fn where_eq_resolves_a_side_effect() {
    let ids = |predicate: &str| -> Vec<String> {
        let script =
            format!(r#"g.withSideEffect("a", "2").V("1").out().id().where({predicate}).toList()"#);
        let mut out: Vec<String> = run(&script)
            .into_array()
            .unwrap()
            .into_iter()
            .map(|v| v.to_string())
            .collect();
        out.sort();
        out
    };
    assert_eq!(ids(r#"P.eq("a")"#), vec!["2"]);
    assert_eq!(ids(r#"P.neq("a")"#), vec!["3", "4"]);
    assert_eq!(ids(r#"P.gt("a")"#), vec!["3", "4"]);
    assert_eq!(ids(r#"P.lte("a")"#), vec!["2"]);
    // An unknown key stays a literal.
    assert!(ids(r#"P.eq("zzz")"#).is_empty());
    assert_eq!(ids(r#"P.eq("3")"#), vec!["3"]);
}

#[test]
fn where_neq_side_effect_through_id_traversal() {
    assert_eq!(
        count(
            r#"g.with_side_effect("a", "2").v("1").out().where(__.id().where(P.neq("a"))).count().next()"#
        ),
        2
    );
}

#[test]
fn a_step_label_shadows_a_side_effect() {
    // `a` is both a side effect ("2") and a path label (the stream value itself): the label wins,
    // so every value equals its own label.
    assert_eq!(
        count(
            r#"g.with_side_effect("a", "2").v("1").out().id().as("a").where(P.eq("a")).count().next()"#
        ),
        3
    );
}

#[test]
fn a_predicate_has_a_script_type_name() {
    // Not the Rust path of the type behind it, in `type_of` and in diagnostics.
    for script in [
        "type_of(P.eq(1))",
        "type_of(gt(2))",
        "type_of(P.gt(2).and(P.lt(5)))",
        r#"type_of(TextP.containing("a"))"#,
    ] {
        assert_eq!(run(script).to_string(), "Predicate", "{script}");
    }
    let err = eval_value(Arc::new(GraphSource::tinkerpop_modern()), "P.eq(1) + 1")
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(err.contains("+ (Predicate, int64)"), "{err}");
}

/// Ids are strings: a number operand of an equality predicate (`eq`, `neq`, `within`,
/// `without`, nested too) compares as its literal text, in the Rust API and the DSL, on vertices
/// and edges. The order predicates on numbers stay a plan-time error (error_diagnostics_tests).
#[test]
fn equality_predicates_on_ids_take_numbers() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();
    let vertices = |p: P<i64>| {
        locked
            .traversal()
            .v(None)
            .has_id_p(p)
            .id()
            .to_list()
            .unwrap()
            .into_iter()
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
    };
    let edges = |p: P<i64>| {
        locked
            .traversal()
            .e(None)
            .has_id_p(p)
            .count()
            .to_list()
            .unwrap()[0]
            .as_i64()
    };
    assert_eq!(vertices(P::Eq(3)), ["3"]);
    assert_eq!(vertices(P::WithIn(vec![1, 2])), ["1", "2"]);
    assert_eq!(vertices(P::Neq(3)), ["1", "2", "4", "5", "6"]);
    assert_eq!(vertices(P::WithOut(vec![1, 2])), ["3", "4", "5", "6"]);
    assert_eq!(
        vertices(P::Or(Box::new(P::Eq(1)), Box::new(P::Eq(2)))),
        ["1", "2"]
    );
    assert_eq!(vertices(P::Not(Box::new(P::Eq(1)))).len(), 5);
    assert_eq!(edges(P::Eq(3)), Some(1));
    assert_eq!(edges(P::Neq(3)), Some(5));
    assert_eq!(edges(P::WithIn(vec![0, 5, 9])), Some(2));
    assert_eq!(edges(P::WithOut(vec![0, 5])), Some(4));
    // a float operand is its literal text "1.0", which no modern id spells (like has_id(1.0))
    let float = locked
        .traversal()
        .v(None)
        .has_id_p(P::Eq(1.0))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(float[0].as_i64(), Some(0));
    assert_eq!(count("g.v().has_id(1.0).count().next()"), 0);
    // the profile renders the predicate as written
    let profile = run("g.v().has_id(P.neq(3)).count().profile().to_string()").to_string();
    assert!(profile.contains("has_id(P.neq(3))"), "{profile}");

    for (script, expected) in [
        ("g.v().has_id(P.neq(3)).count().next()", 5),
        ("g.V().hasId(P.neq(3)).count().next()", 5),
        ("g.v().has_id(P.without(1, 2)).count().next()", 4),
        ("g.V().hasId(P.without([1, 2])).count().next()", 4),
        ("g.v().has_id(P.eq(1).or(P.eq(2))).count().next()", 2),
        ("g.V().hasId(P.eq(1).or(P.eq(2))).count().next()", 2),
        ("g.V().hasId(P.not(P.within(1, 2))).count().next()", 4),
        ("g.v().has(T.id, P.neq(3)).count().next()", 5),
        ("g.V().has(T.id, P.neq(3)).count().next()", 5),
        ("g.e().has_id(P.neq(3)).count().next()", 5),
        ("g.E().hasId(P.without(0, 5)).count().next()", 4),
        ("g.E().has(T.id, P.neq(3)).count().next()", 5),
        ("g.e().has_id(P.eq(1).or(P.eq(2))).count().next()", 2),
        // strings keep working beside numbers
        (r#"g.V().hasId(P.without(1, "2")).count().next()"#, 4),
    ] {
        assert_eq!(count(script), expected, "{script}");
    }
}
