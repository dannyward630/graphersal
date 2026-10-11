//! Integration tests for `inject()`: `null` is a value and a list argument is one value
//! (TinkerPop semantics), in the Rust API and in the Rhai DSL (where `()` is `null` and a Rhai
//! array is one list; several values use the variadic form `inject(a, b, ...)`).

use graphersal::prelude::*;

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

fn list(items: &[i64]) -> ElementProperty {
    ElementProperty::Array(items.iter().copied().map(ElementProperty::from).collect())
}

fn int(value: i64) -> ElementProperty {
    ElementProperty::from(value)
}

mod rust_api {
    use super::*;

    #[test]
    fn null_alone_is_one_value() {
        let graph = modern();
        let lock = graph.read();
        let values = lock
            .traversal()
            .inject(vec![ElementProperty::Null])
            .to_list()
            .unwrap();
        assert_eq!(values, vec![ElementProperty::Null]);
    }

    #[test]
    fn nulls_mixed_with_values_keep_their_order() {
        let graph = modern();
        let lock = graph.read();
        let values = lock
            .traversal()
            .inject(vec![
                ElementProperty::Null,
                int(1),
                int(3),
                ElementProperty::Null,
            ])
            .to_list()
            .unwrap();
        assert_eq!(
            values,
            vec![ElementProperty::Null, int(1), int(3), ElementProperty::Null]
        );
    }

    #[test]
    fn mid_traversal_null_joins_the_stream() {
        let graph = modern();
        let lock = graph.read();
        let values = lock
            .traversal()
            .v("1")
            .values("age")
            .inject(vec![ElementProperty::Null])
            .to_list()
            .unwrap();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&ElementProperty::Null));
        assert!(values.contains(&int(29)));
    }

    #[test]
    fn a_list_is_one_value() {
        let graph = modern();
        let lock = graph.read();
        let values = lock
            .traversal()
            .inject(vec![list(&[1, 2, 3])])
            .to_list()
            .unwrap();
        assert_eq!(values, vec![list(&[1, 2, 3])]);
        let count = lock
            .traversal()
            .inject(vec![list(&[1, 2, 3])])
            .count()
            .next()
            .unwrap();
        assert_eq!(count, Some(int(1)));
    }

    #[test]
    fn two_lists_and_a_nested_list() {
        let graph = modern();
        let lock = graph.read();
        let values = lock
            .traversal()
            .inject(vec![list(&[1, 2]), list(&[3])])
            .to_list()
            .unwrap();
        assert_eq!(values, vec![list(&[1, 2]), list(&[3])]);
        let nested = ElementProperty::Array(vec![list(&[1, 2]), list(&[3])]);
        let values = lock
            .traversal()
            .inject(vec![nested.clone()])
            .to_list()
            .unwrap();
        assert_eq!(values, vec![nested]);
    }

    /// An injected map is an object for `values(k)`, `select(k)` and `has(k, v)`.
    #[test]
    fn an_injected_map_is_an_object() {
        let graph = modern();
        let lock = graph.read();
        let mut element = PropertyMap::default();
        element.insert("name", ElementProperty::from("x"));
        let map = ElementProperty::Object(Box::new(element));
        let values = lock
            .traversal()
            .inject(vec![map.clone()])
            .values("name")
            .to_list()
            .unwrap();
        assert_eq!(values, vec![ElementProperty::from("x")]);
        let selected = lock
            .traversal()
            .inject(vec![map.clone()])
            .select(vec!["name"])
            .to_list()
            .unwrap();
        assert_eq!(selected, vec![ElementProperty::from("x")]);
        let kept = lock
            .traversal()
            .inject(vec![map.clone()])
            .has("name", "x")
            .to_list()
            .unwrap();
        assert_eq!(kept, vec![map]);
    }

    #[test]
    fn a_list_unfolds_into_its_elements() {
        let graph = modern();
        let lock = graph.read();
        let values = lock
            .traversal()
            .inject(vec![list(&[1, 2, 3])])
            .unfold()
            .to_list()
            .unwrap();
        assert_eq!(values, vec![int(1), int(2), int(3)]);
    }

    #[test]
    fn several_values_and_no_value() {
        let graph = modern();
        let lock = graph.read();
        let count = lock
            .traversal()
            .inject(vec![1_i64, 2, 3])
            .count()
            .next()
            .unwrap();
        assert_eq!(count, Some(int(3)));
        let values = lock
            .traversal()
            .inject::<ElementProperty>(vec![])
            .to_list()
            .unwrap();
        assert!(values.is_empty());
    }

    #[test]
    fn several_vertices_are_all_injected() {
        let graph = modern();
        let lock = graph.read();
        let v1 = lock.traversal().v("1").next().unwrap().unwrap();
        let v2 = lock.traversal().v("2").next().unwrap().unwrap();
        let ids = lock
            .traversal()
            .inject(vec![v1, v2])
            .id()
            .to_list()
            .unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn null_through_fold_count_and_is() {
        let graph = modern();
        let lock = graph.read();
        let folded = lock
            .traversal()
            .inject(vec![ElementProperty::Null, int(1)])
            .fold()
            .next()
            .unwrap();
        assert_eq!(
            folded,
            Some(ElementProperty::Array(vec![ElementProperty::Null, int(1)]))
        );
        let count = lock
            .traversal()
            .inject(vec![ElementProperty::Null, ElementProperty::Null])
            .count()
            .next()
            .unwrap();
        assert_eq!(count, Some(int(2)));
        let nulls = lock
            .traversal()
            .inject(vec![ElementProperty::Null, int(1)])
            .is(P::Eq(PValue::Null))
            .to_list()
            .unwrap();
        assert_eq!(nulls, vec![ElementProperty::Null]);
    }
}

