//! Property indexes (book page "Property Indexes"): declared, ordered indexes on `(element kind,
//! label, key)` that a `v()`/`e()` source with a `has()` on the key reads instead of every
//! element of its labels.
//!
//! The oracle is the scan: every query runs with and without the index
//! (`g.with("optimizer.disabled", ["property_index"])`) and the results must be the same
//! multiset. The maintained entries are compared with a fresh rebuild
//! (`TraversalGraph::check_property_indexes`) after every kind of mutation, rollback, replay,
//! load and store operation.
#![cfg(feature = "script")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use graphersal::catalog::{Definition, DefinitionKind, PropertyIndexDefinition};
use graphersal::changes::{ChangeSet, CommitHook, HookError};
use graphersal::prelude::*;
use graphersal::storage::{ElementKind, Graph, GraphStorage, JsonPath, PropertyIndexRange};

use crate::script_host::{ScriptLimits, eval_value_atomic, eval_value_with_limits};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// A small deterministic generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const UUID_A: &str = "123e4567-e89b-12d3-a456-426614174000";
const UUID_B: &str = "123e4567-e89b-12d3-a456-426614174001";

/// The stored values of the random graphs: every scalar kind, `int64`/`float64` twins, the
/// float edge cases, strings, uuids and the containers an index never holds.
fn stored_values() -> Vec<ElementProperty> {
    let mut object = PropertyMap::default();
    object.insert("x", 1.into());
    vec![
        ElementProperty::Null,
        true.into(),
        false.into(),
        (-2_i64).into(),
        0_i64.into(),
        1_i64.into(),
        2_i64.into(),
        3_i64.into(),
        i64::MAX.into(),
        ((1_i64 << 53) + 1).into(),
        1.0.into(),
        2.5.into(),
        (-0.0).into(),
        0.0.into(),
        f64::NAN.into(),
        9_007_199_254_740_992.0.into(),
        "".into(),
        "a".into(),
        "ab".into(),
        "b".into(),
        "B".into(),
        ElementProperty::uuid_from_string(UUID_A).unwrap(),
        ElementProperty::uuid_from_string(UUID_B).unwrap(),
        ElementProperty::Array(vec![1.into()]),
        ElementProperty::Object(Box::new(object)),
    ]
}

/// Rhai literals of query operands (every kind, the twins and the float edge cases).
const OPERANDS: [&str; 22] = [
    "()",
    "true",
    "false",
    "-2",
    "0",
    "1",
    "2",
    "3",
    "9223372036854775807",
    "9007199254740993",
    "1.0",
    "2.5",
    "-0.0",
    "0.0",
    "9007199254740992.0",
    "\"\"",
    "\"a\"",
    "\"ab\"",
    "\"b\"",
    "\"B\"",
    "UUID.fromString(\"123e4567-e89b-12d3-a456-426614174000\")",
    "[1]",
];

/// A random condition on `key` as DSL text: a plain `has(key, value)` or a predicate.
fn random_condition(rng: &mut Rng, key: &str) -> String {
    let a = *rng.pick(&OPERANDS);
    let b = *rng.pick(&OPERANDS);
    let c = *rng.pick(&OPERANDS);
    match rng.below(13) {
        0 | 1 => format!("has(\"{key}\", {a})"),
        2 => format!("has(\"{key}\", P.eq({a}))"),
        3 => format!("has(\"{key}\", P.within({a}, {b}, {c}))"),
        4 => format!("has(\"{key}\", P.gt({a}))"),
        5 => format!("has(\"{key}\", P.gte({a}))"),
        6 => format!("has(\"{key}\", P.lt({a}))"),
        7 => format!("has(\"{key}\", P.lte({a}))"),
        8 => format!("has(\"{key}\", P.between({a}, {b}))"),
        9 => format!("has(\"{key}\", P.inside({a}, {b}))"),
        10 => format!("has(\"{key}\", P.outside({a}, {b}))"),
        11 => format!("has(\"{key}\", P.neq({a}))"),
        _ => format!("has(\"{key}\", P.gt({a}).and(P.lt({b})))"),
    }
}

fn run(graph: &Arc<Graph>, script: &str) -> Result<Dynamic, String> {
    eval_value_with_limits(
        graph.clone(),
        script,
        HashMap::new(),
        &ScriptLimits::default(),
    )
    .map_err(|err| err.to_string())
}

use graphersal::script::rhai::Dynamic;

/// Runs a script that must succeed, its value ignored.
fn ok(graph: &Arc<Graph>, script: &str) {
    let _ = run(graph, script).unwrap();
}

/// The rendered values of a list result, sorted (the order of elements is unspecified).
fn sorted(value: Dynamic) -> Vec<String> {
    let mut items: Vec<String> = value
        .into_array()
        .expect("a list")
        .into_iter()
        .map(|item| item.to_string())
        .collect();
    items.sort();
    items
}

