//! Predicate composition (`P.and/or/not`, `negate()`): two-valued, left-to-right, short-circuit
//! logic over the comparability core, plus the set semantics over labels and keys, side-effect
//! resolution inside a composite, and the pushdown guard.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run(script: &str) -> rhai::Dynamic {
    eval_value(modern(), script).unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn count(script: &str) -> i64 {
    run(script)
        .as_int()
        .unwrap_or_else(|_| panic!("{script}: not an int"))
}

fn names(script: &str) -> Vec<String> {
    let mut out: Vec<String> = run(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect();
    out.sort();
    out
}

fn failure(script: &str) -> String {
    match eval_value(modern(), script) {
        Ok(v) => panic!("{script}: expected an error, got {v}"),
        Err(e) => e.to_string(),
    }
}

/// `true` / `false` / `NaN` as a predicate over `inject(1.0)`.
fn operand(kind: &str) -> &'static str {
    match kind {
        "true" => "P.eq(1.0)",
        "false" => "P.eq(2.0)",
        _ => "P.lt(NaN)",
    }
}

#[test]
fn truth_table_of_and_or_over_true_false_nan() {
    let kinds = ["true", "false", "error"];
    for left in kinds {
        for right in kinds {
            let l = left == "true";
            let r = right == "true";
            for (op, expected) in [("and", l && r), ("or", l || r)] {
                let script = format!(
                    "let NaN = 0.0 / 0.0; g.inject(1.0).is({}.{op}({})).toList().len()",
                    operand(left),
                    operand(right)
                );
                assert_eq!(count(&script), i64::from(expected), "{left} {op} {right}");
            }
        }
    }
}

#[test]
fn chains_group_left_to_right_as_written() {
    // (a.and(b)).or(c) and (a.or(b)).and(c) differ when a is false and c is true / a true, c false.
    assert_eq!(
        count("g.inject(1).is(P.eq(2).and(P.eq(1)).or(P.eq(1))).toList().len()"),
        1
    );
    assert_eq!(
        count("g.inject(1).is(P.eq(1).or(P.eq(2)).and(P.eq(2))).toList().len()"),
        0
    );
    // a.and(b).and(c) == a.and(b.and(c))
    for (a, b, c) in [(1, 1, 1), (1, 1, 2), (1, 2, 1), (2, 1, 1)] {
        let left = count(&format!(
            "g.inject(1).is(P.eq({a}).and(P.eq({b})).and(P.eq({c}))).toList().len()"
        ));
        let right = count(&format!(
            "g.inject(1).is(P.eq({a}).and(P.eq({b}).and(P.eq({c})))).toList().len()"
        ));
        assert_eq!(left, right, "{a} {b} {c}");
    }
}

#[test]
fn not_and_negate_are_the_complement() {
    assert_eq!(
        count("g.inject(1, 2, 3).is(P.not(P.within(1, 2))).toList().len()"),
        1
    );
    assert_eq!(
        count("g.inject(1, 2, 3).is(P.within(1).negate()).toList().len()"),
        2
    );
    assert_eq!(
        count("g.inject(1, 2, 3).is(P::not(P.within(1, 2))).toList().len()"),
        1
    );
    assert_eq!(
        count("g.inject(1, 2, 3).is(not(P.within(1, 2))).toList().len()"),
        1
    );
    // double negation
    assert_eq!(
        count("g.inject(1, 2, 3).is(P.not(P.not(P.eq(2)))).toList().len()"),
        1
    );
}

