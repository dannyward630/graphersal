//! Properties named `id`, `label` and `labels` are ordinary properties (TinkerPop allows the keys
//! unless `ReservedKeysVerificationStrategy` is set). Reading them returns the stored property,
//! never the element's id or label; `id()`, `label()` and `labels()` return the element's own
//! values, never such a property (issue #1).
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

/// Runs `script` on a fresh modern graph and renders every result of the final list as text.
fn list(script: &str) -> Vec<String> {
    let value = eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"));
    value
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|item| item.to_string())
        .collect()
}

/// A new `person` vertex with the properties `label`, `id` and `labels` (in the DSL).
const VERTEX: &str = r#"g.add_v("person").property("name", "Bill").property("label", "CUSTOM")
    .property("id", "X").property("labels", "L")"#;

/// A new `knows` edge from vertex 1 to vertex 2 with the properties `label` and `id`.
const EDGE: &str = r#"g.v("1").add_e("knows").to(__.v("2")).property("label", "CUSTOM")
    .property("id", "X")"#;

fn on_vertex(steps: &str) -> Vec<String> {
    list(&format!("{VERTEX}{steps}"))
}

fn on_edge(steps: &str) -> Vec<String> {
    list(&format!("{EDGE}{steps}"))
}

#[test]
fn values_read_the_property_not_the_element() {
    assert_eq!(on_vertex(r#".values("label").toList()"#), ["CUSTOM"]);
    assert_eq!(on_vertex(r#".values("id").toList()"#), ["X"]);
    assert_eq!(on_vertex(r#".values("labels").toList()"#), ["L"]);
    assert_eq!(
        on_vertex(".values().toList()"),
        ["Bill", "CUSTOM", "X", "L"]
    );
    assert_eq!(on_edge(r#".values("label").toList()"#), ["CUSTOM"]);
    assert_eq!(on_edge(r#".values("id").toList()"#), ["X"]);
    assert_eq!(on_edge(".values().toList()"), ["CUSTOM", "X"]);
}

#[test]
fn property_elements_read_the_property() {
    assert_eq!(
        on_vertex(".properties().value().toList()"),
        ["Bill", "CUSTOM", "X", "L"]
    );
    assert_eq!(
        on_vertex(".properties().key().toList()"),
        ["name", "label", "id", "labels"]
    );
    assert_eq!(
        on_vertex(r#".properties("label").value().toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(on_edge(r#".properties("id").value().toList()"#), ["X"]);
}

#[test]
fn id_label_and_labels_read_the_element_not_the_property() {
    assert_eq!(on_vertex(".label().toList()"), ["person"]);
    assert_eq!(on_vertex(".labels().toList()"), ["[\"person\"]"]);
    let id = on_vertex(".id().toList()");
    assert_ne!(id, ["X"], "id() must not read the property named id");
    assert_eq!(on_edge(".label().toList()"), ["knows"]);
    assert_ne!(on_edge(".id().toList()"), ["X"]);
}

#[test]
fn reading_steps_after_values_see_the_property() {
    // Predicates, string steps, casts and type checks read the value the handle stands for.
    assert_eq!(
        on_vertex(r#".values("label").is("CUSTOM").toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(
        on_vertex(r#".values("label").is("person").toList()"#).len(),
        0
    );
    assert_eq!(on_vertex(r#".values("label").length().toList()"#), ["6"]);
    assert_eq!(
        on_vertex(r#".values("label").toLower().toList()"#),
        ["custom"]
    );
    assert_eq!(on_vertex(r#".values("id").asString().toList()"#), ["X"]);
    assert_eq!(
        on_vertex(r#".values("label").is(P.typeOf(GType.STRING)).toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(
        on_edge(r#".values("label").is("CUSTOM").toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(on_edge(r#".values("id").length().toList()"#), ["1"]);
}

#[test]
fn reading_steps_after_label_and_id_see_the_element() {
    assert_eq!(on_vertex(".label().is(\"person\").toList()"), ["person"]);
    assert_eq!(on_vertex(".label().length().toList()"), ["6"]);
    assert_eq!(on_vertex(".label().asString().toList()"), ["person"]);
    assert_eq!(on_edge(".label().is(\"knows\").toList()"), ["knows"]);
    // The modern graph's vertex "1" has a one-character id.
    assert_eq!(list(r#"g.v("1").id().length().toList()"#), ["1"]);
}

#[test]
fn a_property_value_and_the_label_are_distinct_values() {
    // `label()` and `values("label")` of one vertex are two different values: dedup keeps both.
    assert_eq!(
        on_vertex(r#".union(__.label(), __.values("label")).dedup().toList()"#),
        ["person", "CUSTOM"]
    );
    assert_eq!(
        on_vertex(r#".union(__.id(), __.values("id")).dedup().count().toList()"#),
        ["2"]
    );
    // An aggregated label and property stay distinct as well.
    assert_eq!(
        on_vertex(
            r#".union(__.label(), __.values("label")).aggregate("x").cap("x").unfold().toList()"#
        ),
        ["person", "CUSTOM"]
    );
}

#[test]
fn plain_values_without_reserved_names_are_unchanged() {
    assert_eq!(list(r#"g.v("1").values().toList()"#), ["marko", "29"]);
    assert_eq!(list(r#"g.v("1").values("label").toList()"#).len(), 0);
    assert_eq!(list(r#"g.v("1").values("id").toList()"#).len(), 0);
    assert_eq!(list(r#"g.v("1").label().toList()"#), ["person"]);
}

#[test]
fn by_a_string_reads_the_property_and_by_a_token_the_element() {
    assert_eq!(
        on_vertex(r#".project("p", "t").by("label").by(T.label).toList()"#),
        ["#{\"p\": \"CUSTOM\", \"t\": \"person\"}"]
    );
    assert_eq!(
        on_vertex(r#".project("p", "t").by("id").by(T.id).select("p").toList()"#),
        ["X"]
    );
    assert_eq!(
        on_edge(r#".project("p", "t").by("label").by(T.label).toList()"#),
        ["#{\"p\": \"CUSTOM\", \"t\": \"knows\"}"]
    );
    // Without a property named `label`, `by("label")` is unproductive, as in TinkerPop.
    assert_eq!(
        list(r#"g.v("1").project("p").by("label").toList()"#),
        ["#{}"]
    );
    assert_eq!(list(r#"g.V().groupCount().by("label").toList()"#), ["#{}"]);
    assert_eq!(
        list(r#"g.V().groupCount().by(T.label).unfold().count().toList()"#),
        ["2"]
    );
}

#[test]
fn element_map_keeps_the_tokens_and_a_property_of_the_same_name_apart() {
    // Four entries, as in TinkerPop: T.id, T.label, "name", "label".
    assert_eq!(
        on_vertex(".elementMap().count(Scope.local).toList()"),
        ["6"]
    );
    assert_eq!(
        on_vertex(r#".elementMap().select("label").toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(on_vertex(r#".elementMap().select("id").toList()"#), ["X"]);
    assert_eq!(
        on_vertex(r#".elementMap().values("label").toList()"#),
        ["CUSTOM"]
    );
    // Unfolded, both entries are there: the token one written as "label".
    assert_eq!(
        on_vertex(r#".elementMap().unfold().toList()"#),
        [
            "#{\"id\": \"7\"}",
            "#{\"label\": \"person\"}",
            "#{\"name\": \"Bill\"}",
            "#{\"label\": \"CUSTOM\"}",
            "#{\"id\": \"X\"}",
            "#{\"labels\": \"L\"}"
        ]
    );
    // Rendered as one map, the property wins over the token of the same name.
    assert_eq!(
        on_vertex(".elementMap().toList()"),
        ["#{\"id\": \"X\", \"label\": \"CUSTOM\", \"labels\": \"L\", \"name\": \"Bill\"}"]
    );
    // A token reads the element through the map.
    assert_eq!(
        on_vertex(r#".elementMap().project("t").by(T.label).toList()"#),
        ["#{\"t\": \"person\"}"]
    );
}

#[test]
fn without_a_property_of_that_name_select_finds_nothing() {
    // `"id"`/`"label"` are string keys: an element map holds them only as tokens.
    assert_eq!(
        list(r#"g.V("1").elementMap().select("id").toList()"#).len(),
        0
    );
    assert_eq!(
        list(r#"g.V("1").elementMap().select("label").toList()"#).len(),
        0
    );
    assert_eq!(
        list(r#"g.V("1").valueMap(true).select("label").toList()"#).len(),
        0
    );
    // Rendered, the tokens are written "id" and "label".
    assert_eq!(
        list(r#"g.V("1").elementMap().toList()"#),
        ["#{\"age\": 29, \"id\": \"1\", \"label\": \"person\", \"name\": \"marko\"}"]
    );
    assert_eq!(
        list(r#"g.V("1").elementMap().order().by(T.label).count().toList()"#),
        ["1"]
    );
}

#[test]
fn an_edge_map_has_its_endpoints() {
    assert_eq!(
        list(r#"g.E("0").elementMap().toList()"#),
        [concat!(
            "#{\"IN\": #{\"id\": \"2\", \"label\": \"person\"}, ",
            "\"OUT\": #{\"id\": \"1\", \"label\": \"person\"}, ",
            "\"id\": \"0\", \"label\": \"knows\", \"weight\": 0.5}"
        )]
    );
    assert_eq!(
        on_edge(r#".elementMap().select("label").toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(on_edge(".elementMap().count(Scope.local).toList()"), ["6"]);
}

#[test]
fn value_map_with_tokens_keeps_them_apart_too() {
    assert_eq!(
        on_vertex(r#".valueMap(true).select("label").toList()"#),
        ["[\"CUSTOM\"]"]
    );
    assert_eq!(
        on_vertex(".valueMap(true).count(Scope.local).toList()"),
        ["6"]
    );
    assert_eq!(
        on_vertex(".valueMap(true).toList()"),
        ["#{\"id\": [\"X\"], \"label\": [\"CUSTOM\"], \"labels\": [\"L\"], \"name\": [\"Bill\"]}"]
    );
}

#[test]
fn sorting_a_map_locally_keeps_its_tokens() {
    assert_eq!(
        on_vertex(r#".elementMap().order(Scope.local).by(Column.keys).select("label").toList()"#),
        ["CUSTOM"]
    );
    assert_eq!(
        on_vertex(".elementMap().order(Scope.local).by(Column.keys).count(Scope.local).toList()"),
        ["6"]
    );
    assert_eq!(
        list(
            r#"g.V("1").elementMap().order(Scope.local).by(Column.keys, Order.asc).unfold().toList()"#
        ),
        [
            "#{\"age\": 29}",
            "#{\"id\": \"1\"}",
            "#{\"label\": \"person\"}",
            "#{\"name\": \"marko\"}"
        ]
    );
}

#[test]
fn select_a_token_reads_the_token_entry() {
    assert_eq!(
        on_vertex(".elementMap().select(T.label).toList()"),
        ["person"]
    );
    assert_eq!(on_vertex(".elementMap().select(T.id).toList()"), ["7"]);
    assert_eq!(
        on_vertex(".valueMap(true).select(T.label).toList()"),
        ["person"]
    );
    assert_eq!(
        list(r#"g.E("0").elementMap().select(Direction.OUT).toList()"#),
        ["#{\"id\": \"1\", \"label\": \"person\"}"]
    );
    assert_eq!(
        list(r#"g.E("0").element_map().select(Direction.IN).select(T.id).toList()"#),
        ["2"]
    );
    // Only a map with that token entry: a vertex or a projected map is filtered.
    assert_eq!(list(r#"g.V("1").select(T.label).toList()"#).len(), 0);
    assert_eq!(
        list(r#"g.V("1").project("label").by(T.label).select(T.label).toList()"#).len(),
        0
    );
    assert_eq!(
        list(r#"g.V("1").project("label").by(T.label).select("label").toList()"#),
        ["person"]
    );
}

#[test]
fn select_a_token_in_the_rust_api() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let label = lock
        .traversal()
        .v("1")
        .element_map(None)
        .select(RecordKey::Label)
        .to_list()
        .unwrap();
    assert_eq!(label, vec![ElementProperty::from("person")]);
    let named = lock
        .traversal()
        .v("1")
        .element_map(None)
        .select(RecordKey::Name("name".into()))
        .to_list()
        .unwrap();
    assert_eq!(named, vec![ElementProperty::from("marko")]);
    let missing = lock
        .traversal()
        .v("1")
        .element_map(None)
        .select("label")
        .to_list()
        .unwrap();
    assert!(missing.is_empty());
}