/// `query` (a traversal without terminal, starting after `g`) with and without the index: the
/// sorted ids, or the error texts.
fn both(
    graph: &Arc<Graph>,
    query: &str,
) -> (Result<Vec<String>, String>, Result<Vec<String>, String>) {
    let with = run(graph, &format!("g.{query}.id().to_list()")).map(sorted);
    let without = run(
        graph,
        &format!("g.with(\"optimizer.disabled\", [\"property_index\"]).{query}.id().to_list()"),
    )
    .map(sorted);
    (with, without)
}

/// The full step names of `query`'s profile (the table cuts long names).
fn profile_steps(graph: &Arc<Graph>, query: &str) -> Vec<String> {
    let metrics = run(graph, &format!("g.{query}.profile()"))
        .unwrap()
        .try_cast::<graphersal::profile::TraversalMetrics>()
        .expect("metrics");
    metrics
        .metrics()
        .iter()
        .map(|step| step.name().to_string())
        .collect()
}

/// Whether `query` reads an index (its profile shows `[index: ..]` on the source).
fn uses_index(graph: &Arc<Graph>, query: &str) -> bool {
    profile_steps(graph, query)
        .first()
        .is_some_and(|name| name.contains("[index:"))
}

fn check(graph: &Graph) {
    graph.read().check_property_indexes().unwrap();
}

fn vertex_index(name: &str, label: &str, key: &str) -> Definition {
    Definition::property_index(
        name,
        PropertyIndexDefinition::new(ElementKind::Vertex, label, key),
    )
}

fn edge_index(name: &str, label: &str, key: &str) -> Definition {
    Definition::property_index(
        name,
        PropertyIndexDefinition::new(ElementKind::Edge, label, key),
    )
}

/// A random graph: vertices labelled from `a`, `b`, `c` (some with two labels, some without),
/// a property `k` (sometimes missing) and `j`; edges `e1`/`e2` with `w`.
fn random_graph(rng: &mut Rng, vertices: usize) -> TraversalGraph {
    let values = stored_values();
    let mut graph = TraversalGraph::new();
    let label_sets: [&[&str]; 6] = [&["a"], &["b"], &["c"], &["a", "b"], &["b", "a"], &[]];
    let mut handles = Vec::new();
    for _ in 0..vertices {
        let labels: Vec<CompactString> = rng
            .pick(&label_sets)
            .iter()
            .map(|l| CompactString::from(*l))
            .collect();
        let mut props = PropertyMap::default();
        if rng.below(6) != 0 {
            props.insert("k", rng.pick(&values).clone());
        }
        props.insert("j", (rng.below(3) as i64).into());
        handles.push(graph.add_vertex(None, labels, props).unwrap());
    }
    for _ in 0..vertices {
        let from = *rng.pick(&handles);
        let to = *rng.pick(&handles);
        let mut props = PropertyMap::default();
        if rng.below(5) != 0 {
            props.insert("w", rng.pick(&values).clone());
        }
        let label = if rng.below(2) == 0 { "e1" } else { "e2" };
        graph
            .add_edge(None, Some(label.into()), from, to, props)
            .unwrap();
    }
    graph
}

fn declare_indexes(graph: &Graph) {
    let mut g = graph.write();
    for definition in [
        vertex_index("a_k", "a", "k"),
        vertex_index("b_k", "b", "k"),
        edge_index("e1_w", "e1", "w"),
    ] {
        g.set_definition(definition).unwrap();
    }
}

/// Random queries with and without the index.
fn differential_queries(graph: &Arc<Graph>, rng: &mut Rng, queries: usize) -> usize {
    let mut served = 0;
    for _ in 0..queries {
        let query = match rng.below(5) {
            0 => format!("v().has_label(\"a\").{}", random_condition(rng, "k")),
            1 => format!("v().has_label(\"a\", \"b\").{}", random_condition(rng, "k")),
            2 => format!(
                "v().has_label(\"b\").{}.{}",
                random_condition(rng, "k"),
                random_condition(rng, "j")
            ),
            3 => format!("e().has_label(\"e1\").{}", random_condition(rng, "w")),
            _ => format!("v().has_label(\"a\", \"c\").{}", random_condition(rng, "k")),
        };
        let (with, without) = both(graph, &query);
        assert_eq!(with, without, "{query}");
        if uses_index(graph, &query) {
            served += 1;
        }
    }
    served
}

// ---------------------------------------------------------------------------------------------
// The differential
// ---------------------------------------------------------------------------------------------

#[test]
fn random_has_queries_return_the_same_with_and_without_the_index() {
    for seed in [1_u64, 7, 23] {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let graph = Arc::new(Graph::new(random_graph(&mut rng, 120)));
        declare_indexes(&graph);
        check(&graph);
        let served = differential_queries(&graph, &mut rng, 300);
        assert!(
            served > 100,
            "the index served only {served} of 300 queries"
        );
    }
}

