//! Differential oracle for the optimizer: an optimizer rule must never change a query's result.
//!
//! Every query of the corpus runs on a fresh graph once with every rule disabled (the oracle,
//! like `bulk.merge = false` is for merging) and then, for every registered rule, with all rules
//! on, with all rules but that one, and with only that one. The query's result and the graph
//! state afterwards (vertex, edge and property counts) must be the same in every run. The rule
//! list comes from the optimizer registry (`optimizer_rule_names()`), so a new rule is covered
//! without touching this file.
//!
//! The corpus targets the shapes the rules rewrite (source steps with `has_label`/`has_id`,
//! adjacent filters, `count()`/`groupCount()` after a source step, `outE().hasLabel()`, nested
//! `where()`, adjacency chains), on the modern graph extended with multi-label and unlabeled
//! vertices, and with filters whose children mutate the graph or write side effects.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

/// Adds to the modern graph: a vertex labeled `person` + `admin` (primary `person`), one
/// labeled `admin` + `person` (primary `admin`), one labeled `software` + `person`, an unlabeled
/// vertex, and edges from the multi-label vertices.
const SETUP: &str = r#"
g.addV(["person", "admin"]).property("name", "root").property("age", 50).to_list();
g.addV(["admin", "person"]).property("name", "ops").property("age", 40).to_list();
g.addV(["software", "person"]).property("name", "bot").to_list();
g.addV().property("name", "nolabel").to_list();
g.V().has("name", "root").addE("knows").to(__.V("1")).to_list();
g.V().has("name", "ops").addE("created").to(__.V("3")).to_list();
"#;

