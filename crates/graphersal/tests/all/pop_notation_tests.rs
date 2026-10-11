//! The dual-notation matrix of `Pop` (pop series, step 3): every accepted spelling of every `Pop`
//! member through every `Pop`-taking form, in both receiver spellings, compared against the Rust
//! API. A Gremlin query must paste into the Rhai DSL, and a Rust-style one must work too.
#![cfg(feature = "script")]

use std::sync::Arc;

use graphersal::prelude::*;

type Source<'b> = GraphTraversalSource<'b, &'b TraversalGraph>;

const POPS: [(&str, Pop); 4] = [
    ("first", Pop::First),
    ("last", Pop::Last),
    ("all", Pop::All),
    ("mixed", Pop::Mixed),
];

/// The five spellings of one member: `Pop.m`, `Pop::m`, `Pop.M`, `Pop::M` and the bare `m`.
fn spellings(name: &str) -> [String; 5] {
    let pascal = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
    [
        format!("Pop.{name}"),
        format!("Pop::{name}"),
        format!("Pop.{pascal}"),
        format!("Pop::{pascal}"),
        name.to_string(),
    ]
}

/// The two receiver spellings of the steps the matrix queries use.
const SNAKE: [(&str, &str); 7] = [
    ("{V}", "v"),
    ("{has_label}", "has_label"),
    ("{as}", "as_"),
    ("{out_e}", "out_e"),
    ("{in_v}", "in_v"),
    ("{value_map}", "value_map"),
    ("{out}", "out"),
];
const CAMEL: [(&str, &str); 7] = [
    ("{V}", "V"),
    ("{has_label}", "hasLabel"),
    ("{as}", "as"),
    ("{out_e}", "outE"),
    ("{in_v}", "inV"),
    ("{value_map}", "valueMap"),
    ("{out}", "out"),
];

fn receiver(template: &str, table: &[(&str, &str); 7]) -> String {
    table
        .iter()
        .fold(template.to_string(), |q, (k, v)| q.replace(k, v))
}

fn modern() -> Graph {
    GraphSource::tinkerpop_modern()
}

thread_local! {
    // Building an engine registers the whole DSL (~15 ms in a debug build); the matrix runs
    // hundreds of scripts, so each test thread builds one engine and reuses it.
    static ENGINE: rhai::Engine = crate::script_host::engine();
}

fn eval_dyn(script: &str) -> Result<rhai::Dynamic, String> {
    ENGINE.with(|engine| {
        let mut scope = crate::script_host::graph_scope(Arc::new(modern()));
        engine
            .eval_with_scope::<rhai::Dynamic>(&mut scope, script)
            .map_err(|e| e.to_string())
    })
}

fn eval(script: &str) -> Result<Vec<ElementProperty>, String> {
    let graph = Arc::new(modern());
    let mut scope = crate::script_host::graph_scope(graph.clone());
    let gts = ENGINE
        .with(|engine| {
            engine.eval_with_scope::<crate::script_host::ScriptGraphTraversalSource>(
                &mut scope, script,
            )
        })
        .map_err(|e| e.to_string())?;
    let lock = graph.read();
    gts.ast_convert_to_gts(&*lock)
        .to_list()
        .map_err(|e| format_diagnostic(&e))
}

fn ok(script: &str) -> Vec<ElementProperty> {
    eval(script).unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn sorted(mut v: Vec<ElementProperty>) -> Vec<ElementProperty> {
    v.sort_by_key(|e| format!("{e:?}"));
    v
}

fn run<F>(query: F) -> Vec<ElementProperty>
where
    F: for<'b> Fn(Source<'b>) -> Source<'b>,
{
    let graph = modern();
    let lock = graph.read();
    query(lock.traversal()).to_list().expect("query runs")
}

fn s(v: &str) -> ElementProperty {
    ElementProperty::String(v.into())
}

macro_rules! id_list {
    () => {
        __::unfold().id().fold()
    };
}

macro_rules! name_list {
    () => {
        __::unfold().values("name").fold()
    };
}

/// One matrix form: the Rhai query template (`{P}` is the pop, `{V}`... the receivers) and the Rust
/// API query for a pop. The Rhai result must equal the Rust one for every cell.
struct Form {
    label: &'static str,
    rhai: &'static str,
    rust: fn(Pop) -> Vec<ElementProperty>,
}

