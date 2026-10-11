//! Traversal-level smoke layer: the TinkerPop "modern" graph is loaded through the
//! [`GraphStorage`] trait only, a handful of traversals run over it, and their results must equal
//! the results over a [`TraversalGraph`] loaded the same way.

use graphersal::storage::GraphStorage;
use graphersal::{ElementProperty, GraphTraversalSource, TraversalGraph};

use crate::contract::{edge, props, vertex};

/// Loads the TinkerPop "modern" graph (6 vertices, 6 edges) into an empty storage using only
/// trait methods (`add_vertex`, `add_edge`).
pub fn load_modern<S: GraphStorage>(storage: &mut S) {
    let person = |s: &mut S, id: &str, name: &str, age: i32| {
        vertex(
            s,
            Some(id),
            &["person"],
            props(&[("name", name.into()), ("age", age.into())]),
        )
    };
    let software = |s: &mut S, id: &str, name: &str| {
        vertex(
            s,
            Some(id),
            &["software"],
            props(&[("name", name.into()), ("lang", "java".into())]),
        )
    };
    let marko = person(storage, "1", "marko", 29);
    let vadas = person(storage, "2", "vadas", 27);
    let lop = software(storage, "3", "lop");
    let josh = person(storage, "4", "josh", 32);
    let ripple = software(storage, "5", "ripple");
    let peter = person(storage, "6", "peter", 35);
    let weight = |w: f64| {
        props(&[(
            "weight",
            ElementProperty::Number(graphersal::prelude::Number::from_f64(w)),
        )])
    };
    edge(storage, Some("7"), Some("knows"), marko, vadas, weight(0.5));
    edge(storage, Some("8"), Some("knows"), marko, josh, weight(1.0));
    edge(storage, Some("9"), Some("created"), marko, lop, weight(0.4));
    edge(
        storage,
        Some("10"),
        Some("created"),
        josh,
        ripple,
        weight(1.0),
    );
    edge(storage, Some("11"), Some("created"), josh, lop, weight(0.4));
    edge(
        storage,
        Some("12"),
        Some("created"),
        peter,
        lop,
        weight(0.2),
    );
}

/// Runs the smoke traversals over `storage` and returns `(query, sorted result)` pairs.
pub fn run_queries<S: GraphStorage>(storage: &mut S) -> Vec<(&'static str, Vec<String>)> {
    fn rendered(mut items: Vec<ElementProperty>) -> Vec<String> {
        let mut out: Vec<String> = items.drain(..).map(|item| format!("{item:?}")).collect();
        out.sort();
        out
    }
    let mut results = Vec::new();
    let mut q =
        |name: &'static str, items: Vec<ElementProperty>| results.push((name, rendered(items)));

    q(
        "v().count()",
        GraphTraversalSource::new(&mut *storage)
            .v(None)
            .count()
            .to_list()
            .expect("runs"),
    );
    q(
        "e().count()",
        GraphTraversalSource::new(&mut *storage)
            .e(None)
            .count()
            .to_list()
            .expect("runs"),
    );
    q(
        "v().has_label(person).values(name)",
        GraphTraversalSource::new(&mut *storage)
            .v(None)
            .has_label("person")
            .values("name")
            .to_list()
            .expect("runs"),
    );
    q(
        "v(1).out(knows).values(name)",
        GraphTraversalSource::new(&mut *storage)
            .v("1")
            .out("knows")
            .values("name")
            .to_list()
            .expect("runs"),
    );
    q(
        "v().out().out().values(name)",
        GraphTraversalSource::new(&mut *storage)
            .v(None)
            .out(None)
            .out(None)
            .values("name")
            .to_list()
            .expect("runs"),
    );
    q(
        "v(3).in_(created).values(name)",
        GraphTraversalSource::new(&mut *storage)
            .v("3")
            .in_("created")
            .values("name")
            .to_list()
            .expect("runs"),
    );
    q(
        "v().has_label(software).values(lang)",
        GraphTraversalSource::new(&mut *storage)
            .v(None)
            .has_label("software")
            .values("lang")
            .to_list()
            .expect("runs"),
    );
    q(
        "v().out_e().count()",
        GraphTraversalSource::new(&mut *storage)
            .v(None)
            .out_e(None)
            .count()
            .to_list()
            .expect("runs"),
    );
    results
}

/// Asserts that the smoke traversals over `storage` (loaded through the trait) equal the same
/// traversals over a [`TraversalGraph`].
pub fn assert_matches_reference<S: GraphStorage>(storage: &mut S) {
    load_modern(storage);
    let mut reference = TraversalGraph::new();
    load_modern(&mut reference);
    let got = run_queries(storage);
    let expected = run_queries(&mut reference);
    for ((name, got), (_, expected)) in got.iter().zip(&expected) {
        assert_eq!(
            got, expected,
            "traversal `{name}` differs from the TraversalGraph reference"
        );
    }
    assert_eq!(got.len(), expected.len());
    assert!(
        !got.iter().all(|(_, rows)| rows.is_empty()),
        "the smoke queries returned nothing"
    );
}
