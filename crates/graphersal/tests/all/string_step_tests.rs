//! Cross-cutting behaviour of the string steps `concat`, `format`, `split`, `reverse` and the
//! `TextP.regex`/`notRegex` predicates: path recording for the labels and child traversals they
//! read, bulk and sack propagation, the fused `count()`, the DSL spellings and the regex dialect.
//! The per-step semantics are tested in each step's module.

#![cfg(feature = "script")]

use std::sync::Arc;

use graphersal::prelude::*;

use crate::script_host::eval_value;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn sorted_strings(values: &[ElementProperty]) -> Vec<String> {
    let mut out: Vec<String> = values
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    out.sort();
    out
}

/// `format()` placeholders that name path labels make the step record those labels, with the
/// path analysis on and off.
#[test]
fn format_reads_path_labels_with_and_without_path_analysis() {
    let graph = modern();
    let lock = graph.read();
    for analysis in [true, false] {
        let result = lock
            .traversal()
            .with("path.analysis", analysis)
            .v(None)
            .as_("s")
            .label()
            .as_("subject")
            .select("s")
            .out_e(None)
            .as_("p")
            .label()
            .as_("predicate")
            .select("p")
            .in_v()
            .label()
            .as_("object")
            .format("%{subject} %{predicate} %{object}")
            .to_list()
            .unwrap();
        assert_eq!(
            sorted_strings(&result),
            vec![
                "person created software",
                "person created software",
                "person created software",
                "person created software",
                "person knows person",
                "person knows person",
            ],
            "path.analysis = {analysis}"
        );
    }
}

/// A `concat()` child traversal reads the path of the incoming traverser.
#[test]
fn concat_child_reads_the_incoming_path() {
    let graph = modern();
    let lock = graph.read();
    let result = lock
        .traversal()
        .v(None)
        .has_label("person")
        .values("name")
        .as_("a")
        .constant("Mr.")
        .concat_traversal(vec![__::select("a")])
        .to_list()
        .unwrap();
    assert_eq!(
        sorted_strings(&result),
        vec!["Mr.josh", "Mr.marko", "Mr.peter", "Mr.vadas"]
    );
}

/// A merged (bulk > 1) traverser keeps its bulk through each step: the results are the same
/// multiset with and without merging.
#[test]
fn bulk_is_kept() {
    let graph = modern();
    let lock = graph.read();
    for step in ["concat", "format", "split", "reverse"] {
        let count = |merge: bool| {
            let t = lock
                .traversal()
                .with("bulk.merge", merge)
                .v(None)
                .both(None)
                .barrier()
                .values("name");
            let t = match step {
                "concat" => t.concat(vec!["!"]),
                "format" => t.format("<%{_}>"),
                "split" => t.split("a"),
                _ => t.reverse(),
            };
            t.count().next().unwrap().unwrap()
        };
        assert_eq!(count(true).as_i64(), Some(12), "{step}");
        assert_eq!(count(false).as_i64(), Some(12), "{step}");
    }
}