fn forms() -> Vec<Form> {
    vec![
        Form {
            label: "one label",
            rhai: r#"g.{V}("1").{as}("a").repeat(__.{out}().{as}("a")).times(2).select({P}, "a").by(__.unfold().id().fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v("1")
                        .as_("a")
                        .repeat(__::out(None).as_("a"))
                        .times(2)
                        .select_pop(pop, "a")
                        .by(id_list!())
                })
            },
        },
        Form {
            label: "one label, no by",
            rhai: r#"g.{V}("1").{as}("a").repeat(__.{out}().{as}("a")).times(2).select({P}, "a").count()"#,
            rust: |pop| {
                run(|g| {
                    g.v("1")
                        .as_("a")
                        .repeat(__::out(None).as_("a"))
                        .times(2)
                        .select_pop(pop, "a")
                        .count()
                })
            },
        },
        Form {
            label: "several labels",
            rhai: r#"g.{V}("1").{as}("a").{out}().{as}("b").select({P}, "a", "b").by(__.unfold().id().fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v("1")
                        .as_("a")
                        .out(None)
                        .as_("b")
                        .select_pop(pop, vec!["a", "b"])
                        .by(id_list!())
                })
            },
        },
        Form {
            label: "label array",
            rhai: r#"g.{V}("1").{as}("a").{out}().{as}("b").select({P}, ["a", "b"]).by(__.unfold().id().fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v("1")
                        .as_("a")
                        .out(None)
                        .as_("b")
                        .select_pop(pop, vec!["a", "b"])
                        .by(id_list!())
                })
            },
        },
        Form {
            label: "by over the list",
            rhai: r#"g.{V}("1").{as}("a").repeat(__.{out}().{as}("a")).times(2).select({P}, "a").by(__.unfold().values("name").fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v("1")
                        .as_("a")
                        .repeat(__::out(None).as_("a"))
                        .times(2)
                        .select_pop(pop, "a")
                        .by(name_list!())
                })
            },
        },
        Form {
            label: "by over several labels",
            rhai: r#"g.{V}().{as}("a").{out}("knows").{as}("b").select({P}, "a", "b").by(__.unfold().values("name").fold()).by(__.unfold().id().fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v(None)
                        .as_("a")
                        .out("knows")
                        .as_("b")
                        .select_pop(pop, vec!["a", "b"])
                        .by(name_list!())
                        .by(id_list!())
                })
            },
        },
        Form {
            label: "anonymous in project",
            rhai: r#"g.{V}().{as}("v").both().{as}("v").project("p").by(__.select({P}, "v").by(__.unfold().id().fold()))"#,
            rust: |pop| {
                run(|g| {
                    g.v(None)
                        .as_("v")
                        .both(None)
                        .as_("v")
                        .project(vec!["p"])
                        .by(__::select_pop(pop, "v").by(id_list!()))
                })
            },
        },
        Form {
            label: "traversal key",
            rhai: r#"g.{V}().{as}("a").{out}("knows").{as}("a").select({P}, __.constant("a")).by(__.unfold().values("name").fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v(None)
                        .as_("a")
                        .out("knows")
                        .as_("a")
                        .select_traversal(pop, __::constant("a"))
                        .by(name_list!())
                })
            },
        },
        Form {
            label: "anonymous traversal key",
            rhai: r#"g.{V}().{as}("a").{out}("knows").{as}("a").local(__.select({P}, __.constant("a")).by(__.unfold().values("name").fold()))"#,
            rust: |pop| {
                run(|g| {
                    g.v(None)
                        .as_("a")
                        .out("knows")
                        .as_("a")
                        .local(__::select_traversal(pop, __::constant("a")).by(name_list!()))
                })
            },
        },
        Form {
            label: "edges and labels on the path",
            rhai: r#"g.{V}().{has_label}("person").{as}("a").{out_e}("knows").{as}("a").{in_v}().{as}("a").select({P}, "a").by(__.unfold().label().fold())"#,
            rust: |pop| {
                run(|g| {
                    g.v(None)
                        .has_label("person")
                        .as_("a")
                        .out_e("knows")
                        .as_("a")
                        .in_v()
                        .as_("a")
                        .select_pop(pop, "a")
                        .by(__::unfold().label().fold())
                })
            },
        },
    ]
}

