//! Integration tests for `dedup().by(...)`: TinkerPop's `DedupGlobalStep` with a `by()`
//! modulator keeps the first traverser of each distinct projected key, in stream order, and
//! (TinkerPop 3.6+, "productive `by()`") filters out a traverser whose projection yields nothing.
//! Every expected result below is what TinkerPop returns for the same query on the modern graph.

use graphersal::prelude::Order;
use graphersal::prelude::*;

/// The string results of a traversal, in stream order.
fn strings(results: Vec<ElementProperty>) -> Vec<String> {
    results
        .iter()
        .map(|value| {
            value
                .as_string()
                .unwrap_or_else(|| panic!("expected a string, got {value:?}"))
                .to_string()
        })
        .collect()
}

/// Asserts that `err` is rooted in `InvalidModulator` for `dedup` at `position`.
fn assert_invalid_dedup_modulator(err: &TraverserError, position: usize) {
    match err.root_cause() {
        TraverserError::InvalidModulator {
            step,
            position: actual_position,
            ..
        } => {
            assert_eq!(step, "dedup", "wrong step in: {err}");
            assert_eq!(*actual_position, position, "wrong position in: {err}");
        }
        other => panic!("expected InvalidModulator, got: {other}"),
    }
}

/// TinkerPop: `g.V().dedup().by(label).values("name")` is `[marko, lop]`, the first vertex of
/// each label.
#[test]
fn by_label_keeps_the_first_vertex_of_each_label() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v(None)
        .dedup()
        .by(By::Label)
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(strings(names), ["marko", "lop"]);
    let count = lock
        .traversal()
        .v(None)
        .dedup()
        .by(By::Label)
        .count()
        .to_list()
        .unwrap();
    assert_eq!(count[0].as_i64(), Some(2));
}

/// TinkerPop: `g.V().dedup().by(T.id)` keeps every vertex (ids are unique).
#[test]
fn by_id_keeps_every_vertex() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let count = lock
        .traversal()
        .v(None)
        .dedup()
        .by(By::Id)
        .count()
        .to_list()
        .unwrap();
    assert_eq!(count[0].as_i64(), Some(6));
}

/// TinkerPop: `g.V().dedup().by("lang").values("name")` is `[lop]`: both software vertices have
/// `lang` "java", and the four persons have no `lang`, so their `by()` is unproductive and they
/// are filtered out.
#[test]
fn by_lang_filters_the_vertices_without_it() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v(None)
        .dedup()
        .by("lang")
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(strings(names), ["lop"]);
}

/// TinkerPop: `g.V().dedup().by("age").values("name")` is `[marko, vadas, josh, peter]`: the
/// ages are distinct, and the software vertices (no `age`) are filtered out rather than kept
/// under a shared "missing" key.
#[test]
fn by_age_is_productive_only_for_persons() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v(None)
        .dedup()
        .by("age")
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(strings(names), ["marko", "vadas", "josh", "peter"]);
    let names = lock
        .traversal()
        .v(None)
        .dedup()
        .by(__::values("age"))
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(strings(names), ["marko", "vadas", "josh", "peter"]);
}

/// TinkerPop: `g.V().dedup().by(__.out().count()).values("name")` is
/// `[marko, vadas, josh, peter]`: out-degrees 3, 0, 0, 2, 0, 1 in id order, first of each kept.
#[test]
fn by_out_degree_traversal() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v(None)
        .dedup()
        .by(__::out(None).count())
        .values("name")
        .to_list()
        .unwrap();
    assert_eq!(strings(names), ["marko", "vadas", "josh", "peter"]);
}

/// TinkerPop: `g.V().values("age").dedup().by(__.is(P.gt(28)))` is `[29, 32, 35]`: 27 does not
/// pass the filter, so its `by()` is unproductive and it is dropped.
#[test]
fn a_filtering_by_traversal_drops_the_unproductive_traversers() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let ages = lock
        .traversal()
        .v(None)
        .values("age")
        .dedup()
        .by(__::is(P::Gt(28)))
        .to_list()
        .unwrap();
    let ages: Vec<i64> = ages.iter().map(|v| v.as_i64().expect("an age")).collect();
    assert_eq!(ages, [29, 32, 35]);
}

