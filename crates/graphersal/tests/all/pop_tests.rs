//! `Pop` token and `select(Pop, "a", ...)` over the path (pop series, step 1).

use graphersal::prelude::*;

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

type Source<'b> = GraphTraversalSource<'b, &'b TraversalGraph>;

fn s(v: &str) -> ElementProperty {
    ElementProperty::String(v.into())
}

fn list(items: &[&str]) -> ElementProperty {
    ElementProperty::Array(items.iter().map(|v| s(v)).collect())
}

fn run<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    query(lock.traversal()).to_list().expect("query runs")
}

fn sorted(mut v: Vec<ElementProperty>) -> Vec<ElementProperty> {
    v.sort_by_key(|e| format!("{e:?}"));
    v
}

/// The result with bulk merging off and on is the same multiset.
fn merge_invisible<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    let off = query(lock.traversal().with("bulk.merge", false))
        .to_list()
        .unwrap();
    let on = query(lock.traversal().with("bulk.merge", true))
        .to_list()
        .unwrap();
    assert_eq!(sorted(off), sorted(on.clone()));
    on
}

/// `V(1).as(a).repeat(out().as(a)).times(2)`: marko -> josh -> ripple / lop.
fn loop_a<'b>(g: Source<'b>) -> Source<'b> {
    g.v("1").as_("a").repeat(__::out(None).as_("a")).times(2)
}

/// `by()` argument turning a selected list into the list of its element ids.
macro_rules! id_list {
    () => {
        __::unfold().id().fold()
    };
}

// ---- the Rust API ----

#[test]
fn first_and_last_pick_the_oldest_and_the_newest_occurrence() {
    let first = run(|g| loop_a(g).select_pop(Pop::First, "a").id());
    assert_eq!(first, vec![s("1"), s("1")]);
    let mut last = run(|g| loop_a(g).select_pop(Pop::Last, "a").id());
    last.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(last, vec![s("3"), s("5")]);
    // `select("a")` is `Pop::Last`.
    let plain = run(|g| loop_a(g).select("a").id());
    assert_eq!(sorted(plain), sorted(last));
}

#[test]
fn all_and_mixed_list_every_occurrence_in_path_order() {
    for pop in [Pop::All, Pop::Mixed] {
        let lists = run(|g| loop_a(g).select_pop(pop, "a").by(id_list!()));
        assert_eq!(
            sorted(lists),
            vec![list(&["1", "4", "3"]), list(&["1", "4", "5"])],
            "{pop}"
        );
    }
}

#[test]
fn mixed_is_the_bare_value_for_one_occurrence_and_all_wraps_it() {
    let mixed = run(|g| {
        g.v(None)
            .values("name")
            .as_("a")
            .select_pop(Pop::Mixed, "a")
    });
    assert_eq!(mixed.len(), 6);
    assert!(mixed.contains(&s("marko")));
    let all = run(|g| g.v(None).values("name").as_("a").select_pop(Pop::All, "a"));
    assert_eq!(all.len(), 6);
    assert!(all.contains(&list(&["marko"])));
    let first = run(|g| {
        g.v(None)
            .values("name")
            .as_("a")
            .select_pop(Pop::First, "a")
    });
    assert_eq!(sorted(first), sorted(mixed));
}

#[test]
fn two_labels_on_one_position_are_one_occurrence() {
    for analysis in [true, false] {
        let lists = run(|g| {
            g.with("path.analysis", analysis)
                .v(None)
                .as_("a")
                .has("lang", "java")
                .as_("a")
                .select_pop(Pop::All, "a")
                .by(id_list!())
        });
        assert_eq!(lists.len(), 2);
        assert!(
            lists
                .iter()
                .all(|l| matches!(l, ElementProperty::Array(items) if items.len() == 1)),
            "{lists:?}"
        );
    }
}

