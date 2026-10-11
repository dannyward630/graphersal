//! `filter(traversal)`: keeps a traverser when the child yields at least one result. Unlike
//! `where(traversal)`, `as()` inside the child is a plain step label (no start/end label).
//!
//! Source: the TinkerPop 3.8.2 feature files (`Filter.feature`, `ValueMap.feature`).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> String {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let value = eval_value(graph, script).unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

#[test]
fn keeps_traversers_whose_child_yields() {
    let names = run("g.V().filter(__.outE(\"created\")).values(\"name\").toList()");
    for name in ["marko", "josh", "peter"] {
        assert!(names.contains(name), "{names}");
    }
    assert!(
        !names.contains("vadas") && !names.contains("lop"),
        "{names}"
    );
}

#[test]
fn both_spellings_agree_with_where_for_a_plain_child() {
    let w = run("g.V().where(__.outE(\"created\")).values(\"name\").toList()");
    assert_eq!(
        run("g.V().filter(__.outE(\"created\")).values(\"name\").toList()"),
        w
    );
    assert_eq!(
        run("g.v().filter(__.out_e(\"created\")).values(\"name\").to_list()"),
        w
    );
}

#[test]
fn constant_yields_and_is_with_no_match_does_not() {
    assert_eq!(
        run("g.V().filter(__.constant(0)).count().next()"),
        run("g.V().count().next()")
    );
    assert_eq!(
        run("g.V().filter(__.is(0)).count().next()"),
        run("g.V().limit(0).count().next()")
    );
}

#[test]
fn as_inside_the_child_is_a_plain_label_not_a_scope_label() {
    // where(): the leading `as("a")` is a start label that resolves to nothing -> filtered out.
    let w = run("g.V().where(__.as(\"a\").out(\"knows\").as(\"b\")).count().next()");
    // filter(): the labels are ordinary, the child just has to yield.
    let f = run("g.V().filter(__.as(\"a\").out(\"knows\").as(\"b\")).count().next()");
    assert_ne!(w, f);
    assert_eq!(f, run("g.V().filter(__.out(\"knows\")).count().next()"));
}

#[test]
fn child_labels_do_not_leak_and_path_is_intact() {
    let r = run(
        "g.V().has(\"name\", \"marko\").filter(__.out(\"knows\").as(\"k\")).path().by(\"name\").toList()",
    );
    assert!(r.contains("marko"), "{r}");
    assert!(
        eval_value(
            Arc::new(GraphSource::tinkerpop_modern()),
            "g.V().filter(__.out().as(\"k\")).select(\"k\").toList()"
        )
        .is_err()
            || run("g.V().filter(__.out().as(\"k\")).select(\"k\").toList()").contains("[]")
    );
}

#[test]
fn nested_in_anonymous_and_value_map() {
    let r = run("g.V().hasLabel(\"person\").filter(__.outE(\"created\")).valueMap(true).toList()");
    assert!(
        r.contains("marko") && r.contains("josh") && r.contains("peter") && !r.contains("vadas"),
        "{r}"
    );
    let r = run("g.V().where(__.filter(__.out(\"knows\"))).values(\"name\").toList()");
    assert!(r.contains("marko") && !r.contains("josh"), "{r}");
}

#[test]
fn mutating_child_makes_the_plan_mutating() {
    // The child's mutation is applied (is_mutating recurses), like where()/sideEffect().
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let _ = eval_value(
        graph.clone(),
        "g.V().has(\"name\",\"marko\").filter(__.sideEffect(__.property(\"seen\", true))).toList()",
    )
    .unwrap();
    let r = eval_value(graph, "g.V().has(\"seen\", true).count().next()").unwrap();
    assert_eq!(format!("{r:?}"), "1");
}

#[test]
fn profile_shows_the_step_and_its_child() {
    let r = run("g.V().filter(__.outE(\"created\")).count().profile().to_string()");
    assert!(r.contains("filter(") && r.contains("out_e"), "{r}");
}

#[test]
fn rust_api() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();
    let res = g
        .v(None)
        .filter_t(__::out_e("created"))
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(res.len(), 3);
}
