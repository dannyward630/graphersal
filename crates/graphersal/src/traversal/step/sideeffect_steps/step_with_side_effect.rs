use crate::graph::GraphStorage;
use crate::*;
use std::sync::Arc;

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Starts the traversal with a named side effect holding the given value.
    pub fn with_side_effect<T: Into<ElementProperty>>(mut self, name: &str, value: T) -> Self {
        self.side_effect.insert(
            CompactString::new(name),
            crate::traversal::side_effect::SideEffectValue::seed(value.into()),
        );
        self
    }

    /// Names an explicit target graph for `subgraph(name)`: `withSideEffect("sg", graph)`.
    /// `subgraph("sg")` then writes the selected edges, their endpoint vertices, ids, labels and
    /// properties into `target` (through the [`GraphStorage`] mutation API) when the step has
    /// run, and `cap("sg")` returns `target` itself. A vertex or edge id the target already holds
    /// is reused or skipped, so repeated writes are idempotent; the source's stored schema is
    /// copied only when the target stores none.
    ///
    /// The target must not be the graph being traversed (it is locked then and the step fails
    /// instead of deadlocking).
    ///
    /// ```
    /// use graphersal::prelude::*;
    /// use std::sync::Arc;
    /// let graph = GraphSource::tinkerpop_modern();
    /// let target = Arc::new(GraphSource::empty());
    /// let g = graph.read();
    /// g.traversal()
    ///     .with_side_effect_graph("sg", target.clone())
    ///     .e(None)
    ///     .subgraph("sg")
    ///     .to_list()
    ///     .unwrap();
    /// assert_eq!(target.read().edge_count(), 6);
    /// ```
    pub fn with_side_effect_graph(mut self, name: &str, target: Arc<crate::graph::Graph>) -> Self {
        self.side_effect.insert(
            CompactString::new(name),
            crate::traversal::side_effect::SideEffectValue::Subgraph {
                edges: Default::default(),
                target: Some(target),
            },
        );
        self
    }

    /// Declares a side effect with a **reducer**, `withSideEffect(name, initial, Operator)`:
    /// `aggregate(name)`/`store(name)` combine into the value with `operator` instead of
    /// collecting a list, and `cap(name)` reads the result.
    ///
    /// One `aggregate()` execution folds the values it collected in stream order, except that
    /// `Operator::Assign` and `Operator::AddAll` receive them as one list. `store()` folds after
    /// every element. An empty execution leaves the value unchanged. Deviations from TinkerPop
    /// 3.8.2 (book page "Sack and Operators", `tinkerpop_deviations.md`): the collection rule
    /// above is inferred from the feature files (3.7.2 itself cannot run `sum` here); integer
    /// overflow is an `OperatorFailed` error; an array seed makes a reduced value, never a list.
    ///
    /// ```
    /// use graphersal::prelude::*;
    /// let graph = GraphSource::tinkerpop_modern();
    /// let g = graph.read();
    /// let g = g.traversal();
    /// let sum = g
    ///     .with_side_effect_reduced("a", 1, Operator::Sum)
    ///     .v(None::<()>)
    ///     .aggregate("a")
    ///     .by("age")
    ///     .cap("a")
    ///     .next()
    ///     .unwrap();
    /// assert_eq!(sum.and_then(|v| v.as_i64()), Some(124));
    /// ```
    pub fn with_side_effect_reduced<T: Into<ElementProperty>>(
        mut self,
        name: &str,
        value: T,
        operator: Operator,
    ) -> Self {
        self.side_effect.insert(
            CompactString::new(name),
            crate::traversal::side_effect::SideEffectValue::seed_reduced(value.into(), operator),
        );
        self
    }

    /// Declares a side effect as a **set**: `aggregate(name)`/`store(name)` into it keep each value
    /// once (first-seen order), and `cap(name)`/`select(name)` return it as a deduplicated list.
    ///
    /// Duplicates in `items` collapse. Bulk is invisible: merging equal traversers never changes
    /// the set. A set is a property of the side-effect bucket only; Graphersal has no set value
    /// type (`toSet()`, `GType.SET` are not implemented).
    ///
    /// Deviations from TinkerPop (book page "Set Side Effects"): members keep first-seen order
    /// (a TinkerPop set has an unspecified one); members are compared by value, so `1` and `1.0`
    /// differ, and a property value from `values()` counts by its value, not by
    /// the element it came from (a `properties()` element counts by element and key; an edge property too, where TinkerPop
    /// compares key and value);
    /// `cap`/`select` return a plain array.
    ///
    /// ```
    /// use graphersal::prelude::*;
    /// let graph = GraphSource::tinkerpop_modern();
    /// let g = graph.read();
    /// let g = g.traversal();
    /// let names = g
    ///     .with_side_effect_set("a", Vec::<ElementProperty>::new())
    ///     .v(None::<()>)
    ///     .both(None)
    ///     .values("name")
    ///     .aggregate("a")
    ///     .cap("a")
    ///     .next()
    ///     .unwrap();
    /// assert!(names.is_some());
    /// ```
    pub fn with_side_effect_set<I: IntoIterator<Item = ElementProperty>>(
        mut self,
        name: &str,
        items: I,
    ) -> Self {
        self.side_effect.insert(
            CompactString::new(name),
            crate::traversal::side_effect::SideEffectValue::seed_set(items),
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", "value")
            .v("1")
            .select("x")
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(result.as_string(), Some("value"));
    }

    #[test]
    fn simple_str_array() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", vec!["hello", "world"])
            .v("1")
            .select("x")
            .next()
            .unwrap()
            .unwrap();
        match result {
            ElementProperty::Array(result) => {
                assert_eq!(result[0].as_string(), Some("hello"));
                assert_eq!(result[1].as_string(), Some("world"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn simple_i64_array() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", vec![1_i64, 2_i64])
            .v("1")
            .select("x")
            .next()
            .unwrap()
            .unwrap();
        match result {
            ElementProperty::Array(result) => {
                assert_eq!(result[0].as_i64(), Some(1_i64));
                assert_eq!(result[1].as_i64(), Some(2_i64));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn simple_f64_array() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", vec![1.1_f64, 2.2_f64])
            .v("1")
            .select("x")
            .next()
            .unwrap()
            .unwrap();
        match result {
            ElementProperty::Array(result) => {
                assert_eq!(result[0].as_f64(), Some(1.1_f64));
                assert_eq!(result[1].as_f64(), Some(2.2_f64));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn number() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", 1_usize)
            .v("1")
            .select("x")
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(result.as_i64(), Some(1_i64));
    }

    #[test]
    fn double() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", "hello")
            .with_side_effect("y", "world")
            .v("1")
            .select(vec!["x", "y"])
            .next()
            .unwrap()
            .unwrap();
        match result {
            ElementProperty::Object(result) => {
                assert_eq!(result["x"].as_string(), Some("hello"));
                assert_eq!(result["y"].as_string(), Some("world"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn alias_after_v() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .with_side_effect("x", "hello")
            .v("1")
            .id()
            .as_("z")
            .with_side_effect("y", "world")
            .select(vec!["x", "y", "z"])
            .next()
            .unwrap()
            .unwrap();
        match result {
            ElementProperty::Object(result) => {
                assert_eq!(result["x"].as_string(), Some("hello"));
                assert_eq!(result["y"].as_string(), Some("world"));
                assert_eq!(result["z"].as_string(), Some("1"));
            }
            _ => panic!(),
        }
    }
}