#[test]
fn several_keys_give_a_map_and_one_missing_key_filters() {
    let maps = run(|g| {
        g.v("1")
            .as_("a")
            .out(None)
            .as_("b")
            .select_pop(Pop::All, vec!["a", "b"])
            .by(id_list!())
    });
    assert_eq!(maps.len(), 3);
    // a key that no `as()` declares is the plan-time diagnostic, never a partial map
    let graph = modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v("1")
        .as_("a")
        .out(None)
        .as_("b")
        .select_pop(Pop::All, vec!["a", "zz_not_here"])
        .to_list()
        .expect_err("undeclared label");
    assert!(format_diagnostic(&err).contains("zz_not_here"));
}

#[test]
fn a_label_that_is_declared_but_absent_on_the_path_filters() {
    let out = run(|g| {
        g.v(None)
            .union(vec![__::as_("a").out(None), __::out(None)])
            .select_pop(Pop::All, "a")
    });
    assert!(!out.is_empty() && out.len() < 12);
}

#[test]
fn a_map_key_wins_over_the_path_and_ignores_the_pop() {
    let names = run(|g| {
        g.v(None)
            .as_("a")
            .out(None)
            .as_("a")
            .value_map(vec!["name"])
            .select_pop(Pop::All, "name")
    });
    // the map key `name` is the list of values, not a one-element wrapper around it
    assert!(
        names
            .iter()
            .all(|n| matches!(n, ElementProperty::Array(items) if items.len() == 1)),
        "{names:?}"
    );
}

#[test]
fn a_side_effect_key_wins_over_the_path() {
    let out = run(|g| {
        g.v(None)
            .has_label("software")
            .aggregate("a")
            .as_("a")
            .select_pop(Pop::First, "a")
            .count()
    });
    assert_eq!(out.len(), 1);
}

#[test]
fn an_undeclared_label_is_the_plan_time_diagnostic_even_with_a_pop() {
    let graph = modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .v(None)
        .select_pop(Pop::First, "a")
        .to_list()
        .expect_err("undeclared");
    let text = format_diagnostic(&err);
    assert!(text.contains("never declared"), "{text}");
    assert!(text.contains("select(Pop.all"), "{text}");
    // a map-producing step disables the check: the traverser filters to nothing
    let none = lock
        .traversal()
        .v(None)
        .value_map(None)
        .select_pop(Pop::Last, "a")
        .to_list()
        .unwrap();
    assert!(none.is_empty());
}

#[test]
fn a_list_valued_occurrence_is_returned_whole_like_tinkerpop() {
    // TinkerPop 3.7.2 ImmutablePath (the path of every live traversal) does not unwrap a
    // list-valued occurrence for first/last/all.
    let lists = |pop| run(move |g| g.v("1").values("name").fold().as_("a").select_pop(pop, "a"));
    assert_eq!(lists(Pop::First), vec![list(&["marko"])]);
    assert_eq!(lists(Pop::Last), vec![list(&["marko"])]);
    assert_eq!(lists(Pop::Mixed), vec![list(&["marko"])]);
    assert_eq!(
        lists(Pop::All),
        vec![ElementProperty::Array(vec![list(&["marko"])])]
    );
}

#[test]
fn mixed_over_several_occurrences_appends_to_a_list_valued_oldest_one() {
    // TinkerPop `Path.get(label)`: when the oldest occurrence is a list, later ones are appended
    // to it instead of building [list, v2].
    let out = run(|g| {
        g.v("1")
            .values("name")
            .fold()
            .as_("a")
            .constant("x")
            .as_("a")
            .select_pop(Pop::Mixed, "a")
    });
    assert_eq!(out, vec![list(&["marko", "x"])]);
    let all = run(|g| {
        g.v("1")
            .values("name")
            .fold()
            .as_("a")
            .constant("x")
            .as_("a")
            .select_pop(Pop::All, "a")
    });
    assert_eq!(
        all,
        vec![ElementProperty::Array(vec![list(&["marko"]), s("x")])]
    );
}

