//! The DSL differential on a foreign storage (`roadmap/generic-front-ends.md`, task E05a): the
//! same scripts run on `Arc<Graph<TraversalGraph>>` and on `Arc<Graph<Forwarding>>`, both built
//! from the modern graph, and their rendered results (and the graph state afterwards) must be
//! equal. `Forwarding` forwards every `GraphStorage` method to a `TraversalGraph` but has its
//! own handle types, so any difference is a bug in the generic front-end path.
//!
//! Covered: every documented DSL example (`script/register/docs.rs`, through the hidden
//! `graphersal::script::doc_examples`) and a curated list of mutating, whole-script
//! (`eval_value_atomic`: commit and rollback), dry-run (`eval_value_dry_run`: the compacted
//! `ChangeSet`), saved-query, schema, catalog, `g.mark` and GraphSON import/export scripts.
#![cfg(all(feature = "script", feature = "io"))]

use std::collections::HashMap;
use std::sync::Arc;

use graphersal::TraversalGraph;
use graphersal::changes::ChangeSet;
use graphersal::storage::{Graph, GraphStorage, IntoAnyGraph};

use crate::forwarding::Forwarding;
use crate::script_host::{
    ScriptLimits, eval_value_atomic, eval_value_dry_run, eval_value_with_limits,
};

/// The reference: the modern graph as a `TraversalGraph`.
fn reference() -> Arc<Graph<TraversalGraph>> {
    Arc::new(Graph::new(TraversalGraph::tinkerpop_modern()))
}

/// The modern graph behind the forwarding storage.
fn forwarded() -> Arc<Graph<Forwarding>> {
    Arc::new(Graph::new(Forwarding::new(
        TraversalGraph::tinkerpop_modern(),
    )))
}

/// `(script).to_string()` on `graph`, or the error text.
fn render(graph: impl IntoAnyGraph, script: &str) -> Result<String, String> {
    eval_value_with_limits(
        graph,
        &format!("({{ {script} }}).to_string()"),
        HashMap::new(),
        &ScriptLimits::default(),
    )
    .map(|value| value.to_string())
    .map_err(|err| err.to_string())
}

#[test]
fn every_documented_example_gives_the_same_result_on_both_storages() {
    let mut differences = Vec::new();
    let mut run = 0;
    for (receiver, name, example, result) in graphersal::script::doc_examples() {
        // `None`: the example needs a file or a host setup (the curated list covers those).
        if result.is_none() {
            continue;
        }
        run += 1;
        let expected = render(reference(), example);
        let actual = render(forwarded(), example);
        if expected != actual {
            differences.push(format!(
                "{receiver}.{name}: `{example}`\n  TraversalGraph: {expected:?}\n  Forwarding:     {actual:?}"
            ));
        }
    }
    assert!(run > 200, "only {run} examples ran");
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

/// How a curated script runs.
#[derive(Clone, Copy, Debug)]
enum Mode {
    /// `eval_value_with_limits`: every traversal is its own unit.
    Plain,
    /// `eval_value_atomic`: the whole script is one unit (commit iff it succeeds).
    Atomic,
    /// `eval_value_dry_run`: the whole script, always rolled back, with its change set.
    DryRun,
}

/// The script that renders the state of a graph after a curated script: data, schema, catalog
/// and statistics.
const STATE: &str = r#"[
    g.V().elementMap().toList(),
    g.E().elementMap().toList(),
    g.get_schema(),
    g.queries().map(|q| [q.name, q.folder, q.description]),
    g.compressions().map(|r| [r.name, r.path, r.compressed_values]),
    g.statistics()
]"#;

/// What a curated script did on one storage: its rendered outcome, the change set of a dry run
/// (compacted), the state afterwards and the commit position.
#[derive(PartialEq, Debug)]
struct Outcome {
    result: Result<String, String>,
    changes: Option<ChangeSet>,
    state: Result<String, String>,
    last_commit_seq: u64,
}

fn run_curated<S>(graph: Arc<Graph<S>>, mode: Mode, script: &str) -> Outcome
where
    S: GraphStorage + Default + Send + Sync + 'static,
{
    let text = format!("({{ {script} }}).to_string()");
    let limits = ScriptLimits::default();
    let (result, changes) = match mode {
        Mode::Plain => (
            eval_value_with_limits(graph.clone(), &text, HashMap::new(), &limits),
            None,
        ),
        Mode::Atomic => (
            eval_value_atomic(graph.clone(), &text, HashMap::new(), &limits),
            None,
        ),
        Mode::DryRun => match eval_value_dry_run(graph.clone(), &text, HashMap::new(), &limits) {
            Ok((value, changes)) => (Ok(value), Some(changes.compact())),
            Err(err) => (Err(err), None),
        },
    };
    let last_commit_seq = graph.read().last_commit_seq();
    Outcome {
        result: result
            .map(|value| value.to_string())
            .map_err(|err| err.to_string()),
        changes,
        state: render(graph, STATE),
        last_commit_seq,
    }
}

