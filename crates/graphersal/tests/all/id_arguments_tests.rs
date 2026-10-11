//! `V()`, `E()` and `hasId()` take ids, elements (as returned by `next()`/`toList()`) and lists of
//! them, in both Rhai spellings; any other map is an argument mismatch.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn count(graph: &Arc<Graph>, script: &str) -> i64 {
    eval_value(graph.clone(), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .as_int()
        .unwrap_or_else(|_| panic!("{script}: not an int"))
}

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

#[test]
fn an_edge_is_an_id_argument() {
    let graph = modern();
    let pre = r#"let e1 = g.E("1").next(); let e2 = g.E("2").next();"#;
    assert_eq!(count(&graph, &format!("{pre} g.E(e1).count().next()")), 1);
    assert_eq!(
        count(&graph, &format!("{pre} g.E(e1, e2).count().next()")),
        2
    );
    assert_eq!(
        count(&graph, &format!("{pre} g.E([e1, e2]).count().next()")),
        2
    );
    assert_eq!(
        count(&graph, &format!("{pre} g.e([e1, e2]).count().next()")),
        2
    );
    assert_eq!(
        count(&graph, &format!("{pre} g.E([[e1], e2]).count().next()")),
        2
    );
}

#[test]
fn a_vertex_is_an_id_argument() {
    let graph = modern();
    let pre = r#"let v1 = g.V("1").next(); let v2 = g.V("2").next();"#;
    assert_eq!(count(&graph, &format!("{pre} g.V(v1).count().next()")), 1);
    assert_eq!(
        count(&graph, &format!("{pre} g.v([v1, v2]).count().next()")),
        2
    );
    assert_eq!(
        count(&graph, &format!("{pre} g.V(v1, v2).count().next()")),
        2
    );
}

#[test]
fn has_id_takes_elements_and_lists() {
    let graph = modern();
    let pre = r#"let v1 = g.V("1").next(); let v2 = g.V("2").next();"#;
    assert_eq!(
        count(&graph, &format!("{pre} g.V().hasId(v1).count().next()")),
        1
    );
    assert_eq!(
        count(
            &graph,
            &format!("{pre} g.V().has_id([v1, v2]).count().next()")
        ),
        2
    );
    assert_eq!(
        count(&graph, &format!("{pre} g.V().hasId(v1, v2).count().next()")),
        2
    );
    assert_eq!(
        count(&graph, r#"g.V().hasId(["1", "2", "3"]).count().next()"#),
        3
    );
}

#[test]
fn a_null_id_matches_nothing() {
    let graph = modern();
    assert_eq!(count(&graph, "g.E(()).count().next()"), 0);
    assert_eq!(count(&graph, "g.V(()).count().next()"), 0);
}

#[test]
fn plain_ids_still_work() {
    let graph = modern();
    assert_eq!(count(&graph, r#"g.E("1").count().next()"#), 1);
    assert_eq!(count(&graph, "g.V(1).count().next()"), 1);
    assert_eq!(count(&graph, r#"g.V(["1", "2"]).count().next()"#), 2);
}

#[test]
fn a_map_that_is_not_an_element_is_rejected() {
    let graph = modern();
    for script in [
        "g.E(#{a: 1}).count().next()",
        "g.V(#{a: 1}).count().next()",
        "g.V().hasId(#{a: 1}).count().next()",
        "g.V().has_id([#{a: 1}]).count().next()",
    ] {
        let err = eval_value(graph.clone(), script)
            .expect_err(script)
            .to_string();
        assert!(
            err.contains("Argument mismatch") && err.contains("without an `id` key"),
            "{script}: {err}"
        );
        assert!(err.contains("take ids"), "help missing: {err}");
    }
}