#[test]
fn bulk_merging_does_not_change_a_pop_result() {
    let r = merge_invisible(|g| loop_a(g).select_pop(Pop::All, "a").by(id_list!()));
    assert_eq!(r.len(), 2);
    merge_invisible(|g| {
        g.v(None)
            .as_("a")
            .out(None)
            .as_("a")
            .out(None)
            .as_("a")
            .select_pop(Pop::Mixed, "a")
            .by(id_list!())
    });
}

#[test]
fn a_project_child_sees_the_incoming_path() {
    let rows = run(|g| {
        g.v(None)
            .as_("v")
            .both(None)
            .as_("v")
            .project(vec!["s", "t", "p"])
            .by(__::select_pop(Pop::First, "v").id())
            .by(__::select_pop(Pop::Last, "v").id())
            .by(__::select_pop(Pop::All, "v").by(id_list!()))
    });
    assert_eq!(rows.len(), 12);
}

#[test]
fn profile_shows_the_full_path_for_pop_forms_only() {
    let graph = modern();
    let lock = graph.read();
    let all = lock
        .traversal()
        .v(None)
        .as_("a")
        .out(None)
        .as_("a")
        .select_pop(Pop::All, "a")
        .profile()
        .unwrap()
        .to_string();
    assert!(all.contains("select(Pop.all, \"a\")"), "{all}");
    assert!(all.contains("[path: full]"), "{all}");
    assert!(!all.contains("labels(a)"), "{all}");
    let last = lock
        .traversal()
        .v(None)
        .as_("a")
        .out(None)
        .as_("a")
        .select("a")
        .profile()
        .unwrap()
        .to_string();
    assert!(last.contains("labels(a)"), "{last}");
    assert!(!last.contains("[path: full]"), "{last}");
    let last_pop = lock
        .traversal()
        .v(None)
        .as_("a")
        .out(None)
        .as_("a")
        .select_pop(Pop::Last, "a")
        .profile()
        .unwrap()
        .to_string();
    assert!(last_pop.contains("labels(a)"), "{last_pop}");
}

#[test]
fn no_pop_query_records_no_full_path() {
    let graph = modern();
    let lock = graph.read();
    let text = lock
        .traversal()
        .v(None)
        .out(None)
        .out(None)
        .count()
        .profile()
        .unwrap()
        .to_string();
    assert!(!text.contains("[path:"), "{text}");
}

#[test]
fn rust_api_and_defaults() {
    assert_eq!(Pop::default(), Pop::Last);
    assert_eq!(Pop::Mixed.to_string(), "mixed");
    assert_eq!(Pop::First.name(), "first");
    let r = run(|g| {
        g.v("1")
            .as_("a")
            .out(None)
            .as_("b")
            .select_pop(Pop::First, vec!["a", "b"])
            .by(__::id())
    });
    assert_eq!(r.len(), 3);
}

// ---- select(Pop, traversal), step 2 ----

/// `V().as(a).out(knows).as(a)`: marko -> vadas / josh.
fn knows_a<'b>(g: Source<'b>) -> Source<'b> {
    g.v(None).as_("a").out("knows").as_("a")
}

macro_rules! name_list {
    () => {
        __::unfold().values("name").fold()
    };
}