/// The curated scripts: `(mode, script)`.
const CURATED: &[(Mode, &str)] = &[
    // Mutations, one unit per traversal.
    (
        Mode::Plain,
        r#"g.addV("person").property("name", "ann").property("age", 41).values("name").next()"#,
    ),
    (
        Mode::Plain,
        r#"g.V("1").addE("likes").to(__.V("3")).property("w", 0.5).label().next()"#,
    ),
    (
        Mode::Plain,
        r#"g.V().hasLabel("software").property("lang", "rust").iterate(); g.V().values("lang").toList()"#,
    ),
    (
        Mode::Plain,
        r#"g.V("1").drop().iterate(); [g.V().count().next(), g.E().count().next()]"#,
    ),
    (
        Mode::Plain,
        r#"g.E().hasLabel("knows").drop().iterate(); g.E().count().next()"#,
    ),
    (
        Mode::Plain,
        r#"g.V("2").properties("age").drop().iterate(); g.V("2").valueMap().next()"#,
    ),
    (
        Mode::Plain,
        r#"g.V("1").property(jpath("address.city"), "Berlin").iterate(); g.V("1").values(jpath("address.city")).next()"#,
    ),
    (
        Mode::Plain,
        r#"g.V("1").add_label("employee").labels().next()"#,
    ),
    (
        Mode::Plain,
        r#"g.mergeV(#{"T.id": "9", "T.label": "person", name: "zed"}).values("name").next()"#,
    ),
    (
        Mode::Plain,
        r#"g.V("1").addE("knows").to(__.V("1")).iterate(); g.V("1").out("knows").id().toList()"#,
    ),
    // Whole-script units: commit and rollback.
    (
        Mode::Atomic,
        r#"g.addV("person").property("name", "bo").next(); g.V("6").drop().iterate(); g.V().count().next()"#,
    ),
    (
        Mode::Atomic,
        r#"g.addV("person").property("name", "bo").next(); throw "boom""#,
    ),
    (
        Mode::Atomic,
        r#"g.V("1").property("age", 30).iterate(); g.V("1").values("age").next()"#,
    ),
    (
        Mode::Atomic,
        r#"g.set_schema(g.infer_schema(), SchemaMode.closed); g.addV("robot").next()"#,
    ),
    // Dry runs: the change set is compared too.
    (
        Mode::DryRun,
        r#"g.addV("person").property("name", "cy").next(); g.V("1").property("age", 30).iterate(); g.V().count().next()"#,
    ),
    (
        Mode::DryRun,
        r#"g.V("1").drop().iterate(); g.E().count().next()"#,
    ),
    (
        Mode::DryRun,
        r#"g.set_schema(g.infer_schema(), SchemaMode.open); g.define_query(#{name: "n", body: "fn n() { 1 }"}); 1"#,
    ),
    // Saved queries.
    (
        Mode::Plain,
        r#"g.define_query(#{name: "older", folder: "people", description: "Older than min.", params: #{min: #{schema: #{type: "integer"}, default: 30}}, body: "fn older(min) { g.v().has(\"age\", P.gt(min)) }"}); [g.query("older").values("name").toList(), g.query("older", #{min: 33}).count().next()]"#,
    ),
    (
        Mode::Plain,
        r#"g.define_queries("reports", "/** Persons. */ fn people() { g.v().has_label(\"person\") }\n/** Software. */ fn software() { g.v().has_label(\"software\") }"); [g.query("people").count().next(), g.query("software").count().next(), g.queries("reports").map(|q| q.name)]"#,
    ),
    (
        Mode::Plain,
        r#"g.define_query(#{name: "n", body: "fn n() { 1 }"}); g.move_query("n", "misc"); [g.drop_query("n"), g.drop_query("n")]"#,
    ),
    (
        Mode::Plain,
        r#"g.define_query(#{name: "w", body: "fn w() { g.addV(\"x\") }"}); g.query("w").next()"#,
    ),
    (Mode::Plain, r#"g.query("missing").next()"#),
    // Schemas in every mode, violations.
    (
        Mode::Plain,
        r#"g.set_schema(g.infer_schema()); g.get_schema()["mode"]"#,
    ),
    (
        Mode::Plain,
        r#"g.set_schema(g.infer_schema(), SchemaMode.open); g.addV("robot").property("x", 1).count().next()"#,
    ),
    (
        Mode::Plain,
        r#"g.set_schema(g.infer_schema(), SchemaMode.closed); g.V("1").property("age", "x").iterate()"#,
    ),
    (
        Mode::Plain,
        r#"g.set_schema(#{mode: "closed", vertices: #{person: #{schema: #{type: "object", properties: #{name: #{type: "integer"}}}}}})"#,
    ),
    (
        Mode::Plain,
        r#"g.validate_schema(#{mode: "open", vertices: #{person: #{schema: #{type: "object", properties: #{age: #{type: "string"}}}}}})"#,
    ),
    (
        Mode::Plain,
        r#"g.set_schema(g.infer_schema(), SchemaMode.open); g.patch_schema(#{vertices: #{robot: #{schema: #{type: "object"}}}})["vertices"].keys()"#,
    ),
    // The catalog: compression rules.
    (
        Mode::Plain,
        r#"g.define_compression(#{name: "names", label: "person", path: "name", min_bytes: 1}); [g.recompress("names"), g.compressions()[0].compressed_values, g.V().hasLabel("person").values("name").toList(), g.drop_compression("names")]"#,
    ),
    // Marks: no journal on either storage.
    (Mode::Plain, r#"g.mark("m")"#),
    (Mode::Atomic, r#"g.addV("x").next(); g.mark("m")"#),
];

#[test]
fn curated_scripts_give_the_same_result_on_both_storages() {
    let mut differences = Vec::new();
    for (mode, script) in CURATED {
        let expected = run_curated(reference(), *mode, script);
        let actual = run_curated(forwarded(), *mode, script);
        if expected != actual {
            differences.push(format!(
                "{mode:?} `{script}`\n  TraversalGraph: {expected:?}\n  Forwarding:     {actual:?}"
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

/// A file of this test under the system temp directory, removed on drop.
struct TempFile(std::path::PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "graphersal-foreign-storage-{}-{name}",
            std::process::id()
        )))
    }
    fn path(&self) -> String {
        self.0.display().to_string().replace('\\', "/")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn graphson_export_and_import_give_the_same_result_on_both_storages() {
    // Export: the files are equal.
    let reference_file = TempFile::new("reference.json");
    let forwarded_file = TempFile::new("forwarded.json");
    let export = |path: &str| format!(r#"g.export_graphson("{path}")"#);
    let expected = render(reference(), &export(&reference_file.path()))
        .map(|text| text.replace(&reference_file.path(), "<file>"));
    let actual = render(forwarded(), &export(&forwarded_file.path()))
        .map(|text| text.replace(&forwarded_file.path(), "<file>"));
    assert!(expected.is_ok(), "{expected:?}");
    assert_eq!(expected, actual);
    assert_eq!(
        std::fs::read_to_string(&reference_file.0).unwrap(),
        std::fs::read_to_string(&forwarded_file.0).unwrap()
    );
    // Import (the reference's export) into an emptied graph: same report, same state.
    let import = format!(
        r#"g.V().drop().iterate(); let report = g.import_graphson("{}"); [report, g.V().count().next(), g.E().count().next()]"#,
        reference_file.path()
    );
    let expected = run_curated(reference(), Mode::Plain, &import);
    let actual = run_curated(forwarded(), Mode::Plain, &import);
    assert!(expected.result.is_ok(), "{expected:?}");
    assert_eq!(expected, actual);
    // And as one whole-script unit.
    let expected = run_curated(reference(), Mode::Atomic, &import);
    let actual = run_curated(forwarded(), Mode::Atomic, &import);
    assert_eq!(expected, actual);
}

/// `export_snapshot` on a `Forwarding` graph built by `Graph::new_persistent` writes the
/// snapshot the reference writes (also inside a whole-script unit, which moves the storage into
/// a private graph); built by plain `Graph::new` it is unsupported.
#[cfg(feature = "persist")]
#[test]
fn export_snapshot_writes_the_reference_bytes_on_a_persistent_forwarding_graph() {
    use graphersal::persist::read_snapshot_info;
    // The data sections of a packed snapshot (after the manifest, which records the write
    // time) and the manifest's listing without the write time.
    fn data(path: &std::path::Path) -> (Vec<u8>, graphersal::persist::SnapshotInfo) {
        let bytes = std::fs::read(path).unwrap();
        let len = u64::from_le_bytes(bytes[17..25].try_into().unwrap()) as usize;
        let mut info = read_snapshot_info(bytes.as_slice()).unwrap();
        info.created_at = 0;
        (bytes[25 + len..bytes.len() - 4].to_vec(), info)
    }
    // A fixed lineage id (every segment header carries it), so both graphs write the same bytes.
    fn modern_with_graph_id() -> TraversalGraph {
        let mut graph = TraversalGraph::tinkerpop_modern();
        graphersal::storage::PersistentStorage::set_graph_id(&mut graph, 7);
        graph
    }
    let persistent = || {
        Arc::new(Graph::new_persistent(Forwarding::new(
            modern_with_graph_id(),
        )))
    };
    let export = |path: &str| format!(r#"g.export_snapshot("{path}")"#);
    let reference_file = TempFile::new("reference.gsnap");
    let reference = Arc::new(Graph::new(modern_with_graph_id()));
    let expected = render(reference, &export(&reference_file.path()));
    assert!(expected.is_ok(), "{expected:?}");
    for mode in [Mode::Plain, Mode::Atomic] {
        let forwarded_file = TempFile::new("forwarded.gsnap");
        let actual = run_curated(persistent(), mode, &export(&forwarded_file.path()));
        assert!(actual.result.is_ok(), "{actual:?}");
        assert!(
            data(&forwarded_file.0) == data(&reference_file.0),
            "{mode:?}"
        );
    }

    let forwarded_file = TempFile::new("forwarded.gsnap");
    let err = render(forwarded(), &export(&forwarded_file.path())).unwrap_err();
    assert!(
        err.contains("not supported") || err.contains("does not support"),
        "{err}"
    );
    // The refused export leaves no partial file behind.
    assert!(!forwarded_file.0.exists());
}
