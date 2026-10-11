//! `optional`, `coalesce`, `filter`, `any` and `all` inside child traversals (anonymous forms,
//! cloned into `repeat()` bodies) give the TinkerPop results, and `.profile()` names the steps
//! that hold child traversals.

use graphersal::prelude::*;

fn names(res: Vec<ElementProperty>) -> Vec<String> {
    let mut out: Vec<String> = res
        .into_iter()
        .map(|v| v.as_str().expect("a name").to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn anonymous_optional_coalesce_and_filter() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let g = || lock.traversal();
    // marko -> vadas, josh; everybody else stays
    let expected = ["josh", "lop", "peter", "ripple", "vadas", "vadas", "josh"];
    let mut expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(
        names(
            g().v(None::<()>)
                .local(__::optional(__::out("knows")).values("name"))
                .to_list()
                .unwrap()
        ),
        expected
    );
    assert_eq!(
        names(
            g().v(None::<()>)
                .local(__::coalesce(vec![__::out("knows"), __::identity()]).values("name"))
                .to_list()
                .unwrap()
        ),
        expected
    );
    assert_eq!(
        names(
            g().v(None::<()>)
                .local(__::filter_t(__::out("created")).values("name"))
                .to_list()
                .unwrap()
        ),
        ["josh", "marko", "peter"]
    );
    // a cloned optional() in a repeat() body: two hops of `knows`, staying put where none
    assert_eq!(
        g().v(None::<()>)
            .repeat(__::optional(__::out("knows")))
            .times(2)
            .count()
            .to_list()
            .unwrap()[0]
            .as_i64(),
        Some(7)
    );
}

#[test]
fn anonymous_any_and_all() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let g = || lock.traversal();
    let count = |res: Vec<ElementProperty>| res[0].as_i64();
    // ages 29, 27, 32, 35
    let ages = || g().v(None::<()>).values("age").fold();
    assert_eq!(
        count(
            ages()
                .where_t(__::any(P::Gt(33)))
                .count()
                .to_list()
                .unwrap()
        ),
        Some(1)
    );
    assert_eq!(
        count(
            ages()
                .where_t(__::any(P::Gt(40)))
                .count()
                .to_list()
                .unwrap()
        ),
        Some(0)
    );
    assert_eq!(
        count(
            ages()
                .where_t(__::all(P::Gt(26)))
                .count()
                .to_list()
                .unwrap()
        ),
        Some(1)
    );
    assert_eq!(
        count(
            ages()
                .where_t(__::all(P::Gt(28)))
                .count()
                .to_list()
                .unwrap()
        ),
        Some(0)
    );
}

fn profile_of(
    build: impl FnOnce(GraphTraversalSource<'_, &TraversalGraph>) -> TraverserResult<TraversalMetrics>,
) -> String {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    build(lock.traversal()).unwrap().to_string()
}

/// Every child-holding step renders its children as the DSL writes them (`__.out()`), never as
/// the `Debug` struct of the child (`AnonymousTraversal { steps: [out()] }`).
#[test]
fn profile_renders_children_as_anonymous_traversals() {
    for (text, expected) in [
        (
            profile_of(|g| g.v(None::<()>).optional(__::out(None::<()>)).profile()),
            "optional(__.out())",
        ),
        (
            profile_of(|g| g.v(None::<()>).local(__::out(None::<()>)).profile()),
            "local(__.out())",
        ),
        (
            profile_of(|g| g.v(None::<()>).not(__::out(None::<()>)).profile()),
            "not(__.out())",
        ),
        (
            profile_of(|g| g.v(None::<()>).filter_t(__::out(None::<()>)).profile()),
            "filter(__.out())",
        ),
        (
            profile_of(|g| {
                g.v(None::<()>)
                    .union(vec![__::out(None::<()>), __::in_(None::<()>)])
                    .profile()
            }),
            "union(__.out(), __.in())",
        ),
    ] {
        assert!(!text.contains("AnonymousTraversal {"), "{text}");
        assert!(text.contains(expected), "{expected}: {text}");
    }
}

/// `any(P)`/`all(P)` render like any other step, without a leading dot.
#[test]
fn profile_renders_any_and_all_without_a_leading_dot() {
    let text = profile_of(|g| {
        g.v(None::<()>)
            .values("age")
            .fold()
            .where_t(__::any(P::Gt(33)))
            .all(P::Gt(0))
            .profile()
    });
    assert!(
        !text.contains("::.any(") && !text.contains("> .any("),
        "{text}"
    );
    assert!(!text.contains("\n.all("), "{text}");
    assert!(text.contains("where(__.any(P.gt(33)))"), "{text}");
    assert!(text.contains("\nall(P.gt(0))"), "{text}");
}
