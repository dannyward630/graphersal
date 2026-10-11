#![cfg(all(test, feature = "script"))]

use crate::script::{ScriptGraphTraversalSource, engine_with_limits, graph_scope_with_options};
use crate::{ElementProperty, Graph, GraphTraversalSource, TraversalGraph, TraverserResult};
use std::sync::Arc;

/// Executes a traversal via Rust API and via one or more Rhai script strings,
/// asserting that both return identical `Vec<ElementProperty>` output and satisfy the validator closure.
pub fn assert_traversal_eq<R, V>(graph: Graph, rust_fn: R, rhai_scripts: &[&str], validator: V)
where
    R: for<'b> FnOnce(
        GraphTraversalSource<'b, &'b TraversalGraph>,
    ) -> TraverserResult<Vec<ElementProperty>>,
    V: Fn(&[ElementProperty]),
{
    // 1. Run Rust API traversal
    let lock = graph.read();
    let g = lock.traversal();
    let rust_res = rust_fn(g).expect("Rust API traversal failed");
    validator(&rust_res);
    drop(lock);

    // 2. Run each Rhai script query
    let arc_graph = Arc::new(graph);
    let engine = engine_with_limits(
        &crate::script::ScriptLimits::default(),
        std::sync::Arc::new(crate::graph::AllowAll),
    );

    for &script in rhai_scripts {
        let mut scope = graph_scope_with_options(
            arc_graph.clone(),
            &[],
            std::sync::Arc::new(crate::graph::AllowAll),
        );
        let exec = engine
            .eval_with_scope::<rhai::Dynamic>(&mut scope, script)
            .unwrap_or_else(|e| panic!("Rhai evaluation failed for script `{}`: {:?}", script, e));

        if let Ok(script_gts) = exec.try_cast_result::<ScriptGraphTraversalSource>() {
            let lock = arc_graph.read();
            let rhai_res = script_gts
                .ast_convert_to_gts(&*lock)
                .to_list()
                .unwrap_or_else(|e| {
                    panic!("Rhai AST execution failed for script `{}`: {:?}", script, e)
                });

            assert_eq!(
                rust_res, rhai_res,
                "Result mismatch between Rust API and Rhai script query `{}`",
                script
            );
            validator(&rhai_res);
        } else {
            panic!(
                "Rhai script query `{}` did not evaluate to a ScriptGraphTraversalSource",
                script
            );
        }
    }
}

/// Executes a traversal via the Rust API and via one or more Rhai script strings, asserting that
/// every run FAILS and that each rendered error satisfies the validator closure (it receives the
/// `Display` text of the error, so it can check the cause and the `help()` text).
pub fn assert_traversal_err<R, V>(graph: Graph, rust_fn: R, rhai_scripts: &[&str], validator: V)
where
    R: for<'b> FnOnce(
        GraphTraversalSource<'b, &'b TraversalGraph>,
    ) -> TraverserResult<Vec<ElementProperty>>,
    V: Fn(&str),
{
    let lock = graph.read();
    let rust_err = match rust_fn(lock.traversal()) {
        Ok(res) => panic!("Rust API traversal unexpectedly succeeded: {res:?}"),
        Err(err) => crate::error::format_diagnostic(&*err),
    };
    validator(&rust_err);
    drop(lock);

    let arc_graph = Arc::new(graph);
    let engine = engine_with_limits(
        &crate::script::ScriptLimits::default(),
        std::sync::Arc::new(crate::graph::AllowAll),
    );
    for &script in rhai_scripts {
        let mut scope = graph_scope_with_options(
            arc_graph.clone(),
            &[],
            std::sync::Arc::new(crate::graph::AllowAll),
        );
        let exec = engine
            .eval_with_scope::<rhai::Dynamic>(&mut scope, script)
            .unwrap_or_else(|e| panic!("Rhai evaluation failed for script `{}`: {:?}", script, e));
        let Ok(script_gts) = exec.try_cast_result::<ScriptGraphTraversalSource>() else {
            panic!("Rhai script query `{script}` did not evaluate to a ScriptGraphTraversalSource");
        };
        let lock = arc_graph.read();
        match script_gts.ast_convert_to_gts(&*lock).to_list() {
            Ok(res) => panic!("Rhai script `{script}` unexpectedly succeeded: {res:?}"),
            Err(err) => validator(&crate::error::format_diagnostic(&*err)),
        }
    }
}

/// A small graph with object-valued properties, for the nested-key (`properties(k).has(..)`)
/// arms of the property filters:
///
/// - vertex `doc` `name: "d1"`, top-level `a: 100`, `meta: {a: 1, b: "x"}` (the top-level `a`
///   tells a nested read apart from a top-level one);
/// - vertex `doc` `name: "d2"`, `meta: {b: "y"}`;
/// - vertex `plain` `name: "p"`, `meta: "scalar"` (a non-object value);
/// - edge `ref` d2 -> d1 with `info: {w: 2}` and `note: "n"`.
pub fn nested_doc_graph() -> Graph {
    let graph = Arc::new(crate::GraphSource::empty());
    let _ = crate::script::eval_value(
        graph.clone(),
        r#"
        g.addV("doc").property("name", "d1").property("a", 100)
            .property("meta", #{a: 1, b: "x"}).next();
        g.addV("doc").property("name", "d2").property("meta", #{b: "y"}).next();
        g.addV("plain").property("name", "p").property("meta", "scalar").next();
        g.V().has("name", "d2").addE("ref").to(__.V().has("name", "d1"))
            .property("info", #{w: 2}).property("note", "n").property("w", 200).next();
        "#,
    )
    .expect("building the nested doc graph");
    Arc::try_unwrap(graph).expect("the nested doc graph has a single owner")
}

/// Like [`test_dual!`], but the query must fail in the Rust API and in every Rhai spelling; the
/// validator receives the rendered error text.
#[macro_export]
macro_rules! test_dual_err {
    (
        graph: $graph:expr_2021,
        rust: $rust_expr:expr_2021,
        rhai: [ $($script:expr_2021),+ $(,)? ],
        assert: $validator:expr_2021 $(,)?
    ) => {
        $crate::traversal::test_utils::assert_traversal_err(
            $graph,
            $rust_expr,
            &[ $($script),+ ],
            $validator,
        );
    };
}

/// Test helper: runs one traversal through the Rust API and the Rhai DSL and compares the results.
#[macro_export]
macro_rules! test_dual {
    (
        graph: $graph:expr_2021,
        rust: $rust_expr:expr_2021,
        rhai: [ $($script:expr_2021),+ $(,)? ],
        assert: $validator:expr_2021 $(,)?
    ) => {
        $crate::traversal::test_utils::assert_traversal_eq(
            $graph,
            $rust_expr,
            &[ $($script),+ ],
            $validator,
        );
    };
}

#[cfg(test)]
mod tests {
    use crate::{ElementProperty, GraphSource};

    #[test]
    fn test_assert_traversal_eq_placeholder() {
        test_dual!(
            graph: GraphSource::tinkerpop_modern(),
            rust: |g| g.v(None).count().to_list(),
            rhai: [
                r#"g.V().count()"#,
                r#"g.v().count()"#,
            ],
            assert: |res| assert_eq!(res[0], ElementProperty::Number(6.into())),
        );
    }
}
