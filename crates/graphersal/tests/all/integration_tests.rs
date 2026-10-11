//! Integration tests for public Rust API in `graphersal`.

use graphersal::prelude::*;

/// Tests vertex creation and basic count traversal using the public API.
#[test]
fn test_graph_creation_and_traversal() {
    let graph = GraphSource::empty();

    let v1 = graph
        .write()
        .traversal_mut()
        .add_v("person")
        .property("name", "Alice")
        .property("age", 30)
        .next()
        .expect("should execute add_v");

    let v2 = graph
        .write()
        .traversal_mut()
        .add_v("person")
        .property("name", "Bob")
        .property("age", 25)
        .next()
        .expect("should execute add_v");

    assert!(v1.is_some());
    assert!(v2.is_some());

    let count_res = graph
        .read()
        .traversal()
        .v(None::<()>)
        .count()
        .next()
        .expect("should execute count");
    assert!(count_res.is_some());
    if let Some(ElementProperty::Number(num)) = count_res {
        assert_eq!(num.as_i64(), Some(2));
    } else {
        panic!(
            "Expected ElementProperty::Number for count, got {:?}",
            count_res
        );
    }
}

/// Tests filtering vertices by label and extracting property values.
#[test]
fn test_vertex_filtering_and_values() {
    let graph = GraphSource::empty();

    graph
        .write()
        .traversal_mut()
        .add_v("software")
        .property("name", "Graphersal")
        .property("lang", "Rust")
        .next()
        .expect("should create software vertex");

    let list = graph
        .read()
        .traversal()
        .v(None::<()>)
        .has_label("software")
        .values("name")
        .to_list()
        .expect("should return values");
    assert_eq!(list.len(), 1);
    if let ElementProperty::String(ref name) = list[0] {
        assert_eq!(name, "Graphersal");
    } else {
        panic!("Expected ElementProperty::String, got {:?}", list[0]);
    }
}

/// Tests that optimized traversal queries execute correctly end-to-end on tinkerpop_modern.
#[test]
fn test_optimized_traversals_end_to_end() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    // 1. Source filter + count pushdown on vertices
    let v_count = lock
        .traversal()
        .v(None::<()>)
        .has_label("person")
        .count()
        .next()
        .expect("should execute count")
        .expect("should return count value");
    assert_eq!(v_count.as_i64(), Some(4));

    // 2. Source filter + count pushdown on edges
    let e_count = lock
        .traversal()
        .e(None::<()>)
        .has_label("knows")
        .count()
        .next()
        .expect("should execute count")
        .expect("should return count value");
    assert_eq!(e_count.as_i64(), Some(2));

    // 3. FlatMap folding: out_e + has_label
    let out_edges = lock
        .traversal()
        .v("1")
        .out_e("knows")
        .has_label("knows")
        .to_list()
        .expect("should return out edges");
    assert_eq!(out_edges.len(), 2);
}

/// Tests that .profile() accurately reports the optimized physical execution plan.
#[test]
fn test_optimized_traversal_profiling_physical_plan() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    // Profile v().has_label("person").count()
    let profile = lock
        .traversal()
        .v(None::<()>)
        .has_label("person")
        .count()
        .profile()
        .expect("should execute profile");

    let profile_str = profile.to_string();
    assert!(
        profile_str.contains(".count()"),
        "Profile should contain folded .count(): {}",
        profile_str
    );
    assert!(
        !profile_str.contains("has_label("),
        "Profile should not contain separate has_label step: {}",
        profile_str
    );
}