/// A `by(__...)` child reads the path of the traverser it projects: `select("a")` must see the
/// step label recorded before `dedup()`. TinkerPop:
/// `g.V().as("a").out().dedup().by(__.select("a")).count()` is 3 (marko, josh, peter have out
/// edges).
#[test]
fn a_by_traversal_can_read_the_incoming_path() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let count = lock
        .traversal()
        .v(None)
        .as_("a")
        .out(None)
        .dedup()
        .by(__::select("a"))
        .count()
        .to_list()
        .unwrap();
    assert_eq!(count[0].as_i64(), Some(3));
}

#[test]
fn profile_renders_the_by_modulator() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let profile = lock
        .traversal()
        .v(None)
        .dedup()
        .by(By::Label)
        .count()
        .execute_with(ExecOptions::profile(ProfileType::none()));
    let rendered = profile.profile().unwrap().to_string();
    assert!(rendered.contains("dedup().by(T.label)"), "{rendered}");
    assert_eq!(profile.results()[0].as_i64(), Some(2));
}

#[test]
fn a_label_list_is_rejected() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .dedup()
        .by(["person"])
        .to_list()
        .expect_err("label list");
    assert_invalid_dedup_modulator(&err, 1);
}

#[test]
fn an_order_or_count_is_rejected() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .dedup()
        .by(Order::Desc)
        .to_list()
        .expect_err("order");
    assert_invalid_dedup_modulator(&err, 1);
    let err = lock
        .traversal()
        .v(None)
        .dedup()
        .by(By::Count)
        .to_list()
        .expect_err("count");
    assert_invalid_dedup_modulator(&err, 1);
}

#[test]
fn a_second_by_is_rejected() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .dedup()
        .by("name")
        .by("age")
        .to_list()
        .expect_err("a second by()");
    assert_invalid_dedup_modulator(&err, 2);
}

#[cfg(feature = "script")]
mod rhai {
    use std::sync::Arc;

    use super::{assert_invalid_dedup_modulator, strings};
    use graphersal::prelude::*;

    /// Runs a Rhai query that ends on a traversal and returns its materialized results.
    fn run(script: &str) -> TraverserResult<Vec<ElementProperty>> {
        let graph = Arc::new(GraphSource::tinkerpop_modern());
        let engine = crate::script_host::engine();
        let mut scope = crate::script_host::graph_scope(graph.clone());
        let gts = engine
            .eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(&mut scope, script)
            .unwrap_or_else(|e| panic!("`{script}` failed to evaluate: {e}"));
        let lock = graph.read();
        gts.ast_convert_to_gts(&*lock).to_list()
    }

    fn names(script: &str) -> Vec<String> {
        strings(run(script).unwrap_or_else(|e| panic!("`{script}` failed: {e}")))
    }