#[cfg(feature = "script")]
mod dsl {
    use crate::script_host::eval_value;
    use graphersal::prelude::*;
    use std::sync::Arc;

    /// Evaluates `script` (ending in a list terminal) on the modern graph and renders each result
    /// with Rhai's `Debug` (`()` is null, `[1, 2]` a list).
    fn rendered(script: &str) -> Vec<String> {
        eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
            .unwrap_or_else(|err| panic!("{script}: {err}"))
            .into_array()
            .unwrap_or_else(|err| panic!("{script}: not a list ({err})"))
            .iter()
            .map(|value| format!("{value:?}"))
            .collect()
    }

    /// Asserts the rendered results of `script`, in order. `inject` has one spelling (no
    /// camelCase twin), so `toList()` and `to_list()` are both exercised instead.
    fn assert_both(script: &str, expected: &[&str]) {
        assert_eq!(rendered(script), expected, "{script}");
        let snake = script.replace("toList()", "to_list()");
        assert_eq!(rendered(&snake), expected, "{snake}");
    }

    #[test]
    fn null_alone() {
        assert_both("g.inject(()).toList()", &["()"]);
    }

    #[test]
    fn nulls_mixed_with_values() {
        assert_both("g.inject((), 1, 3, ()).toList()", &["()", "1", "3", "()"]);
        assert_both("g.inject(1, (), 2).count().toList()", &["3"]);
    }