#[test]
fn every_spelling_of_every_pop_through_every_form_equals_the_rust_api() {
    let mut cells = 0usize;
    for form in forms() {
        for (name, pop) in POPS {
            let expected = sorted((form.rust)(pop));
            for spelling in spellings(name) {
                for (receivers, table) in [("snake_case", &SNAKE), ("camelCase", &CAMEL)] {
                    let script = receiver(&form.rhai.replace("{P}", &spelling), table);
                    let got = sorted(ok(&script));
                    assert_eq!(got, expected, "[{}] {receivers}: {script}", form.label);
                    cells += 1;
                }
            }
        }
    }
    assert_eq!(cells, 10 * 4 * 5 * 2);
}

#[test]
fn the_forms_are_not_vacuous() {
    // a matrix that compares empty results proves nothing: every form yields rows for every pop
    for form in forms() {
        for (name, pop) in POPS {
            let rows = (form.rust)(pop);
            assert!(!rows.is_empty(), "[{}] {name} returns nothing", form.label);
        }
    }
    // and the pops differ where they must
    let q = |pop| {
        run(move |g| {
            g.v("1")
                .as_("a")
                .repeat(__::out(None).as_("a"))
                .times(2)
                .select_pop(pop, "a")
                .by(id_list!())
        })
    };
    assert_ne!(sorted(q(Pop::First)), sorted(q(Pop::Last)));
    assert_ne!(sorted(q(Pop::Last)), sorted(q(Pop::All)));
}

#[test]
fn value_map_and_modulators_in_both_spellings() {
    // a map key wins over the path and ignores the pop; valueMap/value_map spell the same
    for (name, _) in POPS {
        let mut results = Vec::new();
        for spelling in spellings(name) {
            for (table, label) in [(&SNAKE, "snake"), (&CAMEL, "camel")] {
                let q = receiver(
                    &format!(
                        r#"g.{{V}}().{{has_label}}("person").{{value_map}}("name").select({spelling}, "name")"#
                    ),
                    table,
                );
                results.push(sorted(ok(&q)));
                assert!(!results.last().unwrap().is_empty(), "{label} {q}");
            }
        }
        assert!(
            results.windows(2).all(|w| w[0] == w[1]),
            "{name}: {results:?}"
        );
    }
}

#[test]
fn rust_token_with_a_snake_receiver_and_gremlin_token_with_a_camel_receiver() {
    // the mixed notations a user pastes
    let expected = sorted(run(|g| {
        g.v("1")
            .as_("a")
            .repeat(__::out(None).as_("a"))
            .times(2)
            .select_pop(Pop::First, "a")
            .id()
    }));
    assert_eq!(
        sorted(ok(
            r#"g.v("1").as_("a").repeat(__.out().as_("a")).times(2).select(Pop::First, "a").id()"#
        )),
        expected
    );
    assert_eq!(
        sorted(ok(
            r#"g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop.first, "a").id()"#
        )),
        expected
    );
    assert_eq!(
        sorted(ok(
            r#"g.v("1").as_("a").repeat(__.out().as_("a")).times(2).select(first, "a").id()"#
        )),
        expected
    );
    assert_eq!(
        sorted(ok(
            r#"g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop::First, "a").id()"#
        )),
        expected
    );
}

#[test]
fn every_spelling_evaluates_to_the_token_and_prints_its_tinkerpop_name() {
    for (name, pop) in POPS {
        for spelling in spellings(name) {
            let value = eval_dyn(&spelling).unwrap_or_else(|e| panic!("{spelling}: {e}"));
            assert_eq!(value.clone().try_cast::<Pop>(), Some(pop), "{spelling}");
            // the token prints as TinkerPop writes it (`Pop.first` is how the DSL renders it)
            let printed = eval_dyn(&format!("({spelling}).to_string()"))
                .unwrap()
                .into_string()
                .unwrap();
            assert!(printed.ends_with(name), "{spelling} prints {printed}");
            assert_eq!(pop.to_string(), name);
            // equality across spellings
            for other in spellings(name) {
                let eq = eval_dyn(&format!("({spelling}) == ({other})")).unwrap();
                assert!(eq.as_bool().unwrap(), "{spelling} == {other}");
            }
        }
    }
    for bad in [
        "Pop.nope",
        "Pop::nope",
        "Pop.FIRST",
        "Pop::ALL",
        "Pop.first()",
        "Pop.First2",
    ] {
        assert!(eval_dyn(bad).is_err(), "{bad} must fail");
    }
}