/// The corpus. `G` is replaced by the traversal source under test.
const CORPUS: &[&str] = &[
    // group_count_pushdown: `label()` is the primary label.
    r#"G.V().groupCount().by(T.label).next()"#,
    r#"G.V().group().by(T.label).by(__.count()).next()"#,
    r#"G.V().group().by(T.label).by(__.out().count()).next()"#,
    r#"G.V().hasLabel("person").groupCount().by(T.label).next()"#,
    r#"G.V().hasLabel("admin").groupCount().by(T.label).next()"#,
    r#"G.V().hasLabel("admin", "software").groupCount().by(T.label).next()"#,
    r#"G.V().hasId("1", "13").groupCount().by(T.label).next()"#,
    r#"G.E().groupCount().by(T.label).next()"#,
    r#"G.E().hasLabel("knows", "knows").groupCount().by(T.label).next()"#,
    // count_pushdown / source_filter_pushdown: has_label matches any label, each element once.
    r#"G.V().count().next()"#,
    r#"G.V().count().is(P.gt(3)).to_list()"#,
    r#"G.V().hasLabel("person").count().next()"#,
    r#"G.V().hasLabel("admin").count().next()"#,
    r#"G.V().hasLabel("person", "admin").count().next()"#,
    r#"G.V().hasLabel("person", "person").count().next()"#,
    r#"G.V().hasLabel("admin").hasLabel("person").count().next()"#,
    r#"G.V().hasLabel("admin").hasLabel("software").count().next()"#,
    r#"G.V().hasLabel("person", "admin").values("name").to_list()"#,
    r#"G.V().hasLabel(P.within("admin", "software")).values("name").to_list()"#,
    r#"G.E().hasLabel("knows", "knows").count().next()"#,
    r#"G.E().hasLabel("knows", "knows").id().to_list()"#,
    r#"G.E().hasLabel("knows", "created").count().next()"#,
    // has_id_pushdown: a filter's id list is a set, a source step's id list is a sequence.
    r#"G.V().hasId("1", "1").count().next()"#,
    r#"G.V().hasId("1", "1").values("name").to_list()"#,
    r#"G.V("1", "1").hasId("1").values("name").to_list()"#,
    r#"G.V("1", "2").hasId("1", "1").count().next()"#,
    r#"G.V().hasId("1").hasId("2").count().next()"#,
    r#"G.V().hasLabel("person").hasId("1", "2", "3").values("name").to_list()"#,
    r#"G.E().hasId("7", "7").count().next()"#,
    r#"G.V("1").V().hasLabel("admin").count().next()"#,
    r#"G.inject(1).V().hasLabel("person").count().next()"#,
    // source_filter_pushdown of has(key, value): checked while the source scans.
    r#"G.V().has("name", "marko").values("name").to_list()"#,
    r#"G.V().hasLabel("person").has("age", 29).values("name").to_list()"#,
    r#"G.V().hasLabel("person").has("age", 29).count().next()"#,
    r#"G.V().has("age", 29).has("name", "marko").count().next()"#,
    r#"G.V().has("age", 29).has("name", "josh").count().next()"#,
    r#"G.V().has("name", "josh").hasLabel("person").values("age").to_list()"#,
    r#"G.V().hasLabel("person").has("age", 29).groupCount().by(T.label).next()"#,
    r#"G.V().has("age", 29.0).count().next()"#,
    r#"G.V().has("nope", 1).count().next()"#,
    r#"G.V("1", "2", "3").has("age", 27).values("name").to_list()"#,
    r#"G.V().hasLabel("admin").has("age", 40).values("name").to_list()"#,
    r#"G.V().hasLabel("admin").hasLabel("person").has("age", 50).values("name").to_list()"#,
    r#"G.V().has("name", "nolabel").count().next()"#,
    r#"G.V().has("name", "marko").out().path().by("name").to_list()"#,
    r#"G.V().has(jpath("name"), "marko").count().next()"#,
    r#"G.E().has("weight", 0.4).count().next()"#,
    r#"G.E().hasLabel("created").has("weight", 0.4).id().to_list()"#,
    r#"G.E().has("weight", 0.4).groupCount().by(T.label).next()"#,
    r#"G.V("1").V().has("name", "josh").values("name").to_list()"#,
    r#"G.inject(1).V().has("lang", "java").count().next()"#,
    r#"G.V().hasLabel("person").local(__.V().has("name", "josh").count()).to_list()"#,
    r#"G.V().has("name", "marko").property("seen", true).V().has("seen", true).count().next()"#,
    // filter_reorder: effectful filters stay where the query put them.
    r#"G.V().filter(__.aggregate("x")).hasLabel("software").cap("x").unfold().count().next()"#,
    r#"G.V().where(__.aggregate("x")).hasId("1").cap("x").unfold().count().next()"#,
    r#"G.V().has("age", P.gt(20)).filter(__.store("y")).hasLabel("person").cap("y").unfold().values("name").to_list()"#,
    r#"G.V().filter(__.sideEffect(__.aggregate("z"))).hasId("2").cap("z").unfold().count().next()"#,
    r#"G.V().filter(__.out().aggregate("o")).hasLabel("person").cap("o").unfold().count().next()"#,
    r#"G.V().filter(__.property("seen", true)).hasId("1").count().next()"#,
    r#"G.V().filter(__.addV("tmp")).hasLabel("software").count().next()"#,
    r#"G.V().hasLabel("software").drop().hasId("3").to_list()"#,
    r#"G.V().where(__.out()).hasLabel("person").hasId("1", "4").values("name").to_list()"#,
    r#"G.V().has("age", P.gt(28)).hasLabel("person").hasId("1", "4", "6").values("name").to_list()"#,
    // where_unnest.
    r#"G.V().where(__.hasLabel("person").has("age", P.gt(30))).values("name").to_list()"#,
    r#"G.V().where(__.hasLabel("admin")).count().next()"#,
    r#"G.V().where(__.hasId("1", "1")).count().next()"#,
    // barrier_pushdown.
    r#"G.V().outE().hasLabel("knows").inV().values("name").to_list()"#,
    r#"G.V("1").outE().hasLabel("knows", "created").count().next()"#,
    r#"G.V().bothE().hasLabel("knows", "knows").count().next()"#,
    // lazy_barrier and bulk.
    r#"G.V().both().both().count().next()"#,
    r#"G.V().both().both().values("name").to_list()"#,
    r#"G.V().out().out().path().by("name").to_list()"#,
    r#"G.withSack(1).V().both().both().sack().sum().next()"#,
    // Nested source steps.
    r#"G.V().hasLabel("person").local(__.V().count()).to_list()"#,
    r#"G.V().hasLabel("person").local(__.V().hasLabel("admin").count()).to_list()"#,
    r#"G.V().hasLabel("software").union(__.V().count(), __.V().hasLabel("admin").count()).to_list()"#,
    r#"G.V().hasLabel("software").where(__.V().hasLabel("admin").count().is(2)).count().next()"#,
    r#"G.V().hasLabel("person").local(__.V().groupCount().by(T.label)).to_list()"#,
    // add_property_fold and mutating steps.
    r#"G.addV("x").property("a", 1).property("a", 2).values("a").to_list()"#,
    r#"G.V().has("name", "nobody").addV("ghost").to_list()"#,
    r#"G.V().has("name", "nobody").as("a").addE("x").from("a").to_list()"#,
    r#"G.V().has("name", "nobody").addE("x").to(__.V("2")).to_list()"#,
];