#[test]
fn random_mutations_keep_the_index_exact_and_the_queries_equal() {
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    let graph = Arc::new(Graph::new(random_graph(&mut rng, 60)));
    declare_indexes(&graph);
    let values: Vec<&str> = OPERANDS.iter().copied().filter(|v| *v != "()").collect();
    for round in 0..120 {
        let id = rng.below(70);
        let value = *rng.pick(&values);
        let mutation = match rng.below(12) {
            0 => format!("g.v(\"{id}\").property(\"k\", {value}).iterate()"),
            1 => format!("g.v(\"{id}\").remove_property(\"k\").iterate()"),
            2 => format!("g.v(\"{id}\").add_label(\"a\").iterate()"),
            3 => format!("g.v(\"{id}\").drop_label(\"a\").iterate()"),
            4 => format!("g.v(\"{id}\").add_label(\"b\").iterate()"),
            5 => format!("g.v(\"{id}\").drop().iterate()"),
            6 => format!("g.add_v(\"a\").property(\"k\", {value}).iterate()"),
            7 => format!("g.e(\"{id}\").property(\"w\", {value}).iterate()"),
            8 => format!("g.e(\"{id}\").set_label(\"e1\").iterate()"),
            9 => format!("g.e(\"{id}\").set_label(\"e2\").iterate()"),
            10 => format!("g.e(\"{id}\").drop().iterate()"),
            _ => format!("g.v(\"{id}\").property(\"k\", #{{x: {value}}}).iterate()"),
        };
        // Every third mutation is rolled back: the script fails after it.
        if round % 3 == 0 {
            let failing = format!("{mutation}; throw \"undo\";");
            assert!(
                eval_value_atomic(
                    graph.clone(),
                    &failing,
                    HashMap::new(),
                    &ScriptLimits::default()
                )
                .is_err()
            );
        } else {
            // Some mutations fail on their own (a missing element): that is fine, the unit
            // rolls back.
            let _ = run(&graph, &mutation);
        }
        check(&graph);
        differential_queries(&graph, &mut rng, 5);
    }
}

// ---------------------------------------------------------------------------------------------
// Maintenance after every kind of mutation
// ---------------------------------------------------------------------------------------------

fn props(entries: &[(&str, ElementProperty)]) -> PropertyMap {
    let mut map = PropertyMap::default();
    for (key, value) in entries {
        map.insert(*key, value.clone());
    }
    map
}

/// Ids of the vertices labelled `label` whose `key` lies in `range`, from the index.
fn looked_up(
    graph: &TraversalGraph,
    label: &str,
    key: &str,
    range: &PropertyIndexRange,
) -> Vec<String> {
    let mut ids: Vec<String> = graph
        .vertices_by_property_index(label, key, range)
        .expect("an index")
        .filter_map(|v| graph.vertex_id(v).map(str::to_string))
        .collect();
    ids.sort();
    ids
}

