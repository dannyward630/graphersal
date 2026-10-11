//! Source steps (`withSack`, `withSideEffect`, `withPath`) and the one script lowering of `g` and `__`.
//!
//! In Gremlin these steps exist only on the traversal source `g`. The DSL can record them on `__`
//! too; there they used to be dropped silently and now fail with
//! `TraverserError::SourceStepInChild` before execution, wherever the child sits. On `g` they keep
//! working. The last test guards that `g` and `__` lower the same steps to the same plan.
#![cfg(feature = "script")]

use crate::script_host::{ScriptGraphTraversalSource, engine, eval_value, graph_scope};
use graphersal::prelude::*;
use graphersal::prelude::{AnonymousTraversal, Graph};
use std::sync::Arc;

/// Every source step, in both spellings, written inside a child traversal, with the step name
/// the error reports.
const IN_CHILD: [(&str, &str); 14] = [
    ("g.V().local(__.withSack(1).sack())", "with_sack"),
    ("g.v().local(__.with_sack(1).sack())", "with_sack"),
    (
        "g.V().local(__.withSack(1, Operator.sum).sack())",
        "with_sack",
    ),
    (
        "g.v().local(__.with_sack(1, Operator::sum).sack())",
        "with_sack",
    ),
    (
        r#"g.V().local(__.withSideEffect("x", 1).select("x"))"#,
        "with_side_effect",
    ),
    (
        r#"g.v().local(__.with_side_effect("x", 1).select("x"))"#,
        "with_side_effect",
    ),
    (
        r#"g.V().local(__.withSideEffect("a", 0, Operator.sum).aggregate("a"))"#,
        "with_side_effect",
    ),
    (
        r#"g.v().local(__.with_side_effect("a", 0, Operator::sum).aggregate("a"))"#,
        "with_side_effect",
    ),
    (
        r#"g.V().local(__.withSideEffect("a", Set.of()).aggregate("a"))"#,
        "with_side_effect",
    ),
    (
        r#"g.v().local(__.with_side_effect("a", Set::of()).aggregate("a"))"#,
        "with_side_effect",
    ),
    (
        r#"let t = GraphSource::empty(); g.E().local(__.withSideEffect("sg", t).subgraph("sg"))"#,
        "with_side_effect",
    ),
    (
        r#"let t = GraphSource::empty(); g.e().local(__.with_side_effect("sg", t).subgraph("sg"))"#,
        "with_side_effect",
    ),
    ("g.V().local(__.withPath().out().path())", "with_path"),
    ("g.v().local(__.with_path().out().path())", "with_path"),
];

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

/// Evaluates `script` to a traversal (no terminal) against `graph`.
fn traversal(graph: &Arc<Graph>, script: &str) -> ScriptGraphTraversalSource {
    let mut scope = graph_scope(graph.clone());
    engine()
        .eval_with_scope::<ScriptGraphTraversalSource>(&mut scope, script)
        .unwrap_or_else(|e| panic!("`{script}` failed to evaluate: {e}"))
}

/// Runs the traversal `script` evaluates to and returns its results.
fn run(script: &str) -> TraverserResult<Vec<ElementProperty>> {
    let graph = modern();
    let gts = traversal(&graph, script);
    let lock = graph.read();
    gts.ast_convert_to_gts(&*lock).to_list()
}

fn assert_source_step_in_child(err: &TraverserError, step: &str, script: &str) {
    match err.root_cause() {
        TraverserError::SourceStepInChild { step: actual, .. } => {
            assert_eq!(actual, step, "wrong step for `{script}`: {err}")
        }
        other => panic!("`{script}`: expected SourceStepInChild, got: {other}"),
    }
}

#[test]
fn a_source_step_inside_a_child_traversal_fails() {
    for (script, step) in IN_CHILD {
        let err = run(script).expect_err(script);
        assert_source_step_in_child(&err, step, script);
    }
}

/// The plan-time validation pass finds the step even where no traverser ever arrives.
#[test]
fn a_source_step_in_an_unreached_branch_fails_before_execution() {
    for script in [
        r#"g.V().has("name", "nobody").where(__.withSack(1))"#,
        r#"g.V().has("name", "nobody").local(__.out().where(__.with_side_effect("x", 1)))"#,
        r#"g.inject(1).discard().union(__.identity(), __.withSack(2).sack())"#,
        r#"g.V().repeat(__.out().withSideEffect("x", 1)).times(2)"#,
        r#"g.V().order().by(__.withSack(1).values("name"))"#,
    ] {
        let err = run(script).expect_err(script);
        let step = if script.contains("Sack") {
            "with_sack"
        } else {
            "with_side_effect"
        };
        assert_source_step_in_child(&err, step, script);
    }
}

