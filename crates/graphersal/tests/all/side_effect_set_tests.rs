//! Set-typed side effects: `withSideEffect("a", Set.of(..))` makes `aggregate`/`store` collect
//! each value once, and `cap`/`select` return a deduplicated list.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> rhai::Dynamic {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn strings(script: &str) -> Vec<String> {
    run(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn aggregate_into_a_set_keeps_each_value_once() {
    let all = sorted(
        ["josh", "lop", "marko", "peter", "ripple", "vadas"]
            .map(String::from)
            .to_vec(),
    );
    for q in [
        r#"g.withSideEffect("a", Set.of()).V().both().values("name").aggregate("a").cap("a").next()"#,
        r#"g.withSideEffect("a", Set::of()).V().both().values("name").aggregate("a").cap("a").next()"#,
        r#"g.with_side_effect("a", Set.ofList([])).V().both().values("name").aggregate("a").cap("a").next()"#,
        r#"g.withSideEffect("a", Set::of_list([])).V().both().values("name").local(__.aggregate("a")).cap("a").next()"#,
        r#"g.withSideEffect("a", Set.of()).V().both().values("name").store("a").cap("a").next()"#,
    ] {
        assert_eq!(sorted(strings(q)), all, "{q}");
    }
}

#[test]
fn a_seed_value_is_kept_first_and_not_duplicated() {
    let q = r#"g.withSideEffect("a", Set.of("marko", "marko", "zed")).V().both().values("name").aggregate("a").cap("a").next()"#;
    let got = strings(q);
    assert_eq!(&got[..2], ["marko", "zed"]);
    assert_eq!(got.len(), 7);
    assert_eq!(got.iter().filter(|n| *n == "marko").count(), 1);
}

#[test]
fn aggregate_by_into_a_set() {
    let q =
        r#"g.withSideEffect("a", Set.of()).V().both().aggregate("a").by("name").cap("a").next()"#;
    assert_eq!(strings(q).len(), 6);
}

#[test]
fn select_reads_a_set_as_a_list() {
    let q = r#"g.withSideEffect("a", Set.of()).V().both().values("name").aggregate("a").limit(1).select("a").next()"#;
    assert_eq!(strings(q).len(), 6);
}

#[test]
fn a_list_seed_still_keeps_duplicates() {
    let q = r#"g.withSideEffect("a", []).V().both().values("name").aggregate("a").cap("a").next()"#;
    assert_eq!(strings(q).len(), 12);
}

#[test]
fn ints_and_floats_are_distinct_members() {
    let q = r#"g.withSideEffect("a", Set.of(1, 1, 1.0)).V().limit(0).cap("a").next()"#;
    assert_eq!(run(q).into_array().unwrap().len(), 2);
}

#[test]
fn bulk_is_invisible_with_merging_on_and_off() {
    for merge in ["true", "false"] {
        let q = format!(
            r#"g.with("bulk.merge", {merge}).withSideEffect("a", Set.of()).V().both().values("name").barrier().aggregate("a").cap("a").next()"#
        );
        assert_eq!(strings(&q).len(), 6, "{q}");
    }
}

#[test]
fn within_tests_membership_of_a_set_side_effect() {
    let q = r#"g.withSideEffect("a", Set.of("marko", "vadas")).V().values("name").where(P.within("a")).count().next()"#;
    assert_eq!(run(q).as_int().unwrap(), 2);
}

#[test]
fn rust_api_with_side_effect_set() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let g = lock.traversal();
    let result = g
        .with_side_effect_set("a", [ElementProperty::from("zed")])
        .v(None::<()>)
        .both(None)
        .values("name")
        .aggregate("a")
        .cap("a")
        .next()
        .unwrap()
        .unwrap();
    let ElementProperty::Array(items) = result else {
        panic!("a set side effect is returned as an array");
    };
    assert_eq!(items.len(), 7);
}

#[test]
fn set_of_rejects_non_plain_values_with_a_help_naming_set_of() {
    for q in [
        r#"g.withSideEffect("a", Set.of(g.V())).V()"#,
        r#"g.withSideEffect("a", Set.of(#{x: 1})).V()"#,
    ] {
        let err = eval_value(Arc::new(GraphSource::tinkerpop_modern()), q)
            .expect_err(q)
            .to_string();
        assert!(err.contains(r#"Set.of("alice", "bob")"#), "{err}");
        assert!(err.contains("Set.of_list"), "{err}");
    }
}

#[test]
fn missing_side_effect_help_mentions_set_seeds() {
    let err = eval_value(
        Arc::new(GraphSource::tinkerpop_modern()),
        r#"g.V().cap("zz").next()"#,
    )
    .expect_err("unpopulated key")
    .to_string();
    assert!(err.contains(r#"with_side_effect("zz", Set.of())"#), "{err}");
}

#[test]
fn equal_values_of_different_elements_collapse() {
    // Two persons named "x": the set compares property values, not the vertices they came from.
    let q = r#"g.withSideEffect("a", Set.of()).V().has("age").values("name").aggregate("a").V().values("name").aggregate("a").cap("a").next()"#;
    assert_eq!(strings(q).len(), 6);
}