#[test]
fn every_mutation_kind_and_its_rollback_keep_the_index_exact() {
    let mut graph = TraversalGraph::new();
    graph
        .set_definition(vertex_index("p_title", "p", "title"))
        .unwrap();
    graph
        .set_definition(vertex_index("q_title", "q", "title"))
        .unwrap();
    graph
        .set_definition(edge_index("r_since", "r", "since"))
        .unwrap();
    let check = |graph: &TraversalGraph| graph.check_property_indexes().unwrap();
    let ceo = PropertyIndexRange::equal("CEO".into());

    // Every mutation runs twice: once rolled back (the index must be as before), once kept.
    macro_rules! both_ways {
        ($graph:ident, $body:expr) => {{
            let mark = $graph.begin_unit();
            let _ = (|$graph: &mut TraversalGraph| $body)(&mut $graph);
            check(&$graph);
            $graph.rollback_unit(mark, &graphersal::changes::RollbackReason::failed(&"test"));
            check(&$graph);
            let result = (|$graph: &mut TraversalGraph| $body)(&mut $graph);
            check(&$graph);
            result
        }};
    }

    let v = both_ways!(
        graph,
        graph
            .add_vertex(
                Some("v".into()),
                vec!["p".into()],
                props(&[("title", "CEO".into())])
            )
            .unwrap_or_else(|_| graph.get_vertex_by_id("v").unwrap())
    );
    assert_eq!(looked_up(&graph, "p", "title", &ceo), ["v"]);
    let w = graph
        .add_vertex(
            Some("w".into()),
            vec!["q".into()],
            props(&[("title", "CTO".into())]),
        )
        .unwrap();
    check(&graph);

    both_ways!(
        graph,
        graph.set_vertex_property(v, "title".into(), "CTO".into())
    )
    .unwrap();
    assert!(looked_up(&graph, "p", "title", &ceo).is_empty());
    both_ways!(
        graph,
        graph.set_vertex_property(v, "title".into(), "CEO".into())
    )
    .unwrap();
    // A label added: the vertex joins the other label's index; removed: it leaves it.
    both_ways!(graph, graph.add_vertex_label(v, "q".into())).unwrap();
    assert_eq!(looked_up(&graph, "q", "title", &ceo), ["v"]);
    both_ways!(graph, graph.remove_vertex_label(v, "p")).unwrap();
    assert!(looked_up(&graph, "p", "title", &ceo).is_empty());
    both_ways!(graph, graph.add_vertex_label(v, "p".into())).unwrap();
    // A jpath write and removal through the top-level key.
    let nested: JsonPath = "title.short".parse().unwrap();
    let whole: JsonPath = "title".parse().unwrap();
    both_ways!(
        graph,
        graph.set_vertex_property(v, "title".into(), ElementProperty::Object(Box::default()))
    )
    .unwrap();
    both_ways!(
        graph,
        graph.set_vertex_property_path(v, &nested, "C".into())
    )
    .unwrap();
    both_ways!(graph, graph.remove_vertex_property_path(v, &nested)).unwrap();
    both_ways!(
        graph,
        graph.set_vertex_property_path(v, &whole, "CEO".into())
    )
    .unwrap();
    assert_eq!(looked_up(&graph, "p", "title", &ceo), ["v"]);
    both_ways!(graph, graph.drop_vertex_property("title", v)).unwrap();
    assert!(looked_up(&graph, "p", "title", &ceo).is_empty());
    graph
        .set_vertex_property(v, "title".into(), "CEO".into())
        .unwrap();

    // Edges: add, property, relabel, drop (and a vertex drop cascading to its edges).
    let e = both_ways!(
        graph,
        graph
            .add_edge(
                Some("e".into()),
                Some("r".into()),
                v,
                w,
                props(&[("since", 2020.into())])
            )
            .unwrap_or_else(|_| graph.get_edge_by_id("e").unwrap())
    );
    let since = PropertyIndexRange::equal(2020.into());
    let edges = |graph: &TraversalGraph| {
        graph
            .edges_by_property_index("r", "since", &since)
            .unwrap()
            .count()
    };
    assert_eq!(edges(&graph), 1);
    both_ways!(
        graph,
        graph.set_edge_property(e, "since".into(), 2021.into())
    )
    .unwrap();
    assert_eq!(edges(&graph), 0);
    both_ways!(
        graph,
        graph.set_edge_property(e, "since".into(), 2020.into())
    )
    .unwrap();
    both_ways!(graph, graph.set_edge_label(e, "s".into())).unwrap();
    assert_eq!(edges(&graph), 0);
    both_ways!(graph, graph.set_edge_label(e, "r".into())).unwrap();
    assert_eq!(edges(&graph), 1);
    both_ways!(graph, graph.drop_edge_property("since", e)).unwrap();
    graph
        .set_edge_property(e, "since".into(), 2020.into())
        .unwrap();
    // A drop and its rollback: the element comes back under a NEW handle.
    let mark = graph.begin_unit();
    graph.drop_vertex(w).unwrap();
    check(&graph);
    assert_eq!(edges(&graph), 0);
    graph.rollback_unit(mark, &graphersal::changes::RollbackReason::failed(&"test"));
    check(&graph);
    assert_eq!(edges(&graph), 1);
    let w = graph.get_vertex_by_id("w").unwrap();
    graph.drop_vertex(w).unwrap();
    check(&graph);
    assert_eq!(edges(&graph), 0);
    let mark = graph.begin_unit();
    graph.drop_vertex(v).unwrap();
    check(&graph);
    graph.rollback_unit(mark, &graphersal::changes::RollbackReason::failed(&"test"));
    check(&graph);
    assert_eq!(looked_up(&graph, "p", "title", &ceo), ["v"]);
    let v = graph.get_vertex_by_id("v").unwrap();
    graph.drop_vertex(v).unwrap();
    check(&graph);
    assert!(looked_up(&graph, "p", "title", &ceo).is_empty());

    // Dropping and redefining an index (and undoing that) rebuild it.
    let x = graph
        .add_vertex(None, vec!["p".into()], props(&[("title", "CEO".into())]))
        .unwrap();
    let _ = x;
    let mark = graph.begin_unit();
    graph
        .remove_definition(DefinitionKind::PROPERTY_INDEX, "p_title")
        .unwrap();
    check(&graph);
    assert!(
        graph
            .vertices_by_property_index("p", "title", &ceo)
            .is_none()
    );
    graph.rollback_unit(mark, &graphersal::changes::RollbackReason::failed(&"test"));
    check(&graph);
    assert_eq!(looked_up(&graph, "p", "title", &ceo).len(), 1);
}