/// A data terminal fails the script; `execute()` returns the error instead of throwing.
#[test]
fn the_script_terminals_report_the_error() {
    let err = eval_value(modern(), "g.V().local(__.withSack(1).sack()).toList()")
        .expect_err("toList() must fail");
    assert!(
        err.to_string()
            .contains("`with_sack()` configures the traversal source g"),
        "{err}"
    );
    let failed = eval_value(
        modern(),
        "let x = g.V().local(__.withSack(1).sack()).execute(); x.error != ()",
    )
    .expect("execute() never throws");
    assert!(failed.as_bool().unwrap_or(false));
}

/// The same steps on `g` configure the source, as before.
#[test]
fn the_source_steps_on_g_still_work() {
    let cases: [(&str, i64); 8] = [
        ("g.withSack(1).V().local(__.out().sack()).sum()", 6),
        ("g.with_sack(1).v().local(__.out().sack()).sum()", 6),
        // lop is reached three times: one traverser with sack 3 and bulk 3 after barrier().
        (
            "g.withSack(1, Operator.sum).V().out().barrier().sack().sum()",
            12,
        ),
        (
            r#"g.withSideEffect("x", 2).V().local(__.select("x")).sum()"#,
            12,
        ),
        (
            r#"g.with_side_effect("a", 0, Operator::sum).v().values("age").aggregate("a").cap("a")"#,
            123,
        ),
        (
            r#"g.withSideEffect("a", Set.of()).V().both().values("name").aggregate("a").cap("a").unfold().count()"#,
            6,
        ),
        (
            r#"let t = GraphSource::empty(); g.withSideEffect("sg", t).E().subgraph("sg").cap("sg").E().count()"#,
            6,
        ),
        // A source step written after other steps still configures `g`.
        ("g.V().withSack(1).local(__.out().sack()).sum()", 6),
    ];
    for (script, expected) in cases {
        let results = run(script).unwrap_or_else(|e| panic!("`{script}`: {e}"));
        assert_eq!(
            results,
            vec![ElementProperty::from(expected)],
            "wrong result for `{script}`"
        );
    }
}

/// `g.<steps>` and `__.<steps>` go through the one lowering: the root plan and the child plan of
/// the same steps are the same, step for step.
#[test]
fn g_and_a_child_traversal_lower_the_same_steps_to_the_same_plan() {
    let bodies = [
        r#"V().hasLabel("person").out("knows").values("name")"#,
        r#"v().as("a").out().as("b").select("a", "b").by("name")"#,
        r#"V().group().by(T.label).by(__.values("name").fold())"#,
        r#"V().repeat(__.out()).times(2).emit().path().by("name")"#,
        r#"V().choose(__.hasLabel("person"), __.out(), __.in()).dedup()"#,
        r#"V().union(__.out(), __.in()).order().by("name", Order.desc).limit(2)"#,
        r#"V().has("age", P.gt(30)).project("n", "a").by("name").by("age")"#,
        r#"V().coalesce(__.outE("knows"), __.inE()).otherV().simplePath()"#,
        r#"E().has("weight", P.lt(0.5)).inV().valueMap(true, "name")"#,
        r#"inject(1, 2, 3).fold().unfold().sum()"#,
        r#"V().sack(Operator.sum).by("age").barrier().sack()"#,
        r#"addV("person").property("name", "x").addE("knows").to(__.V().has("name", "marko"))"#,
        r#"V().aggregate("a").by("name").cap("a")"#,
        r#"V().properties("name").key()"#,
        r#"V().math("_ + 1").by("age")"#,
    ];
    let graph = modern();
    let lock = graph.read();
    for body in bodies {
        let root = traversal(&graph, &format!("g.{body}"));
        let child = traversal(&graph, &format!("__.{body}"));
        let root: AnonymousTraversal<_> = root.ast_convert_to_gts(&*lock).into();
        let root = root.to_string();
        let child = child
            .ast_convert_to_anonymous::<&TraversalGraph>()
            .to_string();
        assert_eq!(root, child, "`{body}` lowers differently at the root");
        assert!(root.len() > "__::".len(), "`{body}` lowered to no step");
    }
}

/// The shared lowering builds the root with the anonymous-traversal builders, yet a modulator
/// with no step before it still names `g` at the root and `__` in a child.
#[test]
fn a_modulator_with_no_step_names_g_at_the_root_and_underscores_in_a_child() {
    for (script, expected) in [
        (r#"g.by("x")"#, "g"),
        (r#"g.from("1")"#, "g"),
        (r#"g.V().where(__.by("x"))"#, "__"),
        (r#"g.V().local(__.to("1"))"#, "__"),
    ] {
        let err = run(script).expect_err(script);
        match err.root_cause() {
            TraverserError::InvalidModulator { step, .. } => {
                assert_eq!(step, expected, "`{script}`: {err}")
            }
            other => panic!("`{script}`: expected InvalidModulator, got: {other}"),
        }
    }
}
