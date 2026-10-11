//! Cross-cutting behaviour of the wave-3 steps: `discard()` (the old `none()`), `none(P)`,
//! `map()`, `flatMap()`, `fail()`, `index()` and the `T.key`/`T.value`, `P.typeOf(name)` tokens,
//! with transactions, sacks, stored list properties and the DSL spellings.
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

#[test]
fn discard_is_the_none_branch_of_choose() {
    assert_eq!(
        run(
            r#"g.V().hasLabel("person").choose(__.values("age")).option(P.between(26, 30), __.values("name")).option(Pick.none, __.discard()).to_list()"#
        ),
        r#"["marko", "vadas"]"#
    );
    // The old filter-everything `none()` no longer exists: `none` takes a predicate.
    let err = run_err(r#"g.V().none().to_list()"#);
    assert!(err.contains("none"), "{err}");
}

#[test]
fn none_reads_a_stored_list_property() {
    let graph = GraphSource::tinkerpop_modern();
    {
        let mut write = graph.write();
        write
            .traversal_mut()
            .v("1")
            .property("scores", vec![1i64, 5, 9])
            .next()
            .unwrap();
    }
    assert_eq!(
        run_on(
            graph,
            r#"g.V("1").values("scores").none(P.gt(10)).count().next()"#
        ),
        "1"
    );
}

#[test]
fn map_and_flat_map_keep_the_parent_sack() {
    assert_eq!(
        run(r#"g.withSack(1).V("1").out().map(__.values("name")).sack().to_list()"#),
        "[1, 1, 1]"
    );
    assert_eq!(
        run(r#"g.withSack(2).V("1").flatMap(__.out()).sack().to_list()"#),
        "[2, 2, 2]"
    );
}

#[test]
fn flat_map_has_both_spellings_and_with_path_changes_nothing() {
    assert_eq!(
        run(r#"g.V("1").flat_map(__.out()).count().next()"#),
        run(r#"g.withPath().V("1").flatMap(__.out()).count().next()"#)
    );
    assert_eq!(
        run(r#"g.with_path().V("1").as("a").out().map(__.select("a").values("name")).to_list()"#),
        r#"["marko", "marko", "marko"]"#
    );
}

/// A traversal that wrote to the graph before reaching `fail()` is rolled back as a whole.
#[test]
fn fail_rolls_back_a_mutating_traversal() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let err = crate::script_host::eval_value(
        graph.clone(),
        r#"g.addV("temp").property("name", "t").fail("abort").to_list()"#,
    )
    .expect_err("fail() must abort");
    assert!(err.to_string().contains("abort"), "{err}");
    let count =
        crate::script_host::eval_value(graph, r#"g.V().hasLabel("temp").count().next()"#).unwrap();
    assert_eq!(format!("{count:?}"), "0");
}

#[test]
fn index_pairs_compose_with_unfold_and_order() {
    assert_eq!(
        run(
            r#"g.V().hasLabel("software").values("name").fold().order(Scope.local).index().unfold().order().by(__.tail(Scope.local, 1)).to_list()"#
        ),
        r#"[["lop", 0], ["ripple", 1]]"#
    );
}

#[test]
fn t_key_and_t_value_in_dedup_group_and_order() {
    assert_eq!(
        run(r#"g.V().properties().dedup().by(T.key).count().next()"#),
        "3"
    );
    assert_eq!(
        run(r#"g.V().properties("lang").dedup().by(T.value).count().next()"#),
        "1"
    );
    assert_eq!(
        run(r#"g.V().properties().group().by(T.key).by(__.count()).select("lang").next()"#),
        "2"
    );
}

#[test]
fn type_of_takes_a_type_name() {
    assert_eq!(
        run(r#"g.V().values("name").is(P.typeOf("String")).count().next()"#),
        "6"
    );
    assert_eq!(
        run(r#"g.V().values("age").is(typeOf("Integer")).count().next()"#),
        "4"
    );
    let err = run_err(r#"g.V().values("age").is(P.typeOf("non-registered-Name")).to_list()"#);
    assert!(err.contains("unknown type name"), "{err}");
}