#[test]
fn rust_pops_equal_the_dsl_tokens() {
    for (name, pop) in POPS {
        assert_eq!(pop.to_string(), name);
        assert_eq!(
            eval_dyn(&format!("Pop.{name}")).unwrap().try_cast::<Pop>(),
            Some(pop)
        );
    }
    // the same query through select_pop, select_traversal and __::select_pop
    for (name, pop) in POPS {
        let direct = sorted(run(|g| {
            g.v("1")
                .as_("a")
                .repeat(__::out(None).as_("a"))
                .times(2)
                .select_pop(pop, "a")
                .by(id_list!())
        }));
        let via_key = sorted(run(|g| {
            g.v("1")
                .as_("a")
                .repeat(__::out(None).as_("a"))
                .times(2)
                .select_traversal(pop, __::constant("a"))
                .by(id_list!())
        }));
        let anon = sorted(run(|g| {
            g.v("1")
                .as_("a")
                .repeat(__::out(None).as_("a"))
                .times(2)
                .local(__::select_pop(pop, "a").by(id_list!()))
        }));
        assert_eq!(direct, via_key, "{name}");
        assert_eq!(direct, anon, "{name}");
        let script = format!(
            r#"g.V("1").as("a").repeat(__.out().as("a")).times(2).select(Pop.{name}, "a").by(__.unfold().id().fold())"#
        );
        assert_eq!(sorted(ok(&script)), direct, "{name}");
    }
}

// ---- shadowing and conflicts (design section 3, rules a-c) ----

#[test]
fn a_script_variable_named_like_a_pop_shadows_the_bare_constant() {
    // documented hazard: `first` is now the string "x", so the call reads the labels "x" and "a"
    let rows =
        ok(r#"let first = "x"; g.V("1").as("x").out().as("a").select(first, "a").by(__.id())"#);
    assert_eq!(rows.len(), 3);
    let text = format!("{rows:?}");
    assert!(text.contains("\"x\"") && text.contains("\"a\""), "{text}");
    // the qualified spelling is immune
    let rows = ok(r#"let first = "x"; g.V("1").as("a").select(Pop.first, "a").id()"#);
    assert_eq!(rows, vec![s("1")]);
}

#[test]
fn the_step_all_and_a_variable_all_coexist_with_the_constant() {
    assert_eq!(ok("let all = 1; g.V().all(P.gt(0)).count()").len(), 1);
    assert_eq!(
        ok(r#"let all = 1; g.V("1").as("a").select(Pop.all, "a").count()"#),
        vec![ElementProperty::from(1i64)]
    );
    assert!(
        eval_dyn("[1, 2].all(|x| x > 0)")
            .unwrap()
            .as_bool()
            .unwrap()
    );
}

#[test]
fn a_parameter_named_pop_is_rejected_and_the_bare_names_are_accepted() {
    let graph = Arc::new(modern());
    let mut params = std::collections::HashMap::new();
    params.insert("Pop".to_string(), s("x"));
    assert!(crate::script_host::eval_value_with_params(graph.clone(), "1", params).is_err());
    for (name, _) in POPS {
        let mut params = std::collections::HashMap::new();
        params.insert(name.to_string(), s("p"));
        let plain = crate::script_host::eval_value_with_params(graph.clone(), name, params.clone())
            .unwrap_or_else(|e| panic!("parameter {name} must be accepted: {e}"));
        assert_eq!(plain.into_string().unwrap(), "p");
        // the qualified token still works next to the parameter
        let value = crate::script_host::eval_value_with_params(
            graph.clone(),
            r#"g.V("1").as("a").select(Pop.first, "a").id().to_list()"#,
            params,
        )
        .unwrap_or_else(|e| panic!("select with parameter {name}: {e}"));
        let list = value.into_array().unwrap();
        assert_eq!(list.len(), 1, "{name}");
    }
}

#[test]
fn a_label_named_like_a_pop_is_a_normal_label() {
    for (name, _) in POPS {
        let script = format!(r#"g.V("1").as("{name}").select(Pop.first, "{name}").id()"#);
        assert_eq!(ok(&script), vec![s("1")], "{script}");
    }
}

#[test]
fn a_lone_pop_or_a_pop_in_the_wrong_place_fails_with_the_help() {
    for spelling in spellings("first") {
        for script in [
            format!(r#"g.V().as("a").select({spelling})"#),
            format!(r#"g.V().as("a").select("a", {spelling})"#),
            format!(r#"g.V().as("a").select({spelling}, 5)"#),
        ] {
            let err = eval(&script).expect_err(&script);
            assert!(
                err.contains("Argument mismatch for step 'select'"),
                "{script}: {err}"
            );
            assert!(err.contains("select(Pop.all, \"a\")"), "{script}: {err}");
        }
    }
}
