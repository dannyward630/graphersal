//! `length()` counts characters (Unicode scalar values), not UTF-8 bytes.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn eval(script: &str) -> String {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    format!("{:?}", eval_value(graph, script).expect(script))
}

#[test]
fn length_counts_characters() {
    assert_eq!(
        eval(r#"g.inject("é", "日本語", "😀", "abc").length().toList()"#),
        "[1, 3, 1, 3]"
    );
}

#[test]
fn length_local_counts_characters() {
    assert_eq!(
        eval(r#"g.inject(["é", "日本語", "😀", ()]).length(Scope.local).next()"#),
        "[1, 3, 1, ()]"
    );
}

#[test]
fn string_steps_keep_multibyte_text_intact() {
    assert_eq!(
        eval(r#"g.inject("日本語").substring(1, 2).next()"#),
        r#""本""#
    );
    assert_eq!(eval(r#"g.inject("  é日  ").trim().length().next()"#), "2");
    assert_eq!(
        eval(r#"g.inject("éé").replace("é", "ee").length().next()"#),
        "4"
    );
}
