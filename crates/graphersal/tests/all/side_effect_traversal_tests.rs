//! `sideEffect(traversal)`: the child runs for every incoming traverser, its output is discarded
//! and the traverser continues unchanged.
//!
//! Source: the TinkerPop 3.8.2 feature files (`SideEffect.feature`, `MergeEdge.feature`).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run_on(graph: &Arc<Graph>, script: &str) -> String {
    let value =
        eval_value(graph.clone(), script).unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

fn run(script: &str) -> String {
    run_on(&modern(), script)
}

fn fails(script: &str) -> bool {
    eval_value(modern(), script).is_err()
}

#[test]
fn the_stream_is_unchanged_in_both_spellings() {
    let plain = run("g.V().values(\"name\").toList()");
    for spelling in ["sideEffect", "side_effect"] {
        assert_eq!(
            run(&format!(
                "g.V().{spelling}(__.identity()).values(\"name\").toList()"
            )),
            plain
        );
        assert_eq!(
            run(&format!(
                "g.V().{spelling}(__.values(\"name\")).values(\"name\").toList()"
            )),
            plain
        );
        // A child with many results and one with none change nothing either.
        assert_eq!(
            run(&format!(
                "g.V().{spelling}(__.both().both()).values(\"name\").toList()"
            )),
            plain
        );
        assert_eq!(
            run(&format!(
                "g.V().{spelling}(__.has(\"nope\")).values(\"name\").toList()"
            )),
            plain
        );
        assert_eq!(
            run(&format!("g.V().{spelling}(__.out()).count().next()")),
            run("g.V().count().next()")
        );
    }
}

#[test]
fn ids_and_order_are_preserved() {
    assert_eq!(
        run("g.V().sideEffect(__.out()).id().toList()"),
        run("g.V().id().toList()")
    );
}

#[test]
fn a_child_property_write_is_real() {
    let graph = modern();
    assert_eq!(
        run_on(
            &graph,
            "g.V().sideEffect(__.property(\"age\", 22)).count().next()"
        ),
        run("g.V().count().next()")
    );
    assert_eq!(
        run_on(&graph, "g.V().has(\"age\", 22).count().next()"),
        run("g.V().count().next()")
    );
}

#[test]
fn a_child_can_drop_properties_and_add_vertices() {
    let graph = modern();
    run_on(
        &graph,
        "g.V().sideEffect(__.properties(\"age\").drop()).toList()",
    );
    assert_eq!(
        run_on(&graph, "g.V().has(\"age\").count().next()"),
        run("g.inject(0).next()")
    );
    run_on(
        &graph,
        "g.V().hasLabel(\"person\").sideEffect(__.addV(\"seen\")).toList()",
    );
    assert_eq!(
        run_on(&graph, "g.V().hasLabel(\"seen\").count().next()"),
        run("g.V().hasLabel(\"person\").count().next()")
    );
}

#[test]
fn aggregate_inside_a_local_child_feeds_cap() {
    assert_eq!(
        run(
            "g.V().sideEffect(__.local(__.aggregate(\"x\").by(\"name\"))).cap(\"x\").unfold().count().next()"
        ),
        run("g.V().count().next()")
    );
    assert_eq!(
        run(
            "g.withSideEffect(\"x\", Set.of()).V().both().both().sideEffect(__.local(__.aggregate(\"x\").by(\"name\"))).cap(\"x\").unfold().count().next()"
        ),
        run("g.V().both().both().values(\"name\").dedup().count().next()")
    );
}

#[test]
fn the_child_sees_the_incoming_path() {
    for analysis in [true, false] {
        let script = format!(
            "g.with(\"path.analysis\", {analysis}).V().as(\"a\").out().sideEffect(__.select(\"a\").aggregate(\"s\").by(\"name\")).cap(\"s\").unfold().dedup().toList()"
        );
        assert_eq!(
            run(&script),
            run("g.V().as(\"a\").out().select(\"a\").values(\"name\").dedup().toList()")
        );
        let path = format!(
            "g.with(\"path.analysis\", {analysis}).V(\"1\").out().sideEffect(__.path().aggregate(\"p\")).cap(\"p\").unfold().count().next()"
        );
        assert_eq!(run(&path), run("g.V(\"1\").out().count().next()"));
    }
}

#[test]
fn it_works_inside_by_union_and_repeat() {
    let plain = run("g.V().group().by(\"name\").by(__.constant(1))");
    assert_eq!(
        run(
            "g.V().group().by(__.values(\"name\").sideEffect(__.constant(\"z\"))).by(__.constant(1).sideEffect(__.constant(\"x\")))"
        ),
        plain
    );
    assert_eq!(
        run("g.V(\"1\").union(__.out().sideEffect(__.out()), __.identity()).count().next()"),
        run("g.V(\"1\").union(__.out(), __.identity()).count().next()")
    );
    assert_eq!(
        run("g.V(\"1\").repeat(__.out().sideEffect(__.aggregate(\"r\"))).times(2).count().next()"),
        run("g.V(\"1\").repeat(__.out()).times(2).count().next()")
    );
    assert_eq!(
        run(
            "g.V(\"1\").repeat(__.out().sideEffect(__.aggregate(\"r\"))).times(2).cap(\"r\").unfold().count().next()"
        ),
        run(
            "g.V(\"1\").out().aggregate(\"r\").out().aggregate(\"r\").cap(\"r\").unfold().count().next()"
        )
    );
}