    #[test]
    fn mid_traversal_null() {
        let values = rendered(r#"g.V("1").values("age").inject(()).toList()"#);
        assert_eq!(values.len(), 2, "{values:?}");
        assert!(values.contains(&"()".to_string()), "{values:?}");
    }

    #[test]
    fn a_list_is_one_value() {
        assert_both("g.inject([1, 2, 3]).toList()", &["[1, 2, 3]"]);
        assert_both("g.inject([1, 2, 3]).count().toList()", &["1"]);
    }

    #[test]
    fn two_lists_and_a_nested_list() {
        assert_both("g.inject([1, 2], [3]).toList()", &["[1, 2]", "[3]"]);
        assert_both("g.inject([[1, 2], [3]]).toList()", &["[[1, 2], [3]]"]);
    }

    #[test]
    fn a_map_is_one_value() {
        assert_both(r#"g.inject(#{"a": 1}).count().toList()"#, &["1"]);
    }

    /// An injected map is an object: `values(k)`, `select(k)`, `select(a, b)` and `has(k, v)` read
    /// its keys like they read a `project()`-ed map (not refused as an undeclared label at plan
    /// time, not a cast error at run time).
    #[test]
    fn an_injected_map_is_an_object() {
        let m = r#"let m = #{name: "x", age: 3};"#;
        assert_both(
            &format!(r#"{m} g.inject(m).values("name").toList()"#),
            &[r#""x""#],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).select("name").toList()"#),
            &[r#""x""#],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).select("name", "age").toList()"#),
            &[r#"#{"age": 3, "name": "x"}"#],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).has("name", "x").count().toList()"#),
            &["1"],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).has("name", "y").count().toList()"#),
            &["0"],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).has("age", P.gt(2)).values("name").toList()"#),
            &[r#""x""#],
        );
        assert_both(
            &format!(r#"{m} g.inject(m, m).values("age").sum().toList()"#),
            &["6"],
        );
    }

    #[test]
    fn a_nested_injected_map_stays_navigable() {
        let m = r#"let m = #{name: "x", address: #{city: "Berlin", zip: [1, 2]}};"#;
        assert_both(
            &format!(r#"{m} g.inject(m).values("address").values("city").toList()"#),
            &[r#""Berlin""#],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).select("address").select("city").toList()"#),
            &[r#""Berlin""#],
        );
        assert_both(
            &format!(r#"{m} g.inject(m).values("address").values("zip").toList()"#),
            &["[1, 2]"],
        );
    }

    #[test]
    fn an_injected_list_of_maps_unfolds_into_objects() {
        let ms = r#"let ms = [#{name: "a"}, #{name: "b"}];"#;
        assert_both(&format!(r#"{ms} g.inject(ms).count().toList()"#), &["1"]);
        assert_both(
            &format!(r#"{ms} g.inject(ms).unfold().values("name").toList()"#),
            &[r#""a""#, r#""b""#],
        );
        assert_both(
            &format!(r#"{ms} g.inject(ms).unfold().select("name").toList()"#),
            &[r#""a""#, r#""b""#],
        );
    }

    /// A scalar is not an object: `values(k)` keeps failing with the same cast error.
    #[test]
    fn a_non_map_injected_value_still_fails_values() {
        for script in [
            r#"g.inject(1).values("name").toList()"#,
            r#"g.inject(1).values("name").to_list()"#,
        ] {
            let err = eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
                .expect_err(script)
                .to_string();
            assert!(
                err.contains("expected type [vertex or edge or object], got type 'computed'"),
                "{script}: {err}"
            );
        }
    }

    #[test]
    fn empty_inject() {
        assert_both("g.inject().toList()", &[]);
    }

    #[test]
    fn unfold_and_count() {
        assert_both("g.inject([1, 2, 3]).unfold().toList()", &["1", "2", "3"]);
        assert_both("g.inject(1, 2, 3).count().toList()", &["3"]);
    }

    #[test]
    fn null_through_fold_count_and_is() {
        assert_both("g.inject((), 1).fold().toList()", &["[(), 1]"]);
        assert_both("g.inject((), ()).count().toList()", &["2"]);
        assert_both("g.inject((), 1).is(P.eq(())).toList()", &["()"]);
        assert_both("g.inject((), 1).is(P.neq(())).toList()", &["1"]);
    }

    #[test]
    fn null_comparisons_follow_tinkerpop() {
        assert_both("g.inject(()).is(P.lte(())).toList()", &["()"]);
        assert_both("g.inject(()).is(P.gte(())).toList()", &["()"]);
        assert_both("g.inject(()).is(P.lt(())).toList()", &[]);
        assert_both("g.inject(()).is(P.gt(())).toList()", &[]);
        assert_both("g.inject((), 5).is(P.gt(1)).toList()", &["5"]);
        assert_both("g.inject((), 5).is(P.within(())).toList()", &["()"]);
    }

    #[test]
    fn string_steps_map_null_to_null() {
        assert_both(r#"g.inject("a", ()).toUpper().toList()"#, &[r#""A""#, "()"]);
        assert_both(r#"g.inject(" a ", ()).trim().toList()"#, &[r#""a""#, "()"]);
    }

    #[test]
    fn any_and_all_filter_lists() {
        assert_both(
            "g.inject([1, 2], [3], 7).any(P.eq(2)).toList()",
            &["[1, 2]"],
        );
        assert_both(
            "g.inject([1, 2], [3], 7, []).all(P.gt(0)).toList()",
            &["[1, 2]", "[3]", "[]"],
        );
    }
}
