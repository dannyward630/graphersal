//! Pins the shape of the `file_tree` sample graph (`TraversalGraph::file_tree()` /
//! `GraphSource::file_tree()`) and its script bindings, so the `glob_path` tests and docs can
//! rely on it.

use graphersal::prelude::*;

fn count<T: FnOnce(&TraversalGraph) -> TraverserResult<Option<ElementProperty>>>(f: T) -> i64 {
    let graph = TraversalGraph::file_tree();
    f(&graph)
        .unwrap()
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| panic!("count did not return an integer"))
}

fn names(graph: &TraversalGraph, id: &str) -> Vec<ElementProperty> {
    graph.traversal().v(id).values("name").to_list().unwrap()
}

#[test]
fn file_tree_has_37_vertices_and_38_edges() {
    assert_eq!(count(|g| g.traversal().v(None::<()>).count().next()), 37);
    assert_eq!(count(|g| g.traversal().e(None::<()>).count().next()), 38);
    assert_eq!(
        count(|g| g
            .traversal()
            .e(None::<()>)
            .has_label("contains")
            .count()
            .next()),
        36
    );
    assert_eq!(
        count(|g| g
            .traversal()
            .e(None::<()>)
            .has_label("points_to")
            .count()
            .next()),
        2
    );
}

#[test]
fn file_tree_label_counts() {
    assert_eq!(
        count(|g| g
            .traversal()
            .v(None::<()>)
            .has_label("directory")
            .count()
            .next()),
        18
    );
    assert_eq!(
        count(|g| g.traversal().v(None::<()>).has_label("file").count().next()),
        17
    );
    assert_eq!(
        count(|g| g
            .traversal()
            .v(None::<()>)
            .has_label("symlink")
            .count()
            .next()),
        2
    );
}

#[test]
fn file_tree_ids_and_names() {
    let graph = TraversalGraph::file_tree();

    assert_eq!(names(&graph, "root"), vec![ElementProperty::from("root")]);
    assert_eq!(
        names(&graph, "root/author/x/some/f.rs"),
        vec![ElementProperty::from("f.rs")]
    );
    assert_eq!(
        names(&graph, "root/docs/résumé.md"),
        vec![ElementProperty::from("résumé.md")]
    );
    assert_eq!(
        names(&graph, "root/docs/data[1].csv"),
        vec![ElementProperty::from("data[1].csv")]
    );
    assert_eq!(
        names(&graph, "root/#slash"),
        vec![ElementProperty::from("a/b")]
    );

    // `root/#unnamed` exists but has no `name` property.
    let unnamed = graph.traversal().v("root/#unnamed").id().to_list().unwrap();
    assert_eq!(unnamed, vec![ElementProperty::from("root/#unnamed")]);
    assert!(names(&graph, "root/#unnamed").is_empty());

    // `root/#42` has the int64 name 42, not a string.
    let answer = names(&graph, "root/#42");
    assert_eq!(answer.len(), 1);
    match &answer[0] {
        ElementProperty::Number(n) => {
            assert!(n.is_i64(), "expected an int64 name: {n:?}");
            assert_eq!(n.as_i64(), Some(42));
        }
        other => panic!("expected an int64 name, got {other:?}"),
    }

    let labels = graph
        .traversal()
        .v("root/latest")
        .label()
        .to_list()
        .unwrap();
    assert_eq!(labels, vec![ElementProperty::from("symlink")]);
}

#[test]
fn file_tree_edges_link_parent_to_child_and_symlinks_to_targets() {
    let graph = TraversalGraph::file_tree();

    let loop_target = graph
        .traversal()
        .v("root/y/loop")
        .out("points_to")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(loop_target, vec![ElementProperty::from("root")]);

    let latest_target = graph
        .traversal()
        .v("root/latest")
        .out("points_to")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(latest_target, vec![ElementProperty::from("root/author")]);

    // `root/author` is reached by `contains` from `root` and by `points_to` from `root/latest`.
    assert_eq!(
        count(|g| g
            .traversal()
            .v("root/author")
            .in_(None::<()>)
            .count()
            .next()),
        2
    );

    let edge_ids = graph
        .traversal()
        .v("root/latest")
        .out_e(None::<()>)
        .id()
        .to_list()
        .unwrap();
    assert_eq!(
        edge_ids,
        vec![ElementProperty::from("points_to:root/latest")]
    );

    let parent = graph
        .traversal()
        .v("root/src/util/io.rs")
        .in_("contains")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(parent, vec![ElementProperty::from("root/src/util")]);

    let contains_edge = graph
        .traversal()
        .e("contains:root/src/util/io.rs")
        .count()
        .next()
        .unwrap();
    assert_eq!(contains_edge.and_then(|v| v.as_i64()), Some(1));

    // Every non-root vertex has exactly one `contains` parent; the root has none.
    let parents_per_vertex = graph
        .traversal()
        .v(None::<()>)
        .in_("contains")
        .count()
        .next()
        .unwrap()
        .and_then(|v| v.as_i64());
    assert_eq!(parents_per_vertex, Some(36));
    assert_eq!(
        count(|g| g.traversal().v("root").in_("contains").count().next()),
        0
    );
}

#[test]
fn graph_source_file_tree_matches_traversal_graph_file_tree() {
    let graph = GraphSource::file_tree();
    let lock = graph.read();
    let n = lock.traversal().v(None::<()>).count().next().unwrap();
    assert_eq!(n.and_then(|v| v.as_i64()), Some(37));
}

#[cfg(feature = "script")]
mod script {
    use crate::script_host::{ScriptGraphSource, engine, eval_value, graph_scope};
    use graphersal::prelude::*;
    use std::sync::Arc;

    #[test]
    fn file_tree_is_reachable_three_ways_from_rhai() {
        for (script, expected) in [
            ("let t = GraphSource::file_tree(); t.v().count().next()", 37),
            ("let t = file_tree(); t.v().count().next()", 37),
            ("let t = file_tree(); t.e().count().next()", 38),
        ] {
            let graph = Arc::new(GraphSource::empty());
            let value = eval_value(graph, script).unwrap_or_else(|e| panic!("{script}: {e}"));
            assert_eq!(value.as_int().ok(), Some(expected), "{script}: {value:?}");
        }

        // The method form `GraphSource.file_tree()` is called on a `GraphSource` value, which a
        // host pushes into the scope.
        let mut scope = graph_scope(Arc::new(GraphSource::empty()));
        scope.push("GraphSource", ScriptGraphSource::default());
        let n: i64 = engine()
            .eval_with_scope(
                &mut scope,
                "let t = GraphSource.file_tree(); t.v().count().next()",
            )
            .unwrap();
        assert_eq!(n, 37);
    }

    #[test]
    fn file_tree_names_are_readable_from_rhai() {
        let graph = Arc::new(GraphSource::empty());
        let value = eval_value(
            graph,
            r#"let t = file_tree(); t.v("root/author/x/some/f.rs").values("name").next()"#,
        )
        .unwrap();
        assert_eq!(value.into_string().ok().as_deref(), Some("f.rs"));
    }
}