#[test]
fn a_failing_child_fails_the_traversal() {
    assert!(fails(
        "g.V().sideEffect(__.values(\"name\").asNumber()).toList()"
    ));
}

#[test]
fn bulk_units_run_a_side_effect_writing_child_once_each() {
    // out().out() of the modern graph reaches `lop` twice (bulk 2 when merged).
    let count = |merge: bool| {
        run(&format!(
            "g.with(\"bulk.merge\", {merge}).V(\"1\").out().out().barrier().sideEffect(__.aggregate(\"x\")).cap(\"x\").unfold().count().next()"
        ))
    };
    assert_eq!(count(true), count(false));
    let results = |merge: bool| {
        run(&format!(
            "g.with(\"bulk.merge\", {merge}).V().both().both().sideEffect(__.values(\"name\")).values(\"name\").toList()"
        ))
    };
    assert_eq!(results(true).len(), results(false).len());
}

#[test]
fn a_mutating_child_makes_the_plan_mutating() {
    // Run on a shared graph: a read-lock-only plan could not write; the write must persist.
    let graph = modern();
    run_on(
        &graph,
        "g.V().has(\"name\", \"marko\").sideEffect(__.property(\"seen\", true)).toList()",
    );
    assert_eq!(
        run_on(&graph, "g.V().has(\"seen\", true).count().next()"),
        run("g.V().has(\"name\", \"marko\").count().next()")
    );
    // Nested one level deeper as well.
    run_on(
        &graph,
        "g.V().has(\"name\", \"josh\").sideEffect(__.local(__.property(\"deep\", 1))).toList()",
    );
    assert_eq!(
        run_on(&graph, "g.V().has(\"deep\", 1).count().next()"),
        run("g.V().has(\"name\", \"josh\").count().next()")
    );
}

#[test]
fn profile_shows_the_step_and_its_child() {
    let out = run("g.V().sideEffect(__.out().aggregate(\"x\")).count().profile().to_string()");
    assert!(out.contains("side_effect("), "{out}");
    assert!(out.contains("aggregate"), "{out}");
}

#[test]
fn count_is_not_pushed_past_the_step() {
    let out = run("g.V().sideEffect(__.out().aggregate(\"x\")).count().profile().to_string()");
    assert!(out.contains("v()") && out.contains("count()"), "{out}");
    assert_eq!(
        run("g.V().sideEffect(__.aggregate(\"x\")).count().next()"),
        run("g.V().count().next()")
    );
}

#[test]
fn rust_api_side_effect_t() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let g = lock.traversal();
    let n = g
        .v(None)
        .side_effect_t(__::out(None).aggregate("x"))
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(n.as_i64(), Some(6));
    let g = lock.traversal();
    let n = g
        .v(None)
        .local(__::side_effect_t(__::values("name")).count())
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(n.as_i64(), Some(6));
}

fn merge_graph() -> Arc<Graph> {
    let graph = Arc::new(GraphSource::empty());
    run_on(
        &graph,
        "g.addV(\"person\").property(\"name\", \"marko\").as(\"a\").addV(\"person\").property(\"name\", \"vadas\").as(\"b\").addE(\"knows\").property(\"weight\", 1).from(\"a\").to(\"b\").toList()",
    );
    graph
}

const KEY: &str = "#{\"T.label\": \"knows\", \"Direction.OUT\": g.V().has(\"name\",\"marko\").id().next(), \"Direction.IN\": g.V().has(\"name\",\"vadas\").id().next()}";

#[test]
fn merge_e_on_match_side_effect_writes_the_matched_edge() {
    let graph = merge_graph();
    let script = format!(
        "g.mergeE({KEY}).option(Merge.onMatch, __.sideEffect(__.property(\"weight\", 0)).constant(#{{}})).toList()"
    );
    let res = run_on(&graph, &script);
    assert!(!res.is_empty());
    assert_eq!(
        run_on(
            &graph,
            "g.E().hasLabel(\"knows\").has(\"weight\", 0).count().next()"
        ),
        run("g.inject(1).next()")
    );
    assert_eq!(
        run_on(&graph, "g.E().has(\"weight\", 1).count().next()"),
        run("g.inject(0).next()")
    );
    assert_eq!(
        run_on(&graph, "g.V().count().next()"),
        run("g.inject(2).next()")
    );
}
