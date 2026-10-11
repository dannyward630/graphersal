//! `choose(T.label)`, `choose(T.id)`, `branch(T.label)`, `choose(P, onTrue[, onFalse])` and the
//! two-argument `choose(traversal, onTrue)`: sugar over the traversal forms.
//!
//! Source: the TinkerPop 3.8.2 feature file `Choose.feature` (`choose(Function)` with `T`,
//! `choose(Predicate, ..)`, `choose(Traversal, Traversal)`).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn run(script: &str) -> String {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let value = eval_value(graph, script).unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

fn error_of(script: &str) -> String {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    match eval_value(graph, script) {
        Ok(v) => panic!("`{script}` should fail, got {v:?}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn choose_t_label_equals_choose_label_traversal() {
    let sugar = run(
        r#"g.V().choose(T.label).option("person", __.out("knows").values("name")).option("bleep", __.constant("bleep")).to_list()"#,
    );
    let expanded = run(
        r#"g.V().choose(__.label()).option("person", __.out("knows").values("name")).option("bleep", __.constant("bleep")).to_list()"#,
    );
    assert_eq!(sugar, expanded);
    assert!(sugar.contains("vadas"), "{sugar}");
}

#[test]
fn choose_t_id_equals_choose_id_traversal() {
    let sugar = run(
        r#"g.V().choose(T.id).option("1", __.constant("one")).option(Pick.none, __.constant("other")).to_list()"#,
    );
    let expanded = run(
        r#"g.V().choose(__.id()).option("1", __.constant("one")).option(Pick.none, __.constant("other")).to_list()"#,
    );
    assert_eq!(sugar, expanded);
    assert!(sugar.contains("one") && sugar.contains("other"), "{sugar}");
}

#[test]
fn branch_takes_t_label() {
    let sugar = run(
        r#"g.V().branch(T.label).option("person", __.constant(1)).option("software", __.constant(2)).to_list()"#,
    );
    let expanded = run(
        r#"g.V().branch(__.label()).option("person", __.constant(1)).option("software", __.constant(2)).to_list()"#,
    );
    assert_eq!(sugar, expanded);
}

#[test]
fn choose_predicate_with_both_branches() {
    let r = run(
        r#"g.V().hasLabel("person").values("age").choose(P.eq(29), __.constant("matched"), __.constant("other")).to_list()"#,
    );
    assert_eq!(r.matches("matched").count(), 1, "{r}");
    assert_eq!(r.matches("other").count(), 3, "{r}");
}

#[test]
fn choose_predicate_without_false_branch_passes_the_value_through() {
    let r = run(
        r#"g.V().hasLabel("person").values("age").choose(P.gt(30), __.constant("old")).to_list()"#,
    );
    assert_eq!(r.matches("old").count(), 2, "{r}");
    assert!(r.contains("29") && r.contains("27"), "{r}");
}

#[test]
fn choose_predicate_inside_local() {
    let r = run(
        r#"g.V().hasLabel("person").values("age").fold().local(__.unfold().choose(P.between(28, 33), __.constant("mid"), __.constant("edge")).fold()).next()"#,
    );
    assert_eq!(r.matches("mid").count(), 2, "{r}");
    assert_eq!(r.matches("edge").count(), 2, "{r}");
}

#[test]
fn choose_two_traversals() {
    let r =
        run(r#"g.V().hasLabel("person").choose(__.values("age"), __.values("name")).to_list()"#);
    assert_eq!(r.matches("marko").count(), 1, "{r}");
    let r = run(r#"g.V().choose(__.has("name", "vadas"), __.values("name")).count().next()"#);
    assert_eq!(r, "6");
}

#[test]
fn choose_two_arguments_keeps_labels_for_paths() {
    let r = run(
        r#"g.V().hasLabel("person").as("p1").choose(__.out_e("knows"), __.out("knows")).as("p2").select("p1", "p2").by("name").to_list()"#,
    );
    assert!(r.contains("marko") && r.contains("vadas"), "{r}");
}

#[test]
fn choose_with_a_predicate_in_the_one_argument_form_is_rejected() {
    let e = error_of("g.V().choose(P.eq(1))");
    assert!(e.contains("choose"), "{e}");
}

#[test]
fn rejected_selectors_name_the_accepted_forms() {
    for script in [
        r#"g.V().choose("x")"#,
        r#"g.V().choose(P.eq(1))"#,
        r#"g.V().choose(T.key)"#,
        r#"g.V().choose(1, __.constant(1))"#,
    ] {
        let e = error_of(script);
        if script.contains("T.key") {
            continue; // T.key is not a DSL token at all: the engine reports an unknown property
        }
        assert!(e.contains("choose"), "{script}: {e}");
    }
    let e = error_of(r#"g.V().choose("x")"#);
    assert!(e.contains("T.label") && e.contains("P.gt(30)"), "{e}");
    let e = error_of(r#"g.V().branch(1)"#);
    assert!(e.contains("T.label"), "{e}");
}

#[test]
fn a_choose_with_a_mutating_branch_is_mutating() {
    let graph = Arc::new(GraphSource::empty());
    let _ = eval_value(
        graph.clone(),
        r#"g.inject(1).choose(P.eq(1), __.addV("a"), __.identity()).to_list()"#,
    )
    .unwrap();
    assert_eq!(run_on(&graph, "g.V().count().next()"), "1");
}

fn run_on(graph: &Arc<Graph>, script: &str) -> String {
    format!("{:?}", eval_value(graph.clone(), script).unwrap())
}
