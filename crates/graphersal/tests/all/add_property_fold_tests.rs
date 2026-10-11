//! The `add_property_fold` optimizer rule: `addV(..)`/`addE(..)` plus the directly following
//! constant `property(k, v)` steps become ONE `add_vertex`/`add_edge` call (schema-audit task 21,
//! decision A). Covers schema enforcement (Open and Closed), fold boundaries, the rule disabled,
//! profile output, a differential on/off run without a schema, bulk, and both notations.
#![cfg(all(feature = "script", feature = "serde"))]

use std::sync::Arc;

use crate::script_host::eval_value;
use graphersal::prelude::*;
use graphersal::schema::{EdgeSchema, PropertySchema, VertexSchema};

fn required(ty: PropertySchema) -> PropertySchema {
    ty
}

/// `Person(name: string!, age: int64!)`, `Company(name!)`, `works_at(since: int64!): Person -> Company`.
fn schema(mode: SchemaMode) -> GraphSchema {
    let mut s = GraphSchema::new().with_mode(mode);
    let vertex = |_label: &str, fields: Vec<(&str, PropertySchema)>| {
        VertexSchema::new(
            fields
                .into_iter()
                .fold(PropertySchema::object(), |root, (k, f)| {
                    root.with_required_property(k, f)
                }),
        )
    };
    s.vertices.insert(
        "Person".into(),
        vertex(
            "Person",
            vec![
                ("name", required(PropertySchema::string())),
                ("age", required(PropertySchema::integer())),
            ],
        ),
    );
    s.vertices.insert(
        "Company".into(),
        vertex(
            "Company",
            vec![("name", required(PropertySchema::string()))],
        ),
    );
    s.edges.insert(
        "works_at".into(),
        EdgeSchema::new(
            PropertySchema::object()
                .with_required_property("since", required(PropertySchema::integer())),
        )
        .with_connection("Person", "Company"),
    );
    s
}

const MODES: [SchemaMode; 2] = [SchemaMode::Open, SchemaMode::Closed];

fn graph_with(mode: Option<SchemaMode>) -> Arc<Graph> {
    let mut graph = TraversalGraph::new();
    if let Some(mode) = mode {
        graph.set_schema(schema(mode)).unwrap();
    }
    Arc::new(Graph::new(graph))
}

/// Person "p1" and Company "c1" (created before the schema is applied, so the fixture is valid
/// under both modes).
fn with_endpoints(mode: SchemaMode) -> Arc<Graph> {
    let mut graph = TraversalGraph::new();
    graph
        .traversal_mut()
        .add_v("Person")
        .property("name", "Ann")
        .property("age", 30)
        .property_id("p1")
        .next()
        .unwrap();
    graph
        .traversal_mut()
        .add_v("Company")
        .property("name", "Acme")
        .property_id("c1")
        .next()
        .unwrap();
    graph.set_schema(schema(mode)).unwrap();
    Arc::new(Graph::new(graph))
}

