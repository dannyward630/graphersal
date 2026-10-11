//! `cap()` of a keyed side effect (`aggregate`/`store`/`subgraph`) that ran but collected nothing
//! is the empty collection, not a missing-key error (TinkerPop registers the key when the step is
//! built). A key that no step declares still fails.

use graphersal::prelude::*;

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

fn ints(values: &[i64]) -> Vec<ElementProperty> {
    values.iter().copied().map(ElementProperty::from).collect()
}

mod rust_api {
    use super::*;

    fn cap_list(
        build: impl for<'b> Fn(
            GraphTraversalSource<'b, &'b TraversalGraph>,
        ) -> GraphTraversalSource<'b, &'b TraversalGraph>,
    ) -> ElementProperty {
        let graph = modern();
        let lock = graph.read();
        let mut out = build(lock.traversal()).to_list().expect("traversal runs");
        assert_eq!(out.len(), 1, "cap emits one collection");
        out.remove(0)
    }

    #[test]
    fn aggregate_and_store_over_a_missing_property_cap_to_empty() {
        assert_eq!(
            cap_list(|g| g.v(None).aggregate("a").by("foo").cap("a")),
            ElementProperty::Array(vec![])
        );
        assert_eq!(
            cap_list(|g| g.v(None).store("a").by("foo").cap("a")),
            ElementProperty::Array(vec![])
        );
    }

    #[test]
    fn a_present_property_is_still_collected() {
        let ElementProperty::Array(mut ages) =
            cap_list(|g| g.v(None).aggregate("a").by("age").cap("a"))
        else {
            panic!("array expected")
        };
        ages.sort_by_key(|v| format!("{v:?}"));
        let mut expected = ints(&[29, 27, 32, 35]);
        expected.sort_by_key(|v| format!("{v:?}"));
        assert_eq!(ages, expected);
    }

    #[test]
    fn an_empty_upstream_caps_to_empty() {
        assert_eq!(
            cap_list(|g| g.v(None).has("name", "zzz").aggregate("a").cap("a")),
            ElementProperty::Array(vec![])
        );
    }

    #[test]
    fn an_empty_subgraph_caps_to_an_empty_graph() {
        let graph = modern();
        let lock = graph.read();
        let sub = lock
            .traversal()
            .v(None)
            .out_e(vec!["noexist"])
            .subgraph("sg")
            .cap("sg")
            .to_graph()
            .unwrap();
        assert_eq!(sub.read().vertex_count(), 0);
        assert_eq!(sub.read().edge_count(), 0);
    }

    #[test]
    fn streaming_first_gives_the_empty_collection() {
        let graph = modern();
        let lock = graph.read();
        let first = lock
            .traversal()
            .v(None)
            .aggregate("a")
            .by("foo")
            .cap("a")
            .next()
            .expect("runs");
        assert_eq!(first, Some(ElementProperty::Array(vec![])));
    }

    #[test]
    fn an_undeclared_key_still_fails() {
        let graph = modern();
        let lock = graph.read();
        assert!(lock.traversal().v(None).cap("zzz").next().is_err());
    }
}

#[cfg(feature = "script")]
mod dsl {
    use super::*;
    use crate::script_host::eval_value;
    use std::sync::Arc;

    fn run(script: &str) -> Result<Vec<String>, String> {
        let value = eval_value(Arc::new(modern()), script).map_err(|e| format!("{e}"))?;
        let items = value.into_array().map_err(|e| e.to_string())?;
        Ok(items.iter().map(|v| format!("{v:?}")).collect())
    }

    /// Runs `script` and its camelCase/snake_case twin.
    fn both(camel: &str, snake: &str) -> Vec<String> {
        let a = run(camel).unwrap_or_else(|e| panic!("{camel}: {e}"));
        let b = run(snake).unwrap_or_else(|e| panic!("{snake}: {e}"));
        assert_eq!(a, b, "{camel} vs {snake}");
        a
    }

    #[test]
    fn the_eight_scenarios_are_empty() {
        for reducer in ["max", "min", "sum", "mean"] {
            let unfolded =
                format!(r#"g.V().aggregate("a").by("foo").cap("a").unfold().{reducer}().toList()"#);
            let unfolded_snake = format!(
                r#"g.v().aggregate("a").by("foo").cap("a").unfold().{reducer}().to_list()"#
            );
            assert!(both(&unfolded, &unfolded_snake).is_empty(), "{unfolded}");
            let local = format!(
                r#"g.V().aggregate("a").by("foo").cap("a").{reducer}(Scope.local).toList()"#
            );
            let local_snake = format!(
                r#"g.v().aggregate("a").by("foo").cap("a").{reducer}(Scope::local).to_list()"#
            );
            assert!(both(&local, &local_snake).is_empty(), "{local}");
        }
    }

    #[test]
    fn empty_collections_in_both_spellings() {
        assert_eq!(
            both(
                r#"g.V().aggregate("a").by("foo").cap("a").toList()"#,
                r#"g.v().store("a").by("foo").cap("a").to_list()"#
            ),
            vec!["[]"]
        );
        assert_eq!(
            run(r#"g.V().has("name","zzz").aggregate("a").cap("a").toList()"#).unwrap(),
            vec!["[]"]
        );
    }

    #[test]
    fn subgraph_caps_to_a_graph_even_when_empty() {
        assert_eq!(
            run(r#"g.V().outE("noexist").subgraph("sg").cap("sg").next().v().count().toList()"#)
                .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            run(r#"g.V().outE("created").subgraph("sg").cap("sg").next().e().count().toList()"#)
                .unwrap(),
            vec!["4"]
        );
    }

    #[test]
    fn a_child_traversal_declares_its_key() {
        assert_eq!(
            run(r#"g.V().local(__.aggregate("a").by("foo")).cap("a").toList()"#).unwrap(),
            vec!["[]"]
        );
    }

    #[test]
    fn a_seeded_set_is_never_replaced() {
        assert_eq!(
            run(
                r#"g.withSideEffect("a", Set.of()).V().aggregate("a").by("foo").cap("a").toList()"#
            )
            .unwrap(),
            vec!["[]"]
        );
        // The seeded set keeps deduplicating once a value is collected.
        assert_eq!(
            run(
                r#"g.withSideEffect("a", Set.of()).V().aggregate("a").by("age").cap("a").toList()"#
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn an_empty_bucket_resolves_in_where_predicates() {
        // The key resolves to the empty collection: within is false, without is true.
        let within =
            run(r#"g.V().aggregate("x").by("foo").constant("x").where(P.within("x")).toList()"#)
                .unwrap();
        assert!(within.is_empty(), "{within:?}");
        let without =
            run(r#"g.V().aggregate("x").by("foo").constant("x").where(P.without("x")).toList()"#)
                .unwrap();
        assert_eq!(without.len(), 6, "{without:?}");
    }

    #[test]
    fn an_undeclared_key_fails_with_the_updated_help() {
        let err =
            eval_value(Arc::new(modern()), r#"g.V().cap("zzz").toList()"#).expect_err("typo key");
        assert!(format!("{err}").contains("zzz"), "{err}");
    }
}
