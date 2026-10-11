//! Vertices and edges order by id, as TinkerPop orders elements (`Orderability.feature`).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn ids(script: &str) -> Vec<String> {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
        .into_array()
        .unwrap()
        .into_iter()
        .map(|v| v.to_string())
        .collect()
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

#[test]
fn edges_order_by_id() {
    let unordered = ids("g.E().id().toList()");
    let ascending = ids("g.E().order().id().toList()");
    assert_eq!(ascending, sorted(unordered.clone()));
    assert_eq!(ids("g.E().order().by(Order.asc).id().toList()"), ascending);
    let mut descending = ascending.clone();
    descending.reverse();
    assert_eq!(
        ids("g.E().order().by(Order.desc).id().toList()"),
        descending
    );
    assert_eq!(
        ids("g.V().out().outE().order().by(Order.desc).id().toList()").len(),
        2
    );
}

#[test]
fn edges_with_the_same_endpoints_order_by_id_not_by_target() {
    // Edges 11 -> 10 by target vertex were the old order; ids decide now.
    let asc = ids("g.V().out().outE().order().by(Order.asc).id().toList()");
    assert_eq!(asc, sorted(asc.clone()));
    let desc = ids("g.V().out().outE().order().by(Order.desc).id().toList()");
    let mut reversed = asc.clone();
    reversed.reverse();
    assert_eq!(desc, reversed);
}

#[test]
fn vertices_order_by_id() {
    assert_eq!(
        ids("g.V().order().id().toList()"),
        sorted(ids("g.V().id().toList()"))
    );
    assert_eq!(
        ids("g.V().out().out().order().by(Order.asc).id().toList()"),
        vec!["3", "5"]
    );
}

#[test]
fn paths_order_by_their_elements() {
    let ordered = ids(
        r#"g.V().out().outE().as("head").path().order().by(Order.asc).select("head").id().toList()"#,
    );
    let reversed = ids(
        r#"g.V().out().outE().as("head").path().order().by(Order.desc).select("head").id().toList()"#,
    );
    let mut flipped = ordered.clone();
    flipped.reverse();
    assert_eq!(reversed, flipped);
    assert_eq!(ordered, sorted(ordered.clone()));
}