fn run(graph: &Arc<Graph>, script: &str) -> Result<(), String> {
    eval_value(graph.clone(), script)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn int(graph: &Arc<Graph>, script: &str) -> i64 {
    eval_value(graph.clone(), script)
        .unwrap_or_else(|e| panic!("`{script}` failed: {e}"))
        .as_int()
        .unwrap_or_else(|_| panic!("`{script}` is not an int"))
}

fn profile(graph: &Arc<Graph>, script: &str) -> String {
    eval_value(graph.clone(), script)
        .unwrap_or_else(|e| panic!("`{script}` failed: {e}"))
        .to_string()
}

const OFF: &str = r#"g.with("optimizer.disabled", ["add_property_fold"])"#;

#[test]
fn vertex_with_required_properties_is_created_in_open_and_closed() {
    for mode in MODES {
        let graph = graph_with(Some(mode));
        run(
            &graph,
            r#"g.addV("Person").property("name", "Ann").property("age", 30).next()"#,
        )
        .unwrap();
        assert_eq!(int(&graph, "g.V().count().next()"), 1, "{mode:?}");
        assert_eq!(int(&graph, r#"g.V().values("age").next()"#), 30, "{mode:?}");
    }
}

#[test]
fn edge_with_required_properties_is_created_in_open_and_closed() {
    for mode in MODES {
        let graph = with_endpoints(mode);
        // property before and after from()/to(): both stay in the run.
        run(
            &graph,
            r#"g.addE("works_at").property("since", 2021).from(__.V("p1")).to(__.V("c1")).next()"#,
        )
        .unwrap();
        run(
            &graph,
            r#"g.addE("works_at").from(__.V("p1")).property("since", 2022).to(__.V("c1")).next()"#,
        )
        .unwrap();
        assert_eq!(int(&graph, "g.E().count().next()"), 2, "{mode:?}");
        run(
            &graph,
            r#"g.addE("works_at").from(__.V("p1")).to(__.V("c1")).property("since", 2023).next()"#,
        )
        .unwrap();
        assert_eq!(
            int(&graph, r#"g.E().has("since", 2023).count().next()"#),
            1,
            "{mode:?}"
        );
    }
}

#[test]
fn a_failing_property_leaves_no_element_behind() {
    for mode in MODES {
        let graph = graph_with(Some(mode));
        assert!(
            run(
                &graph,
                r#"g.addV("Person").property("name", "Ann").property("age", "old").next()"#
            )
            .is_err()
        );
        assert_eq!(int(&graph, "g.V().count().next()"), 0, "{mode:?}");
        let graph = with_endpoints(mode);
        assert!(run(
            &graph,
            r#"g.addE("works_at").property("since", "x").from(__.V("p1")).to(__.V("c1")).next()"#
        )
        .is_err());
        assert_eq!(int(&graph, "g.E().count().next()"), 0, "{mode:?}");
    }
}

#[test]
fn a_repeated_key_keeps_the_last_value_fused_and_unfused() {
    for prefix in ["g", OFF] {
        let graph = graph_with(None);
        run(
            &graph,
            &format!(
                r#"{prefix}.addV("p").property("a", 1).property("b", 7).property("a", 2).next()"#
            ),
        )
        .unwrap();
        assert_eq!(int(&graph, r#"g.V().values("a").next()"#), 2, "{prefix}");
        assert_eq!(int(&graph, r#"g.V().values("b").next()"#), 7, "{prefix}");
    }
}

#[test]
fn as_between_add_and_property_does_not_break_the_fold() {
    for mode in MODES {
        let graph = graph_with(Some(mode));
        run(
            &graph,
            r#"g.addV("Person").as("x").property("name", "Ann").property("age", 30).select("x").values("name").next()"#,
        )
        .unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        assert_eq!(int(&graph, "g.V().count().next()"), 1, "{mode:?}");
    }
}

#[test]
fn a_traversal_valued_property_stops_the_fold() {
    // `age` comes from a traversal that needs the new vertex as input: it cannot be folded, so in
    // Closed the creation (name only) is rejected and nothing is left behind.
    let graph = graph_with(Some(SchemaMode::Closed));
    let script =
        r#"g.addV("Person").property("name", "Ann").property("age", __.constant(5)).next()"#;
    assert!(run(&graph, script).is_err());
    assert_eq!(int(&graph, "g.V().count().next()"), 0);
    // Without a schema the same query works and stores the value.
    let graph = graph_with(None);
    run(&graph, script).unwrap();
    assert_eq!(int(&graph, r#"g.V().values("age").next()"#), 5);
    // The run before the traversal-valued property is fused, everything after stays unfused.
    let plan = profile(
        &graph,
        r#"g.addV("Person").property("name", "Ann").property("age", __.constant(5)).property("z", 1).profile().to_string()"#,
    );
    assert!(
        plan.contains(r#"add_v("Person", properties: ["name"])"#),
        "{plan}"
    );
    assert!(plan.contains(r#"property("age""#), "{plan}");
    assert!(plan.contains(r#"property("z", 1)"#), "{plan}");
}

#[test]
fn a_jpath_keyed_property_stays_unfused() {
    let graph = graph_with(None);
    let plan = profile(
        &graph,
        r#"g.addV("p").property("a", 1).property(jpath("m.k"), 2).profile().to_string()"#,
    );
    assert!(plan.contains(r#"add_v("p", properties: ["a"])"#), "{plan}");
    assert!(plan.contains("property(jpath("), "{plan}");
}

#[test]
fn with_the_rule_disabled_the_creation_fails_cleanly_and_the_graph_is_unchanged() {
    let graph = graph_with(Some(SchemaMode::Closed));
    let err = run(
        &graph,
        &format!(r#"{OFF}.addV("Person").property("name", "Ann").property("age", 30).next()"#),
    )
    .expect_err("the empty creation violates the required properties");
    assert!(err.contains("Missing required property"), "{err}");
    assert_eq!(int(&graph, "g.V().count().next()"), 0);
    // And with the rule on, the same query works.
    run(
        &graph,
        r#"g.addV("Person").property("name", "Ann").property("age", 30).next()"#,
    )
    .unwrap();
    assert_eq!(int(&graph, "g.V().count().next()"), 1);
}

#[test]
fn profile_shows_the_fused_step_and_names_the_rule() {
    let graph = graph_with(None);
    let plan = profile(
        &graph,
        r#"g.addV("Person").property("name", "Ann").property("age", 30).profile().to_string()"#,
    );
    assert!(
        plan.contains(r#"add_v("Person", properties: ["name", "age"])"#),
        "{plan}"
    );
    assert!(!plan.contains("property("), "{plan}");
    assert!(
        plan.contains("Optimizer rules applied:") && plan.contains("add_property_fold"),
        "{plan}"
    );
    let off = profile(
        &graph,
        &format!(r#"{OFF}.addV("Person").property("name", "Ann").profile().to_string()"#),
    );
    assert!(
        off.contains(r#"property("name", "Ann")"#) && !off.contains("properties:"),
        "{off}"
    );
    assert!(!off.contains("add_property_fold"), "{off}");

    let graph = with_endpoints(SchemaMode::Closed);
    let plan = profile(
        &graph,
        r#"g.addE("works_at").property("since", 1).from(__.V("p1")).to(__.V("c1")).profile().to_string()"#,
    );
    assert!(
        plan.contains(r#"add_e("works_at", properties: ["since"])"#),
        "{plan}"
    );
}

#[test]
fn rule_on_and_off_agree_without_a_schema() {
    let queries = [
        r#".addV("p").property("a", 1).property("b", "x").property("a", 3).valueMap().toList()"#,
        r#".addV("p").property("a", 1).property("b", __.values("a")).elementMap().toList()"#,
        r#".addV("p").as("n").property("a", 1).select("n").values("a").toList()"#,
        r#".addV("p").property("a", 1).addV("q").property("b", 2).valueMap().toList()"#,
    ];
    for query in queries {
        let on = graph_with(None);
        let off = graph_with(None);
        let a = eval_value(on.clone(), &format!("g{query}"))
            .map(|v| v.to_string())
            .map_err(|e| e.to_string());
        let b = eval_value(off.clone(), &format!("{OFF}{query}"))
            .map(|v| v.to_string())
            .map_err(|e| e.to_string());
        assert_eq!(a, b, "{query}");
        let dump = "g.V().elementMap().toList()";
        assert_eq!(profile(&on, dump), profile(&off, dump), "{query}");
    }
}

#[test]
fn a_bulked_add_creates_one_element_per_traverser() {
    // A bulk-3 traverser from a merge point still creates per traverser, with the folded properties.
    let graph = graph_with(Some(SchemaMode::Closed));
    run(
        &graph,
        r#"g.inject(1, 1, 1).addV("Person").property("name", "Ann").property("age", 30).toList()"#,
    )
    .unwrap();
    assert_eq!(int(&graph, "g.V().count().next()"), 3);
    assert_eq!(int(&graph, r#"g.V().has("age", 30).count().next()"#), 3);
}

#[test]
fn rust_api_folds_into_one_creation() {
    for mode in MODES {
        let mut graph = TraversalGraph::new();
        graph.set_schema(schema(mode)).unwrap();
        graph
            .traversal_mut()
            .add_v("Person")
            .property("name", "Ann")
            .property("age", 30)
            .next()
            .unwrap();
        assert_eq!(
            graph.traversal().v(None).count().next().unwrap(),
            Some(ElementProperty::from(1))
        );
        let err = graph
            .traversal_mut()
            .with("optimizer.disabled", vec!["add_property_fold"])
            .add_v("Person")
            .property("name", "Bob")
            .property("age", 31)
            .next();
        // Open does not require declared fields on add yet (schema-audit task 22), so only Closed
        // rejects the unfused empty creation; either way nothing partial is left behind.
        if matches!(mode, SchemaMode::Closed) {
            assert!(err.is_err(), "{mode:?}");
            assert_eq!(
                graph.traversal().v(None).count().next().unwrap(),
                Some(ElementProperty::from(1))
            );
        }
    }
}

#[test]
fn nested_add_in_a_child_traversal_is_folded_too() {
    let graph = graph_with(Some(SchemaMode::Closed));
    run(
        &graph,
        r#"g.inject(1).sideEffect(__.addV("Person").property("name", "Ann").property("age", 30)).toList()"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(int(&graph, "g.V().count().next()"), 1);
}
