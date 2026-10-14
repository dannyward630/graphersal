//! Cross-cutting behaviour of the wave-4 forms of existing steps: `group(key)`/`groupCount(key)`
//! side effects, the bare `by()`, the infix `and()`/`or()`, `dedup(label...)`,
//! `where(label, P)`, `addV(traversal)`, `property(traversal, value)`, `tail()`, `union()`,
//! the step-level `with(WithOptions..)`, `UUID()` and `select(traversal)` with a non-string key.
#![cfg(feature = "script")]

use std::sync::Arc;

use graphersal::prelude::*;

fn run_on(graph: Graph, script: &str) -> String {
    let value = crate::script_host::eval_value(Arc::new(graph), script)
        .unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

fn run(script: &str) -> String {
    run_on(GraphSource::tinkerpop_modern(), script)
}

fn run_err(script: &str) -> String {
    match crate::script_host::eval_value(Arc::new(GraphSource::tinkerpop_modern()), script) {
        Ok(value) => panic!("`{script}` succeeded with {value:?}"),
        Err(err) => err.to_string(),
    }
}

// --- group("a") / groupCount("a") ------------------------------------------------------------

#[test]
fn group_count_side_effect_grows_once_per_repeat_iteration() {
    // TinkerPop `Repeat::g_VX1X_repeatXgroupCountXmX_byXloopsX_outX_timesX3X_capXmX`.
    let counts = r#"g.V("1").repeat(__.groupCount("m").by(__.loops()).out()).times(3).cap("m")"#;
    assert_eq!(
        run(&format!("{counts}.select(Column.keys).next()")),
        "[0, 1, 2]"
    );
    assert_eq!(
        run(&format!("{counts}.select(Column.values).next()")),
        "[1, 3, 2]"
    );
}

#[test]
fn group_side_effect_is_read_mid_stream_and_grows_per_local_execution() {
    assert_eq!(
        run(r#"g.V().groupCount("a").select("a").count(Scope.local).to_list()"#),
        "[6, 6, 6, 6, 6, 6]"
    );
    assert_eq!(
        run(r#"g.V().local(__.groupCount("a").select("a").count(Scope.local)).to_list()"#),
        "[1, 2, 3, 4, 5, 6]"
    );
}

#[test]
fn reducing_value_traversal_reads_the_member_paths() {
    // The value traversal holds a barrier (`sum()`) and reads the path label `p` of each member.
    assert_eq!(
        run(
            r#"g.V().hasLabel("person").as("p").out("created").group("a").by("name").by(__.select("p").values("age").sum()).cap("a").next()"#
        ),
        r#"#{"lop": 96, "ripple": 32}"#
    );
}

#[test]
fn group_side_effect_keeps_traverser_bulk() {
    // Merged traversers (bulk > 1 after barrier()) count with their bulk.
    assert_eq!(
        run(r#"g.V().out().barrier().groupCount("a").by("name").cap("a").next()"#),
        run(r#"g.V().out().groupCount().by("name").next()"#)
    );
}

#[test]
fn two_group_steps_fill_one_key() {
    assert_eq!(
        run(
            r#"g.V().union(__.hasLabel("person").groupCount("m").by(T.label), __.hasLabel("software").groupCount("m").by(T.label)).cap("m").next()"#
        ),
        r#"#{"person": 4, "software": 2}"#
    );
    let err =
        run_err(r#"g.V().groupCount("m").by(T.label).group("m").by(T.label).cap("m").next()"#);
    assert!(err.contains("reduce differently"), "{err}");
}

// --- bare by() --------------------------------------------------------------------------------

#[test]
fn bare_by_is_the_identity_in_every_by_step() {
    assert_eq!(
        run(
            r#"g.V("1").as("a").values("name").as("b").select("a", "b").by(__.label()).by().next()"#
        ),
        r#"#{"a": "person", "b": "marko"}"#
    );
    assert_eq!(
        run(r#"g.V().values("name").order().by().to_list()"#),
        run(r#"g.V().values("name").order().to_list()"#)
    );
    // group().by(): the value by() collects the members as a list (TinkerPop's
    // convertValueTraversal), it does not keep the last one.
    assert_eq!(
        run(r#"g.V().group().by(T.label).by().select("person").count(Scope.local).next()"#),
        "4"
    );
}

// --- infix and() / or() -----------------------------------------------------------------------

#[test]
fn infix_and_binds_tighter_than_or() {
    // or(and(out("created"), out("knows")), in("knows")): marko, vadas, josh.
    assert_eq!(
        run(
            r#"g.V().where(__.out("created").and().out("knows").or().in("knows")).values("name").to_list()"#
        ),
        r#"["marko", "vadas", "josh"]"#
    );
}

#[test]
fn infix_right_operand_runs_to_the_end_of_the_traversal() {
    // TinkerPop's ConnectiveStrategy: everything after `and()` is the right operand, so the
    // trailing `values("name")` is inside the and() and the vertex comes out.
    assert_eq!(
        run(r#"g.V().has("name", "marko").and().has("age", 29).values("name").next().id"#),
        r#""1""#
    );
    assert_eq!(
        run(
            r#"g.V().filter(__.has("name", "marko").and().has("age", 29)).values("name").to_list()"#
        ),
        r#"["marko"]"#
    );
}

#[test]
fn infix_operands_exclude_the_start_step_and_its_labels() {
    // `as("a")` labels V(), not the left operand: select("a") inside the right operand sees it.
    assert_eq!(
        run(
            r#"g.V().as("a").out("knows").and().out("created").in("created").as("a").to_list().map(|v| v.id)"#
        ),
        r#"["1"]"#
    );
}

// --- dedup(label...) ------------------------------------------------------------------------

#[test]
fn dedup_by_labels_keys_by_the_labelled_objects() {
    assert_eq!(
        run(r#"g.V().as("a").both().as("b").dedup("a", "b").by(T.label).count().next()"#),
        "3"
    );
    assert_eq!(
        run(r#"g.V().as("a").both().as("b").dedup("a").count().next()"#),
        "6"
    );
    // A label that is not on the path filters the traverser.
    assert_eq!(
        run(r#"g.V().as("a").both().dedup("a", "nope").count().next()"#),
        "0"
    );
}

// --- where(label, P) ------------------------------------------------------------------------

#[test]
fn where_with_a_start_label_tests_the_labelled_object() {
    assert_eq!(
        run(
            r#"g.V("1").as("a").out("created").in("created").as("b").where("a", P.neq("b")).values("name").order().to_list()"#
        ),
        r#"["josh", "peter"]"#
    );
    assert_eq!(
        run(
            r#"g.V().as("a").out("created").in("created").as("b").where("a", P.gt("b")).by("age").select("a", "b").by("name").to_list()"#
        ),
        r#"[#{"a": "josh", "b": "marko"}, #{"a": "peter", "b": "josh"}, #{"a": "peter", "b": "marko"}]"#
    );
}

// --- addV(traversal), property(traversal, value) ---------------------------------------------

#[test]
fn add_v_and_property_take_a_traversal() {
    assert_eq!(
        run(r#"g.addV(__.V("1").label()).label().next()"#),
        r#""person""#
    );
    assert_eq!(
        run(
            r#"g.withSideEffect("a", "nick").V("1").property(__.select("a"), "mk").values("nick").next()"#
        ),
        r#""mk""#
    );
    let err = run_err(r#"g.addV(__.constant(1)).next()"#);
    assert!(err.contains("string"), "{err}");
}

#[test]
fn a_failing_add_v_traversal_rolls_back() {
    let graph = GraphSource::tinkerpop_modern();
    let graph = Arc::new(graph);
    let err = crate::script_host::eval_value(
        graph.clone(),
        r#"g.V().addV(__.choose(__.has("age"), __.label(), __.constant(1))).to_list()"#,
    );
    assert!(err.is_err());
    assert_eq!(
        crate::script_host::eval_value(graph, "g.V().count().next()")
            .unwrap()
            .as_int()
            .unwrap(),
        6
    );
}

// --- tail(), union() --------------------------------------------------------------------------

#[test]
fn tail_and_union_without_arguments() {
    assert_eq!(
        run(r#"g.V().values("name").order().tail().to_list()"#),
        r#"["vadas"]"#
    );
    assert_eq!(run(r#"g.union().to_list()"#), "[]");
    assert_eq!(run(r#"g.V().union().count().next()"#), "0");
}

// --- with(WithOptions..) ----------------------------------------------------------------------

#[test]
fn value_map_tokens_option() {
    assert_eq!(
        run(r#"g.V("1").valueMap("name").with(WithOptions.tokens).next()"#),
        r#"#{"id": "1", "label": "person", "name": ["marko"]}"#
    );
    assert_eq!(
        run(r#"g.V("1").valueMap("name").with(WithOptions::tokens, WithOptions::labels).next()"#),
        r#"#{"label": "person", "name": ["marko"]}"#
    );
    assert_eq!(
        run(r#"g.V("1").valueMap("name").with(WithOptions.tokens, WithOptions.none).next()"#),
        r#"#{"name": ["marko"]}"#
    );
}

#[test]
fn index_map_indexer() {
    assert_eq!(
        run(
            r#"g.V().hasLabel("software").values("name").fold().order(Scope.local).index().with(WithOptions.indexer, WithOptions.map).select(Column.keys).next()"#
        ),
        "[0, 1]"
    );
    assert_eq!(
        run(
            r#"g.V().hasLabel("software").values("name").fold().order(Scope.local).index().with(WithOptions.indexer, WithOptions.map).select(Column.values).next()"#
        ),
        r#"["lop", "ripple"]"#
    );
}

#[test]
fn with_option_on_another_step_is_an_invalid_modulator() {
    let err = run_err(r#"g.V().out().with(WithOptions.tokens).to_list()"#);
    assert!(err.contains("with()"), "{err}");
    let err = run_err(r#"g.V().valueMap().with(WithOptions.indexer, WithOptions.map).to_list()"#);
    assert!(err.contains("indexer"), "{err}");
    // `with(key, value)` with a string key stays the execution option.
    assert_eq!(run(r#"g.with("random.seed", 1).V().count().next()"#), "6");
}

// --- UUID(), select(traversal) with a non-string key ------------------------------------------

#[test]
fn uuid_without_arguments_is_random() {
    let a = run(r#"UUID().to_string()"#);
    let b = run(r#"UUID().to_string()"#);
    assert_ne!(a, b);
    assert_eq!(run(r#"type_of(UUID())"#), r#""Uuid""#);
}

#[test]
fn select_traversal_reads_a_non_string_map_key() {
    assert_eq!(
        run(
            r#"g.V().as("a").group("m").by().by(__.bothE().count()).barrier().select("m").select(__.select("a")).to_list()"#
        ),
        "[3, 1, 3, 3, 1, 1]"
    );
}