/// Every registered rule name, from the optimizer registry.
fn rule_names() -> Vec<String> {
    let names: Vec<String> = graphersal::exec::optimizer_rule_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert!(names.len() >= 9, "{names:?}");
    names
}

/// The traversal source spelling that disables `disabled`.
fn source(disabled: &[&String]) -> String {
    let names: Vec<String> = disabled.iter().map(|n| format!("\"{n}\"")).collect();
    format!("g.with(\"optimizer.disabled\", [{}])", names.join(", "))
}

/// Runs `query` with `G` replaced by `source` on a fresh graph and returns its result (top-level
/// list order ignored: no rule promises an order the query does not ask for) and the graph state.
fn run(query: &str, source: &str) -> String {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let script = format!(
        "{SETUP}\nlet r = {};\n[r, g.V().count().next(), g.E().count().next(), \
         g.V().properties().count().next(), g.E().properties().count().next()]",
        query.replace("G.", &format!("{source}."))
    );
    match eval_value(graph, &script) {
        Ok(value) => {
            let mut parts = value.into_array().expect("the harness returns a list");
            let result = parts.remove(0);
            let result = match result.clone().into_array() {
                Ok(items) => {
                    let mut items: Vec<String> = items.iter().map(|v| format!("{v:?}")).collect();
                    items.sort();
                    format!("{items:?}")
                }
                Err(_) => format!("{result:?}"),
            };
            format!("{result} | state {parts:?}")
        }
        // An error must stay an error; its text may name optimized-plan positions.
        Err(_) => "error".to_string(),
    }
}

#[test]
fn every_rule_keeps_every_corpus_result() {
    let rules = rule_names();
    let all: Vec<&String> = rules.iter().collect();
    let mut failures = Vec::new();
    for query in CORPUS {
        let oracle = run(query, &source(&all));
        let mut variants = vec![("all rules on".to_string(), source(&[]))];
        for rule in &rules {
            let others: Vec<&String> = rules.iter().filter(|r| *r != rule).collect();
            variants.push((format!("all but {rule}"), source(&[rule])));
            variants.push((format!("only {rule}"), source(&others)));
        }
        for (name, variant) in variants {
            let got = run(query, &variant);
            if got != oracle {
                failures.push(format!(
                    "{query}\n  {name}: {got}\n  all rules off: {oracle}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The oracle itself sees the multi-label graph: guards the corpus against a setup that silently
/// stopped creating the vertices the interesting cases need.
#[test]
fn the_corpus_graph_has_multi_label_vertices() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let script = format!("{SETUP}\ng.V().hasLabel(\"admin\").count().next()");
    let admins = eval_value(graph, &script).expect("setup runs");
    assert_eq!(admins.as_int().expect("a count"), 2);
}