#[test]
fn negation_is_a_plain_complement_not_an_opposite_operator() {
    // Documented deviation: `P.not(P.gt(1))` is true for an incomparable value.
    let nan = "let NaN = 0.0 / 0.0; ";
    assert_eq!(
        count(&format!(
            "{nan}g.inject(NaN).is(P.gt(1).negate()).toList().len()"
        )),
        1
    );
    assert_eq!(
        count(&format!(
            "{nan}g.inject(NaN).is(P.lt(1).negate()).toList().len()"
        )),
        1
    );
    assert_eq!(
        count(&format!(
            "{nan}g.inject(NaN).is(P.neq(1).negate()).toList().len()"
        )),
        0
    );
    assert_eq!(
        count(r#"g.inject("foo").is(P.gt(1).negate()).toList().len()"#),
        1
    );
    assert_eq!(
        count(r#"g.inject("foo").is(P.not(P.lt(1))).toList().len()"#),
        1
    );
    assert_eq!(
        count(r#"g.inject("foo").is(P.neq(1).negate()).toList().len()"#),
        0
    );
}

#[test]
fn composite_has_on_the_modern_graph() {
    assert_eq!(
        names(r#"g.V().has("age", P.gt(18).and(P.lt(30)).or(P.gt(35))).values("name").toList()"#),
        ["marko", "vadas"]
    );
    assert_eq!(
        names(r#"g.V().has("age", P.gt(18).and(P.lt(30)).and(P.lt(35))).values("name").toList()"#),
        ["marko", "vadas"]
    );
    assert_eq!(
        names(r#"g.V().has("name", TextP.containing("o").and(P.lt("m"))).values("name").toList()"#),
        ["josh", "lop"]
    );
    assert_eq!(
        names(r#"g.V().has("name", P.lt("m").and(TextP.containing("o"))).values("name").toList()"#),
        ["josh", "lop"]
    );
    assert_eq!(
        names(
            r#"g.V().hasLabel("person").has("age", P.not(P.lte(10).and(P.not(P.between(11, 20)))).and(P.lt(29).or(P.eq(35)))).values("name").toList()"#
        ),
        ["peter", "vadas"]
    );
}

#[test]
fn label_sets_combine_the_set_results() {
    assert_eq!(
        count(r#"g.V().hasLabel(P.not(P.eq("person"))).count().next()"#),
        2
    );
    assert_eq!(
        count(r#"g.V().hasLabel(P.eq("person").or(P.eq("software"))).count().next()"#),
        6
    );
    assert_eq!(
        count(r#"g.V().hasLabel(P.eq("person").and(P.eq("software"))).count().next()"#),
        0
    );
    // A vertex with two labels: set semantics combine the child results.
    let script = r#"
        g.addV(["a", "b"]).toList();
        [
          g.V().hasLabel(P.eq("a").and(P.eq("b"))).count().next(),
          g.V().hasLabel(P.eq("a").and(P.eq("c"))).count().next(),
          g.V().hasLabel(P.not(P.eq("c"))).count().next()
        ]"#;
    let result = run(script).into_array().unwrap();
    let got: Vec<i64> = result.into_iter().map(|v| v.as_int().unwrap()).collect();
    assert_eq!(got[..3], [1, 0, 7]);
}

#[test]
fn where_resolves_side_effects_and_labels_through_the_composite() {
    assert_eq!(
        names(
            r#"g.V("1").out().aggregate("a").out().where(P.not(P.within("a"))).values("name").toList()"#
        ),
        ["ripple"]
    );
    assert_eq!(
        count(r#"g.V().as("a").out().as("d").where(P.eq("a").or(P.eq("d"))).count().next()"#),
        6
    );
    assert_eq!(
        count(r#"g.V().as("a").out().as("d").where(P.eq("a").and(P.eq("d"))).count().next()"#),
        0
    );
    assert_eq!(
        count(r#"g.V().as("a").out().as("d").where(P.neq("a").and(P.neq("d"))).count().next()"#),
        0
    );
}

#[test]
fn path_requirement_is_the_union_of_the_children() {
    let profile =
        run(r#"g.V().as("a").out().as("d").where(P.eq("a").or(P.eq("d"))).count().profile().to_string()"#)
            .to_string();
    assert!(profile.contains("labels(a,d)"), "{profile}");
    assert!(
        profile.contains(r#"where(P.eq("a").or(P.eq("d")))"#),
        "{profile}"
    );
}

#[test]
fn composites_of_a_text_predicate_on_vertices_follow_boolean_algebra() {
    // `containing` on a vertex is false (TinkerPop: a text predicate matches strings only), as
    // `P.eq(1)` on a vertex is; no leaf raises on a stream value, so short-circuiting can never
    // change a result and the composites are plain boolean algebra.
    let text = r#"TextP.containing("a")"#;
    assert_eq!(count(&format!("g.V().is({text}).count().next()")), 0);
    assert_eq!(
        count(&format!("g.V().is(P.neq(1).or({text})).count().next()")),
        6
    );
    assert_eq!(
        count(&format!(
            "g.V().is(P.eq(1).and({text}).negate()).count().next()"
        )),
        6
    );
    assert_eq!(
        count(&format!("g.V().is(P.eq(1).or({text})).count().next()")),
        0
    );
    assert_eq!(
        count(&format!("g.V().is(P.neq(1).and({text})).count().next()")),
        0
    );
}

#[test]
fn composites_are_never_pushed_down() {
    let composite = r#"g.V().has("age", P.gt(27).and(P.lt(33))).values("name").toList()"#;
    let expected = names(composite);
    assert_eq!(expected, ["josh", "marko"]);
    let profile = run(r#"g.V().has("age", P.gt(27).and(P.lt(33))).count().profile().to_string()"#)
        .to_string();
    assert!(
        profile.contains(r#"has("age", P.gt(27).and(P.lt(33)))"#),
        "{profile}"
    );
    assert!(!profile.contains("Optimizer rules applied"), "{profile}");
    let disabled = names(
        r#"g.with("optimizer.disabled", ["source_filter_pushdown", "barrier_pushdown", "has_id_pushdown", "filter_reorder"]).V().has("age", P.gt(27).and(P.lt(33))).values("name").toList()"#,
    );
    assert_eq!(disabled, expected);
}

#[test]
fn rust_api_composition() {
    let graph = GraphSource::tinkerpop_modern();
    let locked = graph.read();
    fn count_of(graph: &Graph, p: P<i32>) -> i64 {
        let locked = graph.read();
        locked
            .traversal()
            .v(None)
            .has_p("age", p)
            .count()
            .to_list()
            .unwrap()[0]
            .as_i64()
            .unwrap()
    }
    assert_eq!(count_of(&graph, P::Gt(27).and(P::Lt(33))), 2);
    assert_eq!(count_of(&graph, P::Lt(28).or(P::Gt(33))), 2);
    assert_eq!(count_of(&graph, P::Gt(29).negate()), 2);
    assert_eq!(count_of(&graph, P::not(P::Gt(29))), 2);
    let text = locked
        .traversal()
        .v(None)
        .has_p("name", P::Containing("o").and(P::Lt("m")))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(text[0].as_i64(), Some(2));
}

#[test]
fn render_shows_the_composite() {
    let profile =
        run(r#"g.V().is(P.gt(1).and(P.lt(5).negate())).count().profile().to_string()"#).to_string();
    assert!(
        profile.contains("is(P.gt(1).and(P.not(P.lt(5))))"),
        "{profile}"
    );
}

#[test]
fn a_non_predicate_operand_fails_with_help() {
    for script in [
        r#"P.gt(1).and(__.out())"#,
        r#"P.gt(1).or(5)"#,
        r#"P.not("x")"#,
        r#"P::not(__.out())"#,
    ] {
        let error = failure(script);
        assert!(error.contains("Argument mismatch"), "{script}: {error}");
        assert!(
            error.contains("P.gt(18).and(P.lt(30))"),
            "{script}: {error}"
        );
        assert!(
            error.contains("P.not(P.within(\"a\"))"),
            "{script}: {error}"
        );
        assert!(error.contains("negate()"), "{script}: {error}");
        assert!(error.contains("__.and("), "{script}: {error}");
    }
}

/// `where(P)` against path labels: elements compare by identity for `eq`/`neq` and are not
/// ordered; with `by()`, each label operand is projected through the ring, and `and`/`or`/`not`
/// combine the leaves left to right.
mod where_against_labels {
    use super::*;

    #[test]
    fn elements_compare_by_identity_and_are_not_ordered() {
        // no vertex is its own neighbour; every two-hop walk returns to its start 12 times
        assert_eq!(
            count(r#"g.V().as("a").both().where(P.neq("a")).count().next()"#),
            12
        );
        assert_eq!(
            count(r#"g.V().as("a").both().both().where(P.eq("a")).count().next()"#),
            12
        );
        assert_eq!(
            count(r#"g.V().as("a").both().where(P.lt("a")).count().next()"#),
            0
        );
        assert_eq!(
            count(r#"g.V().as("a").both().where(P.gte("a")).count().next()"#),
            0
        );
    }

    #[test]
    fn by_projects_the_current_and_the_labelled_object() {
        // marko (29) knows vadas (27) and josh (32)
        assert_eq!(
            names(
                r#"g.V("1").as("a").out("knows").where(P.lt("a")).by("age").values("name").toList()"#
            ),
            ["vadas"]
        );
        // lop has no age: the unproductive by() drops it
        assert_eq!(
            names(r#"g.V("1").as("a").out().where(P.gt("a")).by("age").values("name").toList()"#),
            ["josh"]
        );
        assert_eq!(
            names(
                r#"g.V("1").as("a").out("knows").where(P.not(P.lt("a"))).by("age").values("name").toList()"#
            ),
            ["josh"]
        );
    }

    #[test]
    fn composed_predicates_walk_the_by_ring() {
        assert!(
            names(r#"g.V("1").as("a").out("knows").where(P.lt("a").and(P.gt("a"))).by("age").values("name").toList()"#)
                .is_empty()
        );
        assert_eq!(
            names(
                r#"g.V("1").as("a").out("knows").where(P.lt("a").or(P.gt("a"))).by("age").values("name").toList()"#
            ),
            ["josh", "vadas"]
        );
        // a literal leaf projects only the current object
        assert_eq!(
            names(
                r#"g.V("1").as("a").out().where(P.lt("a").or(P.gt(30))).by("age").values("name").toList()"#
            ),
            ["josh", "vadas"]
        );
        // the second leaf names `b` (the current vertex itself), so `neq` fails for everyone
        assert!(
            names(r#"g.V("1").as("a").out("knows").as("b").where(P.lt("a").and(P.neq("b"))).by("age").by("age").values("name").toList()"#)
                .is_empty()
        );
    }
}