#[test]
fn a_key_traversal_selects_every_occurrence() {
    let expected = vec![list(&["marko", "josh"]), list(&["marko", "vadas"])];
    let got = run(|g| {
        knows_a(g)
            .select_traversal(Pop::All, __::constant("a"))
            .by(name_list!())
    });
    assert_eq!(sorted(got), expected);
    // plain select(traversal) is Pop::Last, and equals select("a")
    let plain = run(|g| {
        knows_a(g)
            .select_traversal(Pop::Last, __::constant("a"))
            .values("name")
    });
    let direct = run(|g| knows_a(g).select("a").values("name"));
    assert_eq!(sorted(plain), sorted(direct.clone()));
    assert_eq!(sorted(direct), vec![s("josh"), s("vadas")]);
    let first = run(|g| {
        knows_a(g)
            .select_traversal(Pop::First, __::constant("a"))
            .values("name")
    });
    assert_eq!(first, vec![s("marko"), s("marko")]);
    let mixed = run(|g| {
        knows_a(g)
            .select_traversal(Pop::Mixed, __::constant("a"))
            .by(name_list!())
    });
    assert_eq!(sorted(mixed), expected);
    let anon = run(|g| {
        knows_a(g).local(__::select_traversal(Pop::First, __::constant("a")).values("name"))
    });
    assert_eq!(anon, vec![s("marko"), s("marko")]);
}

#[test]
fn a_key_traversal_that_yields_nothing_or_a_non_string_filters() {
    assert!(run(|g| knows_a(g).select_traversal(Pop::All, __::has("nope", "x"))).is_empty());
    assert!(run(|g| knows_a(g).select_traversal(Pop::All, __::constant(1))).is_empty());
    // a label that is declared but not on the path filters too
    assert!(
        run(|g| knows_a(g)
            .select_traversal(Pop::All, __::constant("b"))
            .as_("b"))
        .is_empty()
    );
}

#[test]
fn a_key_traversal_reads_the_incoming_path_and_names_maps_and_side_effects() {
    // the key is the text stored under label "k" on the path
    let out = run(|g| {
        g.v("1")
            .values("name")
            .as_("k")
            .constant("a")
            .as_("k")
            .select_traversal(Pop::All, __::select_pop(Pop::First, "k"))
    });
    // first "k" is "marko", which is no label: filtered
    assert!(out.is_empty(), "{out:?}");
    let out = run(|g| {
        g.v("1")
            .constant("a")
            .as_("k")
            .select_traversal(Pop::All, __::select("k"))
    });
    // key "a" is no label either (declared nowhere): filtered, not an error (constant disables the check)
    assert!(out.is_empty());
    // a key naming a map key ignores the pop
    let names = run(|g| {
        g.v("1")
            .as_("a")
            .value_map(vec!["name"])
            .select_traversal(Pop::All, __::constant("name"))
    });
    assert_eq!(names, vec![list(&["marko"])]);
    // a key naming a side effect ignores the pop
    let side = run(|g| {
        g.v(None)
            .has_label("software")
            .aggregate("x")
            .as_("x")
            .select_traversal(Pop::First, __::constant("x"))
            .count()
    });
    assert_eq!(side.len(), 1);
}

#[test]
fn by_applies_to_the_selected_list_and_the_last_by_wins() {
    let last = run(|g| {
        knows_a(g)
            .select_traversal(Pop::All, __::constant("a"))
            .by(__::count_scoped(Scope::Local))
            .by(name_list!())
    });
    assert_eq!(
        sorted(last),
        vec![list(&["marko", "josh"]), list(&["marko", "vadas"])]
    );
    // an unproductive by() filters the traverser
    let none = run(|g| {
        knows_a(g)
            .select_traversal(Pop::Last, __::constant("a"))
            .by("age")
            .by("nope")
    });
    assert!(none.is_empty());
    // a property of a list is the error with the new help
    let graph = modern();
    let lock = graph.read();
    let err = knows_a(lock.traversal())
        .select_traversal(Pop::All, __::constant("a"))
        .by("name")
        .to_list()
        .expect_err("list has no name");
    assert!(format_diagnostic(&err).contains("by(__.unfold().values(\"name\").fold())"));
}

