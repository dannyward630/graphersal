//! `has(key, traversal)`: TinkerPop's `has(String, Traversal)`, i.e.
//! `filter(values(key).traversal)`. The child runs on the property's value and the element passes
//! iff it yields at least one result; an element without the property is filtered out. Before,
//! the DSL turned the anonymous traversal into the string "GraphTraversalSource" and compared
//! the property with it (silently matching nothing).
#![cfg(feature = "script")]

use std::collections::HashMap;
use std::sync::Arc;

use graphersal::prelude::*;
use graphersal::script::rhai::Dynamic;

use crate::script_host::{ScriptLimits, eval_value_with_limits};

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run(graph: &Arc<Graph>, script: &str) -> Result<Dynamic, String> {
    eval_value_with_limits(
        graph.clone(),
        script,
        HashMap::new(),
        &ScriptLimits::default(),
    )
    .map_err(|err| err.to_string())
}

/// The ids a traversal (without terminal, after `g.`) yields, sorted.
fn ids(graph: &Arc<Graph>, query: &str) -> Vec<String> {
    let mut ids: Vec<String> = run(graph, &format!("g.{query}.id().to_list()"))
        .unwrap_or_else(|err| panic!("{query}: {err}"))
        .into_array()
        .expect("a list")
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    ids.sort();
    ids
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn the_child_runs_on_the_value_and_an_element_passes_when_it_yields_something() {
    let g = modern();
    assert_eq!(
        ids(&g, r#"V().has("name", __.is("marko"))"#),
        strings(&["1"])
    );
    assert_eq!(
        ids(&g, r#"V().has("name", __.is("marko"))"#),
        ids(&g, r#"V().has("name", "marko")"#)
    );
    assert_eq!(
        ids(&g, r#"V().has("age", __.is(P.gt(30)))"#),
        strings(&["4", "6"])
    );
    // A case-insensitive search: the lowered value against a lower-case constant.
    assert_eq!(
        ids(
            &g,
            r#"V().has("name", __.to_lower().is("MARKO".to_lower()))"#
        ),
        strings(&["1"])
    );
    // The constant is compared as given.
    assert!(ids(&g, r#"V().has("name", __.to_lower().is("MARKO"))"#).is_empty());
    // A child that yields nothing filters the element out.
    assert!(ids(&g, r#"V().has("name", __.is("nobody"))"#).is_empty());
}

#[test]
fn an_absent_property_filters_the_element_out_whatever_the_child() {
    let g = modern();
    // identity() yields the value of every element that has the property.
    assert_eq!(
        ids(&g, r#"V().has("lang", __.identity())"#),
        strings(&["3", "5"])
    );
    // Even a child that would yield something without input (a constant).
    assert_eq!(
        ids(&g, r#"V().has("lang", __.constant(1))"#),
        strings(&["3", "5"])
    );
    assert!(ids(&g, r#"V().has("missing", __.constant(1))"#).is_empty());
}

#[test]
fn several_results_of_the_child_pass_the_element_once() {
    let g = modern();
    let found = ids(
        &g,
        r#"V().has("name", __.union(__.identity(), __.identity(), __.constant("x")))"#,
    );
    assert_eq!(found, strings(&["1", "2", "3", "4", "5", "6"]));
    let count = run(
        &g,
        r#"g.V().has("age", __.union(__.identity(), __.identity())).count().next()"#,
    )
    .unwrap();
    assert_eq!(count.as_int().unwrap(), 4);
}

#[test]
fn both_spellings_the_label_form_and_anonymous_starts_work() {
    let g = modern();
    assert_eq!(
        ids(
            &g,
            r#"V().hasLabel("person").has("name", __.toLower().is("josh"))"#
        ),
        strings(&["4"])
    );
    assert_eq!(
        ids(&g, r#"V().has("person", "name", __.is("vadas"))"#),
        strings(&["2"])
    );
    // The three-argument form also tests the label.
    assert!(ids(&g, r#"V().has("software", "name", __.is("vadas"))"#).is_empty());
    // Inside a child traversal and as the start of one.
    assert_eq!(
        ids(&g, r#"V().where(__.out().has("lang", __.is("java")))"#),
        strings(&["1", "4", "6"])
    );
    assert_eq!(
        ids(&g, r#"V().where(__.has("age", __.is(P.lt(28))))"#),
        strings(&["2"])
    );
}

#[test]
fn a_jpath_key_reads_the_nested_value() {
    let g = modern();
    let _ = run(
        &g,
        r#"g.V("1").property("meta", #{city: "Berlin", tags: ["a", "b"]}).iterate()"#,
    )
    .unwrap();
    assert_eq!(
        ids(
            &g,
            r#"V().has(jpath("meta.city"), __.to_lower().is("berlin"))"#
        ),
        strings(&["1"])
    );
    assert_eq!(
        ids(&g, r#"V().has(jpath("meta.tags[1]"), __.is("b"))"#),
        strings(&["1"])
    );
    assert!(ids(&g, r#"V().has(jpath("meta.nope"), __.identity())"#).is_empty());
}

#[test]
fn the_step_renders_as_written_and_errors_carry_the_child_step_ids() {
    let g = modern();
    let metrics = run(
        &g,
        r#"g.V().has("name", __.to_lower().is("marko")).profile()"#,
    )
    .unwrap()
    .try_cast::<graphersal::profile::TraversalMetrics>()
    .expect("metrics");
    let names: Vec<String> = metrics
        .metrics()
        .iter()
        .map(|step| step.name().to_string())
        .collect();
    assert_eq!(
        names[1], r#"has("name", __.to_lower().is(P.eq("marko")))"#,
        "{names:?}"
    );
    // Camel spelling respells the child.
    let camel = run(
        &g,
        r#"g.with("render.spelling", "camel").V().has("name", __.to_lower().is("marko")).profile()"#,
    )
    .unwrap()
    .try_cast::<graphersal::profile::TraversalMetrics>()
    .expect("metrics");
    assert!(
        camel.metrics()[1].name().contains("__.toLower()"),
        "{}",
        camel.metrics()[1].name()
    );
    // The child is values(key) followed by the given steps: to_lower() is step 1.0.1.
    let err = run(&g, r#"g.V().has("age", __.to_lower()).to_list()"#).unwrap_err();
    assert!(err.contains("Step #1.0.1 'to_lower()'"), "{err}");
    assert!(err.contains(r#"values("age").to_lower()"#), "{err}");
}

#[test]
fn a_mutating_child_runs_per_value_like_in_tinkerpop() {
    let g = modern();
    // addV() in the child creates one vertex per incoming value (TinkerPop's AddVertexStep
    // inside a filter child), and every element with the property passes.
    let passed = run(&g, r#"g.V().has("name", __.addV("copy")).count().next()"#).unwrap();
    assert_eq!(passed.as_int().unwrap(), 6);
    let copies = run(&g, r#"g.V().hasLabel("copy").count().next()"#).unwrap();
    assert_eq!(copies.as_int().unwrap(), 6);
}

#[test]
fn the_rust_api_builds_the_same_step() {
    let graph = TraversalGraph::tinkerpop_modern();
    let found = graph
        .traversal()
        .v(None)
        .has_traversal("name", __::to_lower().is("josh"))
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(found, vec![ElementProperty::from("josh")]);
    let anonymous = graph
        .traversal()
        .v(None)
        .filter_t(__::has_traversal("age", __::is(P::Gt(30))))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(anonymous, vec![ElementProperty::from(2_i64)]);
}

/// `has(T.id, traversal)` / `has(T.label, traversal)`: TinkerPop's `has(T, Traversal)`, i.e.
/// `filter(id().traversal)` / `filter(label().traversal)`. Before, the DSL turned the traversal
/// into the string "GraphTraversalSource" and matched nothing (issue #9).
#[test]
fn the_token_forms_run_the_child_on_the_id_or_the_label() {
    let g = modern();
    assert_eq!(ids(&g, r#"V().has(T.id, __.is("1"))"#), strings(&["1"]));
    assert_eq!(
        ids(&g, r#"V().has(T.label, __.is("person"))"#),
        strings(&["1", "2", "4", "6"])
    );
    assert_eq!(
        ids(
            &g,
            r#"V().hasLabel("software").has(T.id, __.is(P.neq("3")))"#
        ),
        strings(&["5"])
    );
    // Edges, both spellings and the anonymous start.
    let created = run(&g, r#"g.E().has(T.label, __.is("created")).count().next()"#).unwrap();
    assert_eq!(created.as_int().unwrap(), 4);
    let nested = run(
        &g,
        r#"g.v().where(__.has(T.label, __.is("person"))).count().next()"#,
    )
    .unwrap();
    assert_eq!(nested.as_int().unwrap(), 4);
    // The same as the filter it expands to.
    assert_eq!(
        ids(
            &g,
            r#"V().has(T.label, __.is(P.within("person", "software")).is(P.neq("software")))"#
        ),
        ids(
            &g,
            r#"V().filter(__.label().is(P.within("person", "software")).is(P.neq("software")))"#
        )
    );
}

#[test]
fn the_token_forms_render_as_written_with_the_child_step_ids() {
    let g = modern();
    let metrics = run(&g, r#"g.V().has(T.id, __.is("1")).profile()"#)
        .unwrap()
        .try_cast::<graphersal::profile::TraversalMetrics>()
        .expect("metrics");
    let names: Vec<String> = metrics
        .metrics()
        .iter()
        .map(|step| step.name().to_string())
        .collect();
    assert_eq!(names[1], r#"has(T.id, __.is(P.eq("1")))"#, "{names:?}");
    // The child is label() followed by the given steps: math() on a label string is the
    // failing step 1.0.1.
    let err = run(&g, r#"g.V().has(T.label, __.math("_ + 1")).to_list()"#).unwrap_err();
    assert!(err.contains("Step #1.0.1"), "{err}");
}

#[test]
fn the_rust_api_takes_a_record_key_token() {
    let graph = TraversalGraph::tinkerpop_modern();
    let count = |key: RecordKey, child| {
        graph
            .traversal()
            .v(None)
            .has_traversal(key, child)
            .count()
            .to_list()
            .unwrap()
    };
    assert_eq!(
        count(RecordKey::Label, __::is("person")),
        vec![ElementProperty::from(4_i64)]
    );
    assert_eq!(
        count(RecordKey::Id, __::is("1")),
        vec![ElementProperty::from(1_i64)]
    );
    // A RecordKey::Name is the property of that name.
    assert_eq!(
        count(RecordKey::Name("name".into()), __::is("josh")),
        vec![ElementProperty::from(1_i64)]
    );
    // Direction.OUT/IN name nothing to test: refused before the run, even on an empty graph.
    for key in [RecordKey::Out, RecordKey::In] {
        let err = TraversalGraph::default()
            .traversal()
            .v(None)
            .has_traversal(key, __::identity())
            .to_list()
            .expect_err("only T.id and T.label");
        let report = format_diagnostic(&err);
        assert!(
            report.contains("names nothing a traversal can test"),
            "{report}"
        );
        assert!(
            report.contains("has(T.label, __.is(\"person\"))"),
            "{report}"
        );
    }
}