/// The sack of the incoming traverser travels with the result.
#[test]
fn the_sack_is_kept() {
    for step in [
        r#"concat("x")"#,
        r#"format("%{_}")"#,
        r#"split("a")"#,
        r#"reverse()"#,
    ] {
        let result = eval_value(
            modern(),
            &format!(r#"g.withSack(7).V("1").values("name").{step}.sack().toList()"#),
        )
        .unwrap_or_else(|e| panic!("{step}: {e}"));
        assert_eq!(result.to_string(), "[7]", "{step}");
    }
}

/// A fused `count()` after `concat()`/`split()` still runs them, so their errors are raised;
/// after `format()` it still filters.
#[test]
fn a_fused_count_does_not_skip_the_step() {
    for step in [r#"concat("x")"#, r#"split("a")"#] {
        let err = eval_value(
            modern(),
            &format!(r#"g.V().values("age").{step}.count().next()"#),
        )
        .expect_err(step)
        .to_string();
        assert!(err.contains("string"), "{step}: {err}");
    }
    let filtered = eval_value(modern(), r#"g.V().format("%{age}").count().next()"#).unwrap();
    assert_eq!(filtered.as_int().unwrap(), 4);
}

/// Every name has one spelling (snake and camel case coincide); `split` and `replace` take a
/// `null` (`()`) argument.
#[test]
fn dsl_forms() {
    for (query, expected) in [
        (r#"g.inject("a").concat().toList()"#, r#"["a"]"#),
        (r#"g.inject("a").concat("b").toList()"#, r#"["ab"]"#),
        (
            r#"g.inject("a").concat("b", (), "c").toList()"#,
            r#"["abc"]"#,
        ),
        (
            r#"g.inject("a").concat(__.constant("b"), __.constant("c")).toList()"#,
            r#"["abc"]"#,
        ),
        (r#"__.inject("a").concat("b").toList()"#, r#"["ab"]"#),
        (r#"g.inject("a b").split(()).toList()"#, r#"[["a", "b"]]"#),
        (
            r#"g.inject(["a-b", "c"]).split(Scope::local, "-").toList()"#,
            r#"[[["a", "b"], ["c"]]]"#,
        ),
        (r#"g.inject("ab").reverse().toList()"#, r#"["ba"]"#),
        (r#"g.inject("ab").format("[%{_}]").toList()"#, r#"["[ab]"]"#),
        (r#"g.inject("ab").replace((), "x").toList()"#, r#"["ab"]"#),
        (
            r#"g.inject(["ab"]).replace(Scope.local, "a", ()).toList()"#,
            r#"[["ab"]]"#,
        ),
    ] {
        let value = eval_value(modern(), query).unwrap_or_else(|e| panic!("{query}: {e}"));
        assert_eq!(value.to_string(), expected, "{query}");
    }
    let err = eval_value(modern(), r#"g.inject("a").concat("b", __.constant("c"))"#)
        .expect_err("mixed arguments")
        .to_string();
    assert!(err.contains("only strings or only traversals"), "{err}");
    let err = eval_value(modern(), r#"g.inject("a").split(1)"#)
        .expect_err("a number separator")
        .to_string();
    assert!(err.contains("a string or ()"), "{err}");
}

/// `TextP.regex`/`notRegex` in every spelling, unanchored, on strings only.
#[test]
fn regex_predicates() {
    for (query, expected) in [
        (
            r#"g.V().has("name", TextP.regex("^mar")).values("name").toList()"#,
            r#"["marko"]"#,
        ),
        (
            r#"g.V().has("name", TextP::regex("rip")).values("name").toList()"#,
            r#"["ripple"]"#,
        ),
        (
            r#"g.V().has("name", regex("^(?i)LOP$")).values("name").toList()"#,
            r#"["lop"]"#,
        ),
        (
            r#"g.V().has("name", TextP.notRegex("[aeiou]{1}[a-z]*o")).count().next()"#,
            "5",
        ),
        (
            r#"g.V().has("name", TextP.not_regex("^mar")).count().next()"#,
            "5",
        ),
        (
            r#"g.V().values("age").is(TextP.regex("2")).count().next()"#,
            "0",
        ),
        (
            r#"g.V().values("age").is(TextP.notRegex("2")).count().next()"#,
            "4",
        ),
        (
            r#"g.V().has("name", TextP.regex("^m").or(TextP.regex("^p"))).count().next()"#,
            "2",
        ),
    ] {
        let value = eval_value(modern(), query).unwrap_or_else(|e| panic!("{query}: {e}"));
        assert_eq!(value.to_string(), expected, "{query}");
    }
}

/// An invalid expression fails when the predicate is built, with a help naming the dialect.
#[test]
fn an_invalid_regex_is_reported_with_the_dialect() {
    let err = eval_value(modern(), r#"g.V().has("name", TextP.regex("(?<=a)b"))"#)
        .expect_err("look-behind is not supported")
        .to_string();
    assert!(
        err.contains("Invalid regular expression '(?<=a)b'"),
        "{err}"
    );
    assert!(
        err.contains(
            "syntax of the Rust `regex` crate (https://docs.rs/regex/latest/regex/#syntax)"
        ),
        "{err}"
    );
    let rust = P::regex("(unclosed").expect_err("unclosed group");
    assert!(rust.to_string().contains("unclosed group"), "{rust}");
}

/// The Rust API of the regex predicates.
#[test]
fn regex_predicates_rust_api() {
    let graph = modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v(None)
        .has_p("name", P::not_regex("o").unwrap())
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(sorted_strings(&names), vec!["peter", "ripple", "vadas"]);
}