#[test]
fn the_key_traversal_forces_the_full_path_and_merging_is_invisible() {
    let graph = modern();
    let lock = graph.read();
    let text = knows_a(lock.traversal())
        .select_traversal(Pop::All, __::constant("a"))
        .profile()
        .unwrap()
        .to_string();
    assert!(
        text.contains(r#"select(Pop.all, __.constant("a"))"#),
        "{text}"
    );
    assert!(text.contains("[path: full]"), "{text}");
    let off = knows_a(lock.traversal().with("path.analysis", false))
        .select_traversal(Pop::All, __::constant("a"))
        .by(name_list!())
        .to_list()
        .unwrap();
    let on = merge_invisible(|g| {
        knows_a(g)
            .select_traversal(Pop::All, __::constant("a"))
            .by(name_list!())
    });
    assert_eq!(sorted(off), sorted(on));
}

#[test]
fn the_tail_of_a_mixed_select() {
    fn q<'b>(g: Source<'b>) -> Source<'b> {
        g.v(None).as_("a").out(None).as_("a").out(None).as_("a")
    }
    fn by_names<'b>(g: Source<'b>) -> Source<'b> {
        q(g).select_pop(Pop::Mixed, "a").by(name_list!())
    }
    assert_eq!(
        sorted(run(|g| by_names(g).tail_scoped(Scope::Local, 2))),
        vec![list(&["josh", "lop"]), list(&["josh", "ripple"])]
    );
    assert_eq!(
        sorted(run(|g| by_names(g).tail_scoped(Scope::Local, 1).unfold())),
        vec![s("lop"), s("ripple")]
    );
    // limit(local, 0) leaves an empty list, and tail(local, 1) of an empty list is an empty list
    // (emitted, not dropped): the engine matches TinkerPop, nothing to fix there.
    let empties = run(|g| {
        q(g).select_pop(Pop::Mixed, "a")
            .by(__::limit_scoped(Scope::Local, 0))
            .tail_scoped(Scope::Local, 1)
    });
    assert_eq!(
        empties,
        vec![
            ElementProperty::Array(vec![]),
            ElementProperty::Array(vec![])
        ]
    );
}

// ---- Rhai ----

#[cfg(feature = "script")]
mod dsl {
    use super::*;
    use std::sync::Arc;

    fn eval_dyn(script: &str) -> Result<rhai::Dynamic, String> {
        crate::script_host::eval_value(Arc::new(modern()), script).map_err(|e| e.to_string())
    }

    fn eval(script: &str) -> Result<Vec<ElementProperty>, String> {
        let graph = Arc::new(modern());
        let engine = crate::script_host::engine();
        let mut scope = crate::script_host::graph_scope(graph.clone());
        let gts = engine
            .eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(&mut scope, script)
            .map_err(|e| e.to_string())?;
        let lock = graph.read();
        gts.ast_convert_to_gts(&*lock)
            .to_list()
            .map_err(|e| format_diagnostic(&e))
    }

    fn ok(script: &str) -> Vec<ElementProperty> {
        eval(script).unwrap_or_else(|e| panic!("{script}: {e}"))
    }

    fn pop_of(script: &str) -> Pop {
        eval_dyn(script)
            .unwrap_or_else(|e| panic!("{script}: {e}"))
            .try_cast::<Pop>()
            .unwrap_or_else(|| panic!("{script} is not a Pop"))
    }

    #[test]
    fn every_notation_evaluates_to_the_token() {
        for (name, pop) in [
            ("first", Pop::First),
            ("last", Pop::Last),
            ("all", Pop::All),
            ("mixed", Pop::Mixed),
        ] {
            let pascal = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
            for script in [
                format!("Pop.{name}"),
                format!("Pop::{name}"),
                format!("Pop.{pascal}"),
                format!("Pop::{pascal}"),
                name.to_string(),
            ] {
                assert_eq!(pop_of(&script), pop, "{script}");
            }
        }
        assert!(eval_dyn("Pop.nope").is_err());
        assert!(eval_dyn("Pop::nope").is_err());
        assert!(
            eval_dyn("Pop.first == Pop::first")
                .unwrap()
                .as_bool()
                .unwrap()
        );
        assert!(eval_dyn("Pop.first != Pop.all").unwrap().as_bool().unwrap());
        assert_eq!(
            eval_dyn("Pop.all.to_string()")
                .unwrap()
                .into_string()
                .unwrap(),
            "Pop.all"
        );
    }

