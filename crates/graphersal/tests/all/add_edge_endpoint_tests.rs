//! `addE()`: `property()` between `addE()` and `from()`/`to()`, traversal and side-effect
//! endpoints, and a traversal label (TinkerPop's `AddEdgeStep` / `FromToModulating`).
#![cfg(feature = "script")]

use std::sync::Arc;

use crate::script_host::eval_value;
use graphersal::prelude::*;

fn empty() -> Arc<Graph> {
    Arc::new(Graph::new(TraversalGraph::new()))
}

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

/// An empty graph with vertices `a` and `b` carrying a `name` and labelled by `as()`.
const TWO: &str =
    r#"g.addV("p").property("name","a").as("a").addV("p").property("name","b").as("b")"#;

fn int(graph: &Arc<Graph>, script: &str) -> i64 {
    eval_value(graph.clone(), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .as_int()
        .unwrap_or_else(|_| panic!("{script}: not an int"))
}

fn fail(graph: &Arc<Graph>, script: &str) -> String {
    eval_value(graph.clone(), script)
        .map(|_| ())
        .expect_err(script)
        .to_string()
}

fn run(graph: &Arc<Graph>, script: &str) {
    let _ = eval_value(graph.clone(), script).unwrap_or_else(|e| panic!("{script}: {e}"));
}

#[test]
fn property_before_from_and_to_in_every_order() {
    for chain in [
        r#".addE("knows").property("weight",1).from("a").to("b")"#,
        r#".addE("knows").property("weight",1).to("b").from("a")"#,
        r#".addE("knows").from("a").property("weight",1).to("b")"#,
        r#".addE("knows").as("e").property("weight",1).from("a").to("b")"#,
        r#".add_e("knows").property("weight",1).from("a").to("b")"#,
    ] {
        let graph = empty();
        run(&graph, &format!("{TWO}{chain}.toList()"));
        assert_eq!(
            int(
                &graph,
                r#"g.E().hasLabel("knows").has("weight",1).count().next()"#
            ),
            1,
            "{chain}"
        );
        assert_eq!(
            int(
                &graph,
                r#"g.E().hasLabel("knows").outV().has("name","a").count().next()"#
            ),
            1,
            "{chain}"
        );
        assert_eq!(
            int(
                &graph,
                r#"g.E().hasLabel("knows").inV().has("name","b").count().next()"#
            ),
            1,
            "{chain}"
        );
    }
}

#[test]
fn token_id_still_folds_with_properties_in_between() {
    let graph = empty();
    run(
        &graph,
        &format!(
            r#"{TWO}.addE("knows").property(T.id, "e1").property("weight",1).from("a").to("b").toList()"#
        ),
    );
    assert_eq!(
        int(&graph, r#"g.E("e1").has("weight",1).count().next()"#),
        1
    );
}

#[test]
fn from_to_after_another_step_still_names_that_step() {
    let graph = modern();
    let report = fail(&graph, r#"g.V().property("a",1).from("x").toList()"#);
    assert!(
        report.contains("`property()` cannot use its from()/to() #1"),
        "{report}"
    );
    let report = fail(&graph, r#"g.V().out().from("x").toList()"#);
    assert!(
        report.contains("`out()` cannot use its from()/to() #1"),
        "{report}"
    );
}

#[test]
fn traversal_endpoints_in_both_spellings() {
    for script in [
        r#"g.addE("x").from(__.V("1")).to(__.V("2")).toList()"#,
        r#"g.add_e("x").from(__.V("1")).to(__.V("2")).toList()"#,
        r#"g.V("3").addE("x").from(__.V("1")).to(__.V("2")).toList()"#,
        r#"g.V("1").addE("x").to(__.V("2")).toList()"#,
        r#"g.V("2").addE("x").from(__.V("1")).toList()"#,
        r#"g.V("1").addE("x").to(__.V().has("name","vadas")).toList()"#,
    ] {
        let graph = modern();
        run(&graph, script);
        assert_eq!(
            int(
                &graph,
                r#"g.V("1").outE("x").inV().hasId("2").count().next()"#
            ),
            1,
            "{script}"
        );
    }
}

#[test]
fn a_traversal_endpoint_may_yield_a_vertex_id() {
    let graph = modern();
    run(&graph, r#"g.V("1").addE("x").to(__.V("2").id()).toList()"#);
    assert_eq!(
        int(
            &graph,
            r#"g.V("1").outE("x").inV().hasId("2").count().next()"#
        ),
        1
    );
}

#[test]
fn a_traversal_endpoint_without_a_result_fails() {
    let graph = modern();
    let report = fail(&graph, r#"g.V("1").addE("x").to(__.V("nope")).toList()"#);
    assert!(report.contains("vertex"), "{report}");
    assert_eq!(int(&graph, r#"g.E().hasLabel("x").count().next()"#), 0);
}

#[test]
fn a_non_vertex_endpoint_value_fails() {
    let graph = modern();
    fail(
        &graph,
        r#"g.V("1").addE("x").to(__.V("2").values("name")).toList()"#,
    );
    fail(
        &graph,
        r#"g.V("1").addE("x").to(__.V("2").outE()).toList()"#,
    );
    assert_eq!(int(&graph, r#"g.E().hasLabel("x").count().next()"#), 0);
}

#[test]
fn to_addv_creates_one_vertex_per_incoming_traverser_in_a_repeat() {
    let graph = empty();
    run(
        &graph,
        r#"g.addV().as("first").repeat(__.addE("next").to(__.addV()).inV()).times(5).addE("next").to(__.select("first")).toList()"#,
    );
    assert_eq!(int(&graph, r#"g.V().count().next()"#), 6);
    assert_eq!(int(&graph, r#"g.E().count().next()"#), 6);
    assert_eq!(int(&graph, r#"g.V().not(__.outE()).count().next()"#), 0);
}

#[test]
fn a_path_label_holding_an_id_is_an_endpoint() {
    let graph = modern();
    // `values("name")` is `marko`, which is no vertex id.
    fail(
        &graph,
        r#"g.V("1").values("name").as("who").V("2").addE("x").from("who").toList()"#,
    );
    run(
        &graph,
        r#"g.V("1").id().as("who").V("2").addE("x").from("who").toList()"#,
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V("1").outE("x").inV().hasId("2").count().next()"#
        ),
        1
    );
}

#[test]
fn a_side_effect_holding_a_vertex_or_an_id_is_an_endpoint() {
    let graph = modern();
    run(
        &graph,
        r#"g.withSideEffect("b", "2").V("1").addE("likes").to("b").property("weight", 0.5).toList()"#,
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V("1").outE("likes").has("weight",0.5).inV().hasId("2").count().next()"#
        ),
        1
    );

    let graph = modern();
    run(
        &graph,
        r#"g.V("2").aggregate("v").V("1").addE("x").to(__.cap("v").unfold()).toList()"#,
    );
    assert_eq!(
        int(
            &graph,
            r#"g.V("1").outE("x").inV().hasId("2").count().next()"#
        ),
        1
    );
}

#[test]
fn a_side_effect_bucket_is_not_an_endpoint() {
    let graph = modern();
    let report = fail(
        &graph,
        r#"g.V("2").aggregate("v").V("1").addE("x").to("v").toList()"#,
    );
    assert!(report.contains("vertex"), "{report}");
}

#[test]
fn a_plain_vertex_id_string_is_an_endpoint() {
    let graph = modern();
    run(&graph, r#"g.V("1").addE("x").to("2").toList()"#);
    run(&graph, r#"g.addE("y").from("1").to("3").toList()"#);
    assert_eq!(
        int(
            &graph,
            r#"g.E().hasLabel("x").inV().hasId("2").count().next()"#
        ),
        1
    );
    assert_eq!(
        int(
            &graph,
            r#"g.E().hasLabel("y").inV().hasId("3").count().next()"#
        ),
        1
    );
}

#[test]
fn an_unknown_name_fails() {
    let graph = modern();
    fail(&graph, r#"g.V("1").addE("x").to("nobody").toList()"#);
    assert_eq!(int(&graph, r#"g.E().hasLabel("x").count().next()"#), 0);
}

#[test]
fn a_traversal_label() {
    for script in [
        r#"g.V().has("name","marko").as("a").outE("created").as("b").inV().addE(__.select("b").label()).to("a").toList()"#,
        r#"g.V().has("name","marko").as("a").outE("created").as("b").inV().add_e(__.select("b").label()).to("a").toList()"#,
    ] {
        let graph = modern();
        run(&graph, script);
        // The new edge runs from `lop` to `marko` and is labelled like the edge it came from.
        assert_eq!(
            int(
                &graph,
                r#"g.V().has("name","lop").outE("created").inV().has("name","marko").count().next()"#
            ),
            1,
            "{script}"
        );
    }
}

#[test]
fn a_non_string_traversal_label_fails() {
    let graph = modern();
    fail(
        &graph,
        r#"g.V("1").addE(__.values("age")).to("2").toList()"#,
    );
    fail(
        &graph,
        r#"g.V("1").addE(__.V("nope").label()).to("2").toList()"#,
    );
    assert_eq!(int(&graph, r#"g.V("1").outE().count().next()"#), 3);
}

#[test]
fn add_e_takes_a_label_or_a_traversal_only() {
    let graph = modern();
    let report = fail(&graph, r#"g.V("1").addE(5).to("2").toList()"#);
    assert!(
        report.contains("Argument mismatch for step 'add_e'"),
        "{report}"
    );
    assert!(
        report.contains("add_e(__.select(\"b\").label())"),
        "{report}"
    );
}

#[test]
fn a_from_only_after_an_edge_still_fails() {
    // The second `addE` has `from()` only, so its incoming traverser (an edge) is the `to` end.
    let graph = modern();
    fail(
        &graph,
        r#"g.addE("knows").from(__.V().has("name","marko")).to(__.V().has("name","vadas")).property("weight",0.5).addE("knows").from(__.V().has("name","marko")).toList()"#,
    );
}

#[test]
fn a_parent_with_several_units_creates_one_edge_per_unit_with_bulk_merge_on_and_off() {
    for merge in ["true", "false"] {
        let graph = modern();
        run(
            &graph,
            &format!(
                r#"g.with("bulk.merge", {merge}).V("1","1").addE("x").to(__.V("2")).toList()"#
            ),
        );
        assert_eq!(
            int(&graph, r#"g.E().hasLabel("x").count().next()"#),
            2,
            "bulk.merge={merge}"
        );
    }
}

#[test]
fn a_child_reading_a_label_works_with_path_analysis_on_and_off() {
    for prefix in ["g", r#"g.with("path.analysis", false)"#] {
        let graph = modern();
        run(
            &graph,
            &format!(
                r#"{prefix}.V("1").as("first").out("knows").addE("x").to(__.select("first")).toList()"#
            ),
        );
        assert_eq!(
            int(
                &graph,
                r#"g.E().hasLabel("x").inV().hasId("1").count().next()"#
            ),
            2,
            "{prefix}"
        );
    }
}

#[test]
fn profile_shows_the_child_traversals() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let profile = graph
        .traversal_mut()
        .v("1")
        .as_("a")
        .add_e_by(__::select("a").label())
        .to_by(__::v("2"))
        .profile()
        .unwrap()
        .to_string();
    assert!(profile.contains("add_e("), "{profile}");
    assert!(profile.contains("select"), "{profile}");
    assert!(profile.contains(r#".to(__"#), "{profile}");
}

#[test]
fn rust_api_takes_traversals_for_endpoints_and_label() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let edge = graph
        .traversal_mut()
        .add_e("x")
        .from_by(__::v("1"))
        .to_by(__::v("2"))
        .next()
        .unwrap();
    assert!(matches!(edge, Some(ElementProperty::Edge(_))));
    let edge = graph
        .traversal_mut()
        .v("1")
        .add_e_by(__::constant("viaTraversal"))
        .to_by(__::v("3"))
        .property("w", 1)
        .label()
        .next()
        .unwrap();
    assert_eq!(edge, Some(ElementProperty::from("viaTraversal")));
    let anon = graph
        .traversal_mut()
        .v("1")
        .local(__::add_e_by(__::constant("y")).to_by(__::v("2")))
        .count()
        .next()
        .unwrap();
    assert_eq!(anon, Some(ElementProperty::from(1)));
}

#[test]
fn a_list_of_names_creates_one_edge_per_name() {
    let graph = modern();
    run(&graph, r#"g.V("1").addE("x").to(["2","3"]).toList()"#);
    assert_eq!(int(&graph, r#"g.V("1").outE("x").count().next()"#), 2);
}