    #[test]
    fn by_modulators_match_tinkerpop() {
        let cases: [(&str, &[&str]); 9] = [
            (
                r#"g.V().dedup().by(T.label).values("name")"#,
                &["marko", "lop"],
            ),
            (
                r#"g.v().dedup().by(T::label).values("name")"#,
                &["marko", "lop"],
            ),
            (
                r#"g.V().dedup().by(By.Label).values("name")"#,
                &["marko", "lop"],
            ),
            (r#"g.V().dedup().by("lang").values("name")"#, &["lop"]),
            (
                r#"g.v().dedup().by("age").values("name")"#,
                &["marko", "vadas", "josh", "peter"],
            ),
            (
                r#"g.V().dedup().by(__.values("age")).values("name")"#,
                &["marko", "vadas", "josh", "peter"],
            ),
            (
                r#"g.V().dedup().by(__.out().count()).values("name")"#,
                &["marko", "vadas", "josh", "peter"],
            ),
            (
                r#"g.v().dedup().by(__.out().count()).values("name")"#,
                &["marko", "vadas", "josh", "peter"],
            ),
            (
                r#"g.V().dedup().by(T.id).values("name")"#,
                &["marko", "vadas", "lop", "josh", "ripple", "peter"],
            ),
        ];
        for (script, expected) in cases {
            assert_eq!(names(script), expected, "`{script}`");
        }
    }

    #[test]
    fn unusable_by_modulators_are_rejected() {
        let cases = [
            (r#"g.V().dedup().by(["person"])"#, 1),
            (r#"g.V().dedup().by(Order.desc)"#, 1),
            (r#"g.V().dedup().by(By.Count)"#, 1),
            (r#"g.V().dedup().by("name").by("age")"#, 2),
        ];
        for (script, position) in cases {
            let err = run(script).expect_err(script);
            assert_invalid_dedup_modulator(&err, position);
        }
    }
}

/// `dedup()` (no `by()`) over every stream shape: elements and vertex property elements by
/// identity, values and map entries by value, paths by their objects, lists by their items.
mod shapes {
    use super::*;

    fn count(
        query: impl FnOnce(
            GraphTraversalSource<'_, &TraversalGraph>,
        ) -> TraverserResult<Vec<ElementProperty>>,
    ) -> i64 {
        let graph = GraphSource::tinkerpop_modern();
        let lock = graph.read();
        let res = query(lock.traversal()).unwrap();
        res[0].as_i64().expect("a count")
    }

    #[test]
    fn values_and_entries_compare_by_value() {
        // weights 0.5, 1.0, 0.4, 1.0, 0.4, 0.2
        assert_eq!(
            count(|g| g.e(None).values("weight").dedup().count().to_list()),
            4
        );
        // 12 valueMap entries; the two `lang: [java]` entries are equal
        assert_eq!(
            count(|g| g.v(None).value_map(None).unfold().dedup().count().to_list()),
            11
        );
    }

    #[test]
    fn vertex_property_elements_compare_by_identity() {
        // lop and ripple both have lang = java: two distinct property elements
        assert_eq!(
            count(|g| g.v(None).properties("lang").dedup().count().to_list()),
            2
        );
    }

    #[test]
    fn paths_and_lists_compare_by_content() {
        // 12 one-hop paths, all different
        assert_eq!(
            count(|g| g.v(None).both(None).path().dedup().count().to_list()),
            12
        );
        // the same path twice per vertex collapses to one per vertex
        assert_eq!(
            count(|g| g
                .v(None)
                .union(vec![__::path(), __::path()])
                .dedup()
                .count()
                .to_list()),
            6
        );
        // two equal lists
        assert_eq!(
            count(|g| g
                .v(None)
                .both(None)
                .fold()
                .union(vec![__::identity(), __::identity()])
                .dedup()
                .count()
                .to_list()),
            1
        );
        // jpath properties compare by path text and value: one per vertex
        assert_eq!(
            count(|g| g
                .v(None)
                .union(vec![
                    __::properties("name".parse::<JsonPath>().unwrap()),
                    __::properties("name".parse::<JsonPath>().unwrap()),
                ])
                .dedup()
                .count()
                .to_list()),
            6
        );
    }

    /// Not a bug: TinkerPop 3.8.2 compares an edge property by key and value only
    /// (`TinkerProperty.equals`), so the six `weight` properties of the modern graph dedup to
    /// four. The vendored scenarios `Dedup::g_V_bothE_properties_dedup_count` (4) and
    /// `HasKey::g_V_bothE_properties_dedup_hasKeyXweightX_value` (four weights) pin it; vertex
    /// properties are elements with an id and dedup by identity.
    #[test]
    fn edge_properties_compare_by_key_and_value_like_tinkerpop() {
        assert_eq!(
            count(|g| g.e(None).properties("weight").dedup().count().to_list()),
            4
        );
        assert_eq!(
            count(|g| g
                .v(None)
                .both_e(None)
                .properties(None)
                .dedup()
                .count()
                .to_list()),
            4
        );
    }
}