#[test]
fn a_second_index_on_the_same_key_is_a_conflict_with_help() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(&graph, r#"g.create_index(#{label: "person", key: "age"})"#);
    let err = run(
        &graph,
        r#"g.create_index(#{name: "again", label: "person", key: "age"})"#,
    )
    .unwrap_err();
    assert!(err.contains("Conflicting property index 'again'"), "{err}");
    assert!(err.contains("g.drop_index(\"person_age\")"), "{err}");
    // Redefining under the same name is fine.
    ok(&graph, r#"g.create_index(#{label: "person", key: "age"})"#);
    let err = run(&graph, r#"g.create_index(#{label: "person", key: ""})"#).unwrap_err();
    assert!(err.contains("top-level property"), "{err}");
}

#[test]
fn compressed_strings_are_indexed_by_their_text() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    let long = "Senior engineer, led the platform team. ".repeat(20);
    ok(
        &graph,
        r#"g.define_compression(#{name: "bio", label: "person", path: "bio", min_bytes: 16})"#,
    );
    ok(&graph, r#"g.create_index(#{label: "person", key: "bio"})"#);
    ok(
        &graph,
        &format!(r#"g.v("1").property("bio", "{long}").iterate()"#),
    );
    check(&graph);
    assert!(graph.read().memory_usage().unwrap().compressed_values > 0);
    let query = format!(r#"v().has_label("person").has("bio", "{long}")"#);
    assert!(uses_index(&graph, &query));
    let (with, without) = both(&graph, &query);
    assert_eq!(with, Ok(vec!["1".to_string()]));
    assert_eq!(with, without);
    // Dropping the rule decompresses: the entries are unchanged.
    ok(&graph, r#"g.drop_compression("bio")"#);
    check(&graph);
    assert_eq!(both(&graph, &query).0, Ok(vec!["1".to_string()]));
}

// ---------------------------------------------------------------------------------------------
// Replay, change sets, catalog file
// ---------------------------------------------------------------------------------------------

struct Collect(Arc<Mutex<Vec<ChangeSet>>>);

impl CommitHook for Collect {
    fn before_commit(&mut self, changes: &ChangeSet) -> Result<(), HookError> {
        self.0.lock().map_err(|_| "poisoned")?.push(changes.clone());
        Ok(())
    }
}

#[test]
fn replay_and_apply_change_set_rebuild_the_same_index() {
    let journal = Arc::new(Mutex::new(Vec::new()));
    let mut live = TraversalGraph::new();
    live.register_commit_hook(Box::new(Collect(journal.clone())))
        .unwrap();
    let live = Arc::new(Graph::new(live));
    ok(&live, r#"g.add_v("p").property("n", 1).iterate()"#);
    ok(&live, r#"g.create_index(#{label: "p", key: "n"})"#);
    ok(
        &live,
        r#"g.add_v("p").property("n", 2).add_v("p").property("n", 2).iterate()"#,
    );
    ok(&live, r#"g.v().has("n", 1).property("n", 3).iterate()"#);
    ok(&live, r#"g.v().has("n", 2).limit(1).drop().iterate()"#);
    check(&live);

    let mut replayed = TraversalGraph::new();
    let mut applied = TraversalGraph::new();
    for changes in journal.lock().unwrap().iter() {
        replayed = replayed.replay(changes).unwrap();
        applied.apply_change_set(changes).unwrap();
    }
    for graph in [replayed, applied] {
        graph.check_property_indexes().unwrap();
        let infos = graph.property_indexes();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].stats.entries, 2);
        assert_eq!(infos[0].stats.distinct_values, 2);
        let graph = Arc::new(Graph::new(graph));
        let query = r#"v().has_label("p").has("n", P.gte(2))"#;
        assert!(uses_index(&graph, query));
        assert_eq!(both(&graph, query).0.unwrap().len(), 2);
    }
}

#[test]
fn index_definitions_round_trip_through_the_catalog_json() {
    let definition = vertex_index("person_title", "person", "title");
    let text = definition.to_json();
    assert!(text.contains("\"kind\":\"property_index\""), "{text}");
    assert_eq!(Definition::from_json(&text).unwrap(), definition);
    let catalog =
        graphersal::catalog::catalog_to_json([&definition, &edge_index("e", "knows", "w")]);
    let back = graphersal::catalog::catalog_from_json(&catalog).unwrap();
    assert_eq!(back[0], definition);
    assert_eq!(
        back[1].as_property_index().unwrap().element(),
        ElementKind::Edge
    );
    assert!(DefinitionKind::PROPERTY_INDEX.is_known());
}

// ---------------------------------------------------------------------------------------------
// Optimizer, profile, multi-label, edges
// ---------------------------------------------------------------------------------------------

#[test]
fn the_profile_shows_the_access_path_and_the_rule_can_be_turned_off() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(&graph, r#"g.v("1").property("title", "CEO").iterate()"#);
    ok(
        &graph,
        r#"g.create_index(#{label: "person", key: "title"})"#,
    );
    let lock = graph.read();
    let metrics = lock
        .traversal()
        .v(None)
        .has_label("person")
        .has("title", "CEO")
        .profile()
        .unwrap();
    assert_eq!(
        metrics.metrics()[0].name(),
        r#"v(labels: ["person"]).has("title", "CEO") [index: vertex person.title]"#
    );
    let profile = metrics.to_string();
    assert!(
        profile.contains("Optimizer rules applied: source_filter_pushdown, property_index"),
        "{profile}"
    );
    let off = lock
        .traversal()
        .with("optimizer.disabled", vec!["property_index"])
        .v(None)
        .has_label("person")
        .has("title", "CEO")
        .profile()
        .unwrap()
        .to_string();
    assert!(!off.contains("[index:"), "{off}");
    assert!(!off.contains("property_index"), "{off}");
    // Without a has_label() (or on another label, or another key) no index is used.
    for query in [
        lock.traversal().v(None).has("title", "CEO").profile(),
        lock.traversal()
            .v(None)
            .has_label("software")
            .has("title", "CEO")
            .profile(),
        lock.traversal()
            .v(None)
            .has_label("person")
            .has("name", "marko")
            .profile(),
    ] {
        assert!(!query.unwrap().to_string().contains("[index:"));
    }
}

#[test]
fn the_most_selective_index_is_picked() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(
        &graph,
        r#"g.v().has_label("person").property("team", "all").iterate()"#,
    );
    ok(&graph, r#"g.create_index(#{label: "person", key: "team"})"#);
    ok(&graph, r#"g.create_index(#{label: "person", key: "name"})"#);
    let steps = profile_steps(
        &graph,
        r#"v().has_label("person").has("team", "all").has("name", "josh")"#,
    );
    assert!(
        steps[0].ends_with("[index: vertex person.name]"),
        "{steps:?}"
    );
}

#[test]
fn a_multi_label_vertex_is_in_the_index_of_every_label_and_found_once() {
    let mut graph = TraversalGraph::new();
    graph.set_definition(vertex_index("a_k", "a", "k")).unwrap();
    graph.set_definition(vertex_index("b_k", "b", "k")).unwrap();
    graph
        .add_vertex(
            Some("ab".into()),
            vec!["a".into(), "b".into()],
            props(&[("k", 1.into())]),
        )
        .unwrap();
    graph
        .add_vertex(
            Some("b".into()),
            vec!["b".into()],
            props(&[("k", 1.into())]),
        )
        .unwrap();
    let one = PropertyIndexRange::equal(1.into());
    assert_eq!(looked_up(&graph, "a", "k", &one), ["ab"]);
    assert_eq!(looked_up(&graph, "b", "k", &one), ["ab", "b"]);
    let graph = Arc::new(Graph::new(graph));
    let query = r#"v().has_label("a", "b").has("k", 1)"#;
    assert!(uses_index(&graph, query));
    assert_eq!(
        both(&graph, query).0,
        Ok(vec!["ab".to_string(), "b".to_string()])
    );
    // One label without an index: the scan.
    assert!(!uses_index(
        &graph,
        r#"v().has_label("a", "c").has("k", 1)"#
    ));
}

#[test]
fn edge_indexes_serve_e_with_ranges() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(
        &graph,
        r#"g.create_index(#{element: "edge", label: "knows", key: "weight"})"#,
    );
    let indexes = run(&graph, "g.indexes()").unwrap().to_string();
    assert!(
        indexes.contains("\"name\": \"knows_weight_edge\""),
        "{indexes}"
    );
    let query = r#"e().has_label("knows").has("weight", P.gt(0.6))"#;
    let steps = profile_steps(&graph, query);
    assert!(
        steps[0].ends_with("[index: edge knows.weight]"),
        "{steps:?}"
    );
    let (with, without) = both(&graph, query);
    assert_eq!(with.as_ref().map(Vec::len), Ok(1));
    assert_eq!(with, without);
    // camelCase spellings.
    ok(&graph, r#"g.createIndex(#{label: "person", key: "age"})"#);
    assert_eq!(
        run(&graph, r#"g.dropIndex("person_age")"#)
            .unwrap()
            .as_bool(),
        Ok(true)
    );
    assert_eq!(
        run(&graph, r#"g.drop_index("person_age")"#)
            .unwrap()
            .as_bool(),
        Ok(false)
    );
}

#[test]
fn a_string_eq_predicate_in_a_child_traversal_is_not_served() {
    // `P.eq("a")` first names a path label: inside a child the traversers carry one.
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(&graph, r#"g.create_index(#{label: "person", key: "name"})"#);
    let script = r#"g.v("1").as("josh").map(__.v().has_label("person").has("name", P.eq("josh")).count()).to_list()"#;
    let with = run(&graph, script).unwrap().to_string();
    let without = run(
        &graph,
        &script.replacen(
            "g.",
            r#"g.with("optimizer.disabled", ["property_index"])."#,
            1,
        ),
    )
    .unwrap()
    .to_string();
    assert_eq!(with, without);
    // At the root it is served.
    assert!(uses_index(
        &graph,
        r#"v().has_label("person").has("name", P.eq("josh"))"#
    ));
}

#[test]
fn memory_usage_accounts_the_index() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    assert_eq!(graph.read().memory_usage().unwrap().property_index_bytes, 0);
    ok(&graph, r#"g.create_index(#{label: "person", key: "name"})"#);
    let stats = graph.read().memory_usage().unwrap();
    assert!(stats.property_index_bytes > 0);
    assert_eq!(
        stats.total_bytes(),
        stats.structure_bytes
            + stats.index_bytes
            + stats.property_bytes
            + stats.compression_dictionary_bytes
            + stats.property_index_bytes
    );
    let listed = run(&graph, "g.indexes()[0].bytes")
        .unwrap()
        .as_int()
        .unwrap();
    assert_eq!(listed as u64, stats.property_index_bytes);
    assert_eq!(
        run(&graph, "g.memory_usage().property_index_bytes")
            .unwrap()
            .as_int()
            .unwrap() as u64,
        stats.property_index_bytes
    );
}

#[test]
fn the_rust_api_creates_lists_and_drops() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    graph
        .traversal_mut()
        .create_index(
            "ages",
            PropertyIndexDefinition::new(ElementKind::Vertex, "person", "age"),
        )
        .unwrap();
    let listed = graph.traversal().list_indexes().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0.name(), "ages");
    assert_eq!(listed[0].1.unwrap().entries, 4);
    assert!(graph.traversal_mut().drop_index("ages").unwrap());
    assert!(graph.traversal().list_indexes().unwrap().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------------------------

#[test]
fn a_read_only_host_lists_indexes_but_cannot_create_or_drop_them() {
    use graphersal::auth::AccessPolicy;
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(&graph, r#"g.create_index(#{label: "person", key: "age"})"#);
    let checked = |script: &str| {
        graphersal::script::eval_value_with_limits(
            graph.clone(),
            script,
            HashMap::new(),
            &ScriptLimits::default(),
            Arc::new(AccessPolicy::read_only()),
        )
        .map_err(|err| err.to_string())
    };
    assert!(checked("g.indexes()").is_ok());
    assert!(checked(r#"g.v().has_label("person").has("age", P.gt(30)).count().next()"#).is_ok());
    let denied = checked(r#"g.create_index(#{label: "person", key: "name"})"#).unwrap_err();
    assert!(
        denied.contains(r#"Permission denied for 'create_index' (Create Definition(property_index "person_name")"#),
        "{denied}"
    );
    let denied = checked(r#"g.create_index(#{label: "person", key: "age"})"#).unwrap_err();
    assert!(
        denied.contains(r#"(Update Definition(property_index "person_age")"#),
        "{denied}"
    );
    let denied = checked(r#"g.drop_index("person_age")"#).unwrap_err();
    assert!(
        denied.contains(
            r#"Permission denied for 'drop_index' (Delete Definition(property_index "person_age")"#
        ),
        "{denied}"
    );
    let reading = AccessPolicy::allow_all().deny(
        graphersal::auth::ActionKind::Read,
        graphersal::auth::ResourceKind::Definition,
    );
    let denied = graphersal::script::eval_value_with_limits(
        graph.clone(),
        "g.indexes()",
        HashMap::new(),
        &ScriptLimits::default(),
        Arc::new(reading),
    )
    .unwrap_err()
    .to_string();
    assert!(
        denied.contains("Permission denied for 'indexes'"),
        "{denied}"
    );
}

// ---------------------------------------------------------------------------------------------
// A foreign storage
// ---------------------------------------------------------------------------------------------

#[test]
fn the_forwarding_storage_serves_the_same_index() {
    use crate::forwarding::Forwarding;
    let forwarded = Arc::new(Graph::new(Forwarding::new(
        TraversalGraph::tinkerpop_modern(),
    )));
    let reference = Arc::new(GraphSource::tinkerpop_modern());
    let on_forwarded = |script: &str| {
        eval_value_with_limits(
            forwarded.clone(),
            script,
            HashMap::new(),
            &ScriptLimits::default(),
        )
        .map(|value| value.to_string())
        .map_err(|err| err.to_string())
    };
    let create = r#"g.create_index(#{label: "person", key: "age"})"#;
    on_forwarded(create).unwrap();
    ok(&reference, create);
    let query = r#"g.v().has_label("person").has("age", P.between(29, 35)).values("name").order().to_list()"#;
    assert_eq!(
        on_forwarded(query),
        run(&reference, query).map(|v| v.to_string())
    );
    let profile = on_forwarded(
        r#"g.v().has_label("person").has("age", P.between(29, 35)).profile().to_string()"#,
    )
    .unwrap();
    assert!(profile.contains("[index: vertex person.age]"), "{profile}");
    assert_eq!(
        on_forwarded("g.indexes()"),
        run(&reference, "g.indexes()").map(|v| v.to_string())
    );
}

// ---------------------------------------------------------------------------------------------
// Stores: the definition travels, the entries are rebuilt at every load
// ---------------------------------------------------------------------------------------------

#[cfg(all(feature = "persist", not(target_arch = "wasm32")))]
mod stores {
    use super::*;
    use graphersal::persist::{
        MemDir, RecoveryTarget, RollbackMode, Store, StoreDir, StoreOptions,
    };

    fn options() -> StoreOptions {
        StoreOptions::new()
            .with_wal_segment_bytes(600)
            .with_checkpoint_after_bytes(0)
    }

    /// The index of `graph` serves `person.age` with these stats and equals a rebuild.
    fn served(graph: &TraversalGraph, entries: u64) {
        graph.check_property_indexes().unwrap();
        let infos = graph.property_indexes();
        assert_eq!(infos.len(), 1, "{infos:?}");
        assert_eq!(infos[0].stats.entries, entries);
        let found = graph
            .vertices_by_property_index("person", "age", &PropertyIndexRange::equal(29.into()))
            .unwrap()
            .count();
        assert_eq!(found, 1);
    }

    #[test]
    fn the_index_is_rebuilt_on_open_read_only_open_backup_fork_and_rollback() {
        let main: Arc<dyn StoreDir> = Arc::new(MemDir::named("pindex-main"));
        let store = Store::create(&main, options()).unwrap();
        let graph = store.graph().clone();
        run_on(&graph, r#"g.add_v("person").property("age", 29).iterate()"#);
        let before_index = graph.read().last_commit_seq();
        run_on(&graph, r#"g.create_index(#{label: "person", key: "age"})"#);
        run_on(&graph, r#"g.add_v("person").property("age", 30).iterate()"#);
        served(&graph.read(), 2);
        store.close().unwrap();

        // Reopen: the catalog from the WAL, the entries from the data.
        let store = Store::open(&main, options()).unwrap();
        served(&store.graph().read(), 2);
        // A checkpoint stores the definition (never the entries); reopen from the manifest.
        store.checkpoint(None).unwrap();
        run_on(
            store.graph(),
            r#"g.add_v("person").property("age", 31).iterate()"#,
        );
        store.close().unwrap();
        let store = Store::open(&main, options()).unwrap();
        assert_eq!(store.open_report().commits_replayed, 1);
        served(&store.graph().read(), 3);

        // A read-only view of the latest state and of a past one.
        let latest = Store::open_read_only(&main, RecoveryTarget::Latest).unwrap();
        served(&latest.graph, 3);
        let past = Store::open_read_only(&main, RecoveryTarget::CommitSeq(before_index)).unwrap();
        assert!(past.graph.property_indexes().is_empty());
        past.graph.check_property_indexes().unwrap();

        // A backup opens with the index; a fork carries it.
        let backup: Arc<dyn StoreDir> = Arc::new(MemDir::named("pindex-backup"));
        store.backup(&backup).unwrap();
        let opened = Store::open_backup(&backup, options()).unwrap();
        served(&opened.graph().read(), 3);
        drop(opened);
        let fork: Arc<dyn StoreDir> = Arc::new(MemDir::named("pindex-fork"));
        store.fork(RecoveryTarget::Latest, &fork).unwrap();
        let forked = Store::open(&fork, options()).unwrap();
        served(&forked.graph().read(), 3);
        forked.close().unwrap();

        // A rollback to before the index removes it (the same graph instance is reloaded).
        store
            .rollback_to(RecoveryTarget::CommitSeq(before_index), RollbackMode::Move)
            .unwrap();
        let graph = store.graph().read();
        assert!(graph.property_indexes().is_empty());
        graph.check_property_indexes().unwrap();
        drop(graph);
        store.close().unwrap();
    }

    fn run_on(graph: &Arc<Graph>, script: &str) {
        ok(graph, script);
    }

    #[test]
    fn a_packed_snapshot_loads_with_the_index() {
        let graph = Arc::new(GraphSource::tinkerpop_modern());
        ok(&graph, r#"g.create_index(#{label: "person", key: "age"})"#);
        let mut bytes = Vec::new();
        graphersal::persist::write_snapshot(
            &*graph.read(),
            &mut bytes,
            &graphersal::persist::WriteOptions::default(),
        )
        .unwrap();
        let loaded = graphersal::persist::read_snapshot(
            bytes.as_slice(),
            &graphersal::persist::ReadOptions::default(),
        )
        .unwrap();
        served(&loaded, 4);
    }
}

/// Dates and times are not index keys (integration of #3 and #4): an indexed key holding dates
/// keeps the index exact, a condition on a date is served by a scan, and the results are the
/// same with and without the index.
#[test]
fn date_values_are_not_index_keys_and_conditions_on_them_scan() {
    let graph = Arc::new(GraphSource::tinkerpop_modern());
    ok(&graph, r#"g.create_index(#{label: "person", key: "born"})"#);
    ok(
        &graph,
        r#"g.v().has("name", "marko").property("born", date("1990-01-01")).iterate();
           g.v().has("name", "josh").property("born", date("1985-06-30")).iterate();
           g.v().has("name", "vadas").property("born", 1990).iterate()"#,
    );
    check(&graph);
    for query in [
        r#"v().has_label("person").has("born", P.gt(date("1988-01-01")))"#,
        r#"v().has_label("person").has("born", date("1985-06-30"))"#,
        r#"v().has_label("person").has("born", P.lt(datetime("2000-01-01T00:00Z")))"#,
    ] {
        assert!(!uses_index(&graph, query), "{query}");
        let (with, without) = both(&graph, query);
        assert_eq!(with, without, "{query}");
    }
    let query = r#"v().has_label("person").has("born", 1990)"#;
    assert!(uses_index(&graph, query));
    let (with, without) = both(&graph, query);
    assert_eq!(with, without);
    assert_eq!(with.unwrap().len(), 1);
    ok(
        &graph,
        r#"g.v().has("name", "marko").property("born", date("1991-01-01")).iterate();
           g.v().has("name", "josh").drop().iterate()"#,
    );
    check(&graph);
}
