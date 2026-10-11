//! Behavioural errors that used to be silent: `length()` of a non-string, `asBool()`/`asString()`
//! of null, a second `by()` on single-`by()` steps, `inject()` as a `repeat()` body step.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn error(script: &str) -> String {
    eval_value(modern(), script).expect_err(script).to_string()
}

#[test]
fn length_accepts_strings_only() {
    let graph = modern();
    let ok = eval_value(
        graph.clone(),
        r#"g.V().values("name").length().sum().next()"#,
    )
    .unwrap();
    assert_eq!(ok.as_int().unwrap(), 4 + 3 + 5 + 5 + 6 + 5);
    for script in [
        r#"g.inject(["a", "b"]).length()"#,
        r#"g.V().length()"#,
        r#"g.E().length()"#,
        r#"g.V("1").out().path().length()"#,
        r#"g.V().values("name").fold().length()"#,
        r#"g.V().values("age").length()"#,
        r#"g.V().valueMap().length()"#,
    ] {
        let message = error(&format!("{script}.toList()"));
        assert!(message.contains("Cast exception"), "{script}: {message}");
    }
    // Null stays null; the local scope maps the strings of a list.
    let local = eval_value(
        graph,
        r#"g.V().values("name").order().fold().length(Scope.local).next()"#,
    )
    .unwrap();
    assert_eq!(format!("{local:?}"), "[4, 3, 5, 5, 6, 5]");
}

#[test]
fn as_bool_and_as_string_reject_null_but_as_number_keeps_it() {
    for script in [
        "g.inject(()).asBool()",
        "g.inject(()).as_bool()",
        "g.inject(1, ()).asString()",
        "g.inject(()).as_string()",
    ] {
        let message = error(&format!("{script}.toList()"));
        assert!(
            message.contains("Cast exception") && message.contains("null"),
            "{script}: {message}"
        );
    }
    let kept = eval_value(modern(), "g.inject(()).asNumber().toList()").unwrap();
    assert_eq!(format!("{kept:?}"), "[()]");
    let still = eval_value(modern(), "g.inject(()).cast(GType.STRING).toList()");
    assert!(still.is_ok(), "{still:?}");
}

#[test]
fn a_second_by_is_rejected_where_only_one_is_taken() {
    for (script, step) in [
        (
            r#"g.V().aggregate("x").by("name").by("age").cap("x")"#,
            "aggregate",
        ),
        (
            r#"g.V().local(__.aggregate("x").by("name").by("age")).cap("x")"#,
            "aggregate",
        ),
        (r#"g.V().store("x").by("name").by("age").cap("x")"#, "store"),
        (
            r#"g.V().out("created").groupCount().by("name").by("age")"#,
            "group_count",
        ),
        (
            r#"g.V().valueMap("name", "age").by(__.unfold()).by(__.unfold())"#,
            "value_map",
        ),
    ] {
        let message = error(&format!("{script}.toList()"));
        assert!(
            message.contains(&format!("`{step}()` cannot use its by() #2")),
            "{script}: {message}"
        );
    }
    // group() keeps its two by(), and one by() stays fine everywhere.
    assert!(eval_value(modern(), r#"g.V().group().by("name").by("age").toList()"#).is_ok());
    assert!(
        eval_value(
            modern(),
            r#"g.V().aggregate("x").by("name").cap("x").toList()"#
        )
        .is_ok()
    );
    assert!(
        eval_value(
            modern(),
            r#"g.V().valueMap("name").by(__.unfold()).toList()"#
        )
        .is_ok()
    );
}

#[test]
fn inject_is_forbidden_as_a_direct_repeat_body_step() {
    let message = error(r#"g.V().repeat(__.inject("y")).times(2).toList()"#);
    assert!(
        message.contains("inject() is not allowed as a step of a repeat() body"),
        "{message}"
    );
    let named = error(r#"g.V().repeat("a", __.out().inject("y")).times(2).toList()"#);
    assert!(named.contains("inject() is not allowed"), "{named}");
    // A nested inject, or one before the loop, is fine.
    let nested = eval_value(
        modern(),
        r#"g.V("1").repeat(__.union(__.identity(), __.inject("y"))).times(1).count().next()"#,
    );
    assert!(nested.is_ok(), "{nested:?}");
    assert!(
        eval_value(
            modern(),
            r#"g.inject("y").repeat(__.identity()).times(2).toList()"#
        )
        .is_ok()
    );
}
