//! `where(traversal)` start and end labels (TinkerPop's `WhereStartStep`/`WhereEndStep`), also for
//! the `and()`/`or()`/`not()` roots of a `where()`.
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

fn sorted_strings(script: &str) -> Vec<String> {
    let mut out: Vec<String> = run(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn start_label_starts_the_child_at_the_labelled_object() {
    for query in [
        r#"g.V().as("a").out("created").where(__.as("a").values("name").is("josh")).in("created").values("name").toList()"#,
        r#"g.v().as("a").out("created").where(__.as("a").values("name").is("josh")).in("created").values("name").to_list()"#,
    ] {
        assert_eq!(
            sorted_strings(query),
            ["josh", "josh", "marko", "peter"],
            "{query}"
        );
    }
}

#[test]
fn start_label_in_the_rust_api() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();
    let names = g
        .v(None)
        .as_("a")
        .out("created")
        .where_t(__::as_("a").values("name").is("josh"))
        .in_("created")
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(names.len(), 4);
}

#[test]
fn end_label_keeps_the_traverser_only_for_an_equal_result() {
    assert_eq!(
        count(r#"g.V().limit(2).constant(1).as("b").where(__.constant(2).as("b")).count().next()"#),
        0
    );
    assert_eq!(
        count(r#"g.V().limit(2).constant(1).as("b").where(__.constant(1).as("b")).count().next()"#),
        2
    );
    assert_eq!(
        count(r#"g.v().limit(2).constant(1).as("b").where(__.constant(2).as("b")).count().next()"#),
        0
    );
}

#[test]
fn start_and_end_label_together() {
    assert_eq!(
        count(
            r#"g.V().as("a").out("knows").as("b").where(__.as("a").out("knows").as("b")).select("a","b").count().next()"#
        ),
        2
    );
    // The end object is not reachable from the start object: nothing passes.
    assert_eq!(
        count(
            r#"g.V().as("a").out("knows").as("b").where(__.as("b").out("knows").as("a")).count().next()"#
        ),
        0
    );
}

#[test]
fn and_or_not_roots_read_their_own_labels() {
    assert_eq!(
        count(
            r#"g.V().as("a").out("created").as("b").where(__.and(__.as("b").in(), __.not(__.as("a").out("created").has("name","ripple")))).select("a","b").count().next()"#
        ),
        2
    );
    assert_eq!(
        count(
            r#"g.V().as("a").out("created").as("b").where(__.or(__.as("b").has("name","ripple"), __.as("a").has("name","josh"))).select("a","b").count().next()"#
        ),
        2
    );
    // An end label inside a connective child.
    assert_eq!(
        count(
            r#"g.V().as("a").out("knows").as("b").where(__.or(__.as("a").out("knows").as("b"), __.as("b").has("name","nobody"))).count().next()"#
        ),
        2
    );
}

#[test]
fn a_connective_after_a_start_label_is_an_ordinary_step() {
    // `as("a")` inside the `not()` child stays a plain label of the child's own path here.
    assert_eq!(
        count(r#"g.V().as("a").where(__.as("a").not(__.as("x").out("knows"))).count().next()"#),
        5
    );
}

#[test]
fn missing_labels_filter_without_an_error() {
    assert_eq!(count(r#"g.V().where(__.as("zz").out()).count().next()"#), 0);
    assert_eq!(
        count(r#"g.V().as("a").where(__.out().as("zz")).count().next()"#),
        0
    );
}

#[test]
fn outside_where_as_keeps_its_plain_meaning() {
    assert_eq!(
        count(r#"g.V("1").union(__.as("x").out()).count().next()"#),
        3
    );
    assert_eq!(
        count(r#"g.V().not(__.as("a").out("knows")).count().next()"#),
        5
    );
    assert_eq!(
        count(r#"g.V().and(__.as("a").out("knows"), __.as("b").out("created")).count().next()"#),
        1
    );
}

#[test]
fn lookup_order_is_map_key_side_effect_path_label() {
    // Side-effect key.
    assert_eq!(
        count(
            r#"g.withSideEffect("s", "marko").V("1").where(__.as("s").is("marko")).count().next()"#
        ),
        1
    );
    // Key of the current map value.
    assert_eq!(
        count(r#"g.V().elementMap().where(__.as("name").is("marko")).count().next()"#),
        1
    );
    // The side-effect key wins over a path label of the same name.
    assert_eq!(
        count(
            r#"g.withSideEffect("a", "se").V("1").as("a").where(__.as("a").is("se")).count().next()"#
        ),
        1
    );
}

#[test]
fn more_than_one_label_on_a_start_or_end_step_is_rejected() {
    for query in [
        r#"g.V().as("a").where(__.as("a", "b").out()).count().next()"#,
        r#"g.V().as("a").where(__.out().as("a", "b")).count().next()"#,
        r#"g.V().as("a").where(__.and(__.as("a", "b").out())).count().next()"#,
    ] {
        let err = eval_value(Arc::new(GraphSource::tinkerpop_modern()), query)
            .expect_err(query)
            .to_string();
        assert!(err.contains("can have only one label"), "{query}: {err}");
    }
}

#[test]
fn named_loops_work_in_an_end_label_until_condition() {
    let query = r#"g.V("6").repeat("a", __.both("created").simplePath()).emit(__.repeat("b", __.both("knows")).until(__.loops("b").as("b").where(__.loops("a").as("b"))).has("name", "vadas")).dedup().values("name").toList()"#;
    assert_eq!(sorted_strings(query), ["josh"]);
}

#[test]
fn profile_shows_the_labels_and_the_recorded_path() {
    let text = run(
        r#"g.V().as("a").out("created").where(__.as("a").values("name").is("josh")).profile().to_string()"#,
    )
    .to_string();
    assert!(
        text.contains(r#"where(__.as("a").values("name").is(P.eq("josh")))"#),
        "{text}"
    );
    assert!(
        text.contains("out(\"created\") [path: labels(a)]"),
        "{text}"
    );
    let text = run(
        r#"g.V().limit(2).constant(1).as("b").where(__.constant(1).as("b")).profile().to_string()"#,
    )
    .to_string();
    assert!(text.contains(r#"where(__.constant(1).as("b"))"#), "{text}");
    assert!(text.contains("[path: labels(b)]"), "{text}");
}

#[test]
fn where_unnest_only_flattens_label_free_children() {
    let plain = run(r#"g.V().where(__.hasLabel("person")).profile().to_string()"#).to_string();
    assert!(!plain.contains("where_t"), "{plain}");
    let labelled =
        run(r#"g.V().as("a").where(__.as("a").hasLabel("person")).profile().to_string()"#)
            .to_string();
    assert!(
        labelled.contains(r#"where(__.as("a").has_label("person"))"#),
        "{labelled}"
    );
    assert_eq!(
        count(r#"g.V().as("a").where(__.as("a").hasLabel("person")).count().next()"#),
        4
    );
}

#[test]
fn results_do_not_depend_on_bulk_merging() {
    let merged =
        count(r#"g.V().as("a").out().in().where(__.as("a").out("created")).count().next()"#);
    let unmerged = count(
        r#"g.with("bulk.merge", false).V().as("a").out().in().where(__.as("a").out("created")).count().next()"#,
    );
    assert_eq!(merged, unmerged);
}