    #[test]
    fn select_with_a_pop_in_every_spelling() {
        let expected = vec![list(&["1", "4", "3"]), list(&["1", "4", "5"])];
        for pop in ["Pop.all", "Pop::all", "Pop.All", "all"] {
            let script = format!(
                r#"g.V("1").as("a").repeat(__.out().as("a")).times(2).select({pop}, "a").by(__.unfold().id().fold())"#
            );
            assert_eq!(sorted(ok(&script)), expected, "{script}");
        }
        for step in ["as", "as_"] {
            let script = format!(
                r#"g.v("1").{step}("a").repeat(__.out().{step}("a")).times(2).select(Pop.first, "a").id()"#
            );
            assert_eq!(ok(&script), vec![s("1"), s("1")], "{script}");
        }
        assert_eq!(
            sorted(ok(
                r#"g.V("1").as("a").repeat(__.out().as("a")).times(2).select(last, "a").id()"#
            )),
            vec![s("3"), s("5")]
        );
        assert_eq!(
            sorted(ok(
                r#"g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop.mixed, ["a"]).by(__.unfold().id().fold())"#
            )),
            expected
        );
        let multi = ok(r#"g.V("1").as("a").out().as("b").select(Pop.first, "a", "b").by(__.id())"#);
        assert_eq!(multi.len(), 3);
    }

    #[test]
    fn a_pop_is_not_stringified_into_labels() {
        // a label really named `first` exists: select(Pop.first, "a") must still read `a`
        let out = ok(r#"g.V("1").as("first").out().as("a").select(Pop.first, "a").id()"#);
        assert_eq!(out.len(), 3);
        // and select("first", "a") reads the two labels as a map
        let map = ok(r#"g.V("1").as("first").out().as("a").select("first", "a").by(__.id())"#);
        assert_eq!(map.len(), 3);
        // a label named `first` that does not exist is not what select(Pop.first, "a") touches
        assert!(eval(r#"g.V("1").as("a").select(Pop.first, "a").id()"#).is_ok());
    }

    #[test]
    fn malformed_pop_calls_are_argument_mismatches() {
        for script in [
            r#"g.V().as("a").select(Pop.first)"#,
            r#"g.V().as("a").select("a", Pop.first)"#,
            r#"g.V().as("a").select(Pop.first, 5)"#,
            r#"g.V().as("a").select(Pop.first, ["a", 5])"#,
        ] {
            let err = eval(script).expect_err(script);
            assert!(
                err.contains("Argument mismatch for step 'select'"),
                "{script}: {err}"
            );
            assert!(err.contains("select(Pop.all, \"a\")"), "{script}: {err}");
        }
    }

    #[test]
    fn parameters_a_pop_and_the_steps_of_the_same_name() {
        let graph = Arc::new(modern());
        let mut params = std::collections::HashMap::new();
        params.insert("Pop".to_string(), s("x"));
        assert!(crate::script_host::eval_value_with_params(graph.clone(), "1", params).is_err());
        let mut params = std::collections::HashMap::new();
        params.insert("first".to_string(), s("a"));
        let value = crate::script_host::eval_value_with_params(graph, "first", params).unwrap();
        assert_eq!(value.into_string().unwrap(), "a");
        // the constants do not break the step all(P), a variable of the same name, or Rhai arrays
        assert!(eval("let all = 1; g.V().all(P.gt(0))").is_ok());
        assert!(
            eval_dyn("[1, 2].all(|x| x > 0)")
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn project_children_and_anonymous_traversals_take_a_pop() {
        let rows = ok(
            r#"g.V().as("v").both().as("v").project("s","t","p").by(__.select(first, "v").id()).by(__.select(last, "v").id()).by(__.select(Pop.all, "v").by(__.unfold().id().fold()))"#,
        );
        assert_eq!(rows.len(), 12);
    }

    #[test]
    fn select_pop_with_a_traversal_key_in_every_notation() {
        let expected = vec![list(&["marko", "josh"]), list(&["marko", "vadas"])];
        for pop in ["Pop.all", "Pop::all", "Pop.All", "Pop::All", "all"] {
            let script = format!(
                r#"g.V().as("a").out("knows").as("a").select({pop}, __.constant("a")).by(__.unfold().values("name").fold())"#
            );
            assert_eq!(sorted(ok(&script)), expected, "{script}");
        }
        let first = ok(
            r#"g.V().as("a").out("knows").as("a").select(first, __.constant("a")).values("name")"#,
        );
        assert_eq!(first, vec![s("marko"), s("marko")]);
        let plain =
            ok(r#"g.V().as("a").out("knows").as("a").select(__.constant("a")).values("name")"#);
        let direct = ok(r#"g.V().as("a").out("knows").as("a").select("a").values("name")"#);
        assert_eq!(sorted(plain), sorted(direct));
        // the array form of select still reads labels
        assert_eq!(ok(r#"g.V("1").as("a").select(["a"]).id()"#), vec![s("1")]);
        // empty and non-string key traversals filter
        assert!(ok(r#"g.V().as("a").select(Pop.all, __.has("nope"))"#).is_empty());
        assert!(ok(r#"g.V().as("a").select(Pop.all, __.constant(1))"#).is_empty());
        // anonymous use
        let rows = ok(
            r#"g.V().as("v").both().as("v").project("p").by(__.select(Pop.all, __.constant("v")).by(__.unfold().id().fold()))"#,
        );
        assert_eq!(rows.len(), 12);
        let local = ok(
            r#"g.V().as("a").out("knows").as("a").local(__.select(Pop.first, __.constant("a")).values("name"))"#,
        );
        assert_eq!(local, vec![s("marko"), s("marko")]);
    }

    #[test]
    fn a_traversal_key_with_other_keys_is_an_argument_mismatch() {
        for script in [
            r#"g.V().as("a").select(Pop.all, __.constant("a"), "b")"#,
            r#"g.V().as("a").select(Pop.all, "b", __.constant("a"))"#,
        ] {
            let err = eval(script).expect_err(script);
            assert!(
                err.contains("Argument mismatch for step 'select'"),
                "{script}: {err}"
            );
            assert!(
                err.contains("select(Pop.all, __.constant"),
                "{script}: {err}"
            );
        }
    }

    #[test]
    fn the_tail_scenarios_and_by_over_a_list_help() {
        let base = r#"g.V().as("a").out().as("a").out().as("a").select(Pop.mixed, "a")"#;
        let by = r#".by(__.unfold().values("name").fold())"#;
        assert_eq!(
            sorted(ok(&format!("{base}{by}.tail(Scope.local, 2)"))),
            vec![list(&["josh", "lop"]), list(&["josh", "ripple"])]
        );
        assert_eq!(
            sorted(ok(&format!("{base}{by}.tail(Scope.local)"))).len(),
            2
        );
        assert_eq!(
            sorted(ok(&format!("{base}{by}.tail(Scope.local, 1).unfold()"))),
            vec![s("lop"), s("ripple")]
        );
        assert_eq!(
            ok(&format!(
                "{base}.by(__.limit(Scope.local, 0)).tail(Scope.local, 1)"
            )),
            vec![
                ElementProperty::Array(vec![]),
                ElementProperty::Array(vec![])
            ]
        );
        let err = eval(&format!(r#"{base}.by("name")"#)).expect_err("a list has no name");
        assert!(
            err.contains("by(__.unfold().values(\"name\").fold())"),
            "{err}"
        );
    }
}
