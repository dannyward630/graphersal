//! Downstream-style tests for the additive API contract: public enums and error variants are
//! `#[non_exhaustive]`, so code outside `graphersal` matches them with a `_` arm and destructures
//! error variants with `..`. Adding a variant or a field must not break this file.

use graphersal::prelude::*;

/// Renders a value the way a downstream consumer would: known variants explicitly, everything
/// added later through a fallback.
fn describe(value: &ElementProperty) -> String {
    match value {
        ElementProperty::String(s) => format!("string {s}"),
        ElementProperty::Boolean(b) => format!("boolean {b}"),
        _ => String::from("other"),
    }
}

#[test]
fn a_downstream_match_on_element_property_needs_a_fallback_arm() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let names = lock
        .traversal()
        .v("1")
        .values("name")
        .to_list()
        .expect("marko has a name");
    assert_eq!(
        names.iter().map(describe).collect::<Vec<_>>(),
        ["string marko"]
    );
}

#[test]
fn a_downstream_match_on_a_query_error_uses_a_fallback_and_rest_patterns() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    let err = lock
        .traversal()
        .with("repeat.max_loops", 3)
        .v("1")
        .repeat(__::both(None))
        .to_list()
        .expect_err("an unbounded repeat() hits the loop limit");
    let summary = match err.root_cause() {
        TraverserError::RepeatLimitExceeded { max_loops, .. } => format!("loop limit {max_loops}"),
        other => format!("other: {other}"),
    };
    assert_eq!(summary, "loop limit 3");
}

#[test]
fn downstream_matches_on_the_property_errors_use_rest_patterns() {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let err = graph
        .traversal_mut()
        .v("1")
        .property_with(Cardinality::Set, "age", 1)
        .next()
        .expect_err("set cardinality");
    let summary = match err.root_cause() {
        TraverserError::UnsupportedCardinality { cardinality, .. } => {
            format!("cardinality {cardinality}")
        }
        TraverserError::TokenPropertyOutsideAdd { token, .. } => format!("token {token}"),
        other => format!("other: {other}"),
    };
    assert_eq!(summary, "cardinality set");
}

#[test]
fn downstream_matches_on_the_merge_errors_use_rest_patterns() {
    let mut graph = TraversalGraph::new();
    let mut map = PropertyMap::default();
    map.insert("~id", "1".into());
    let err = graph
        .traversal_mut()
        .merge_v(ElementProperty::Object(Box::new(map)))
        .next()
        .expect_err("hidden key");
    let summary = match err.root_cause() {
        TraverserError::InvalidMergeMap { step, .. } => format!("invalid map of {step}"),
        TraverserError::MergeOverride { key, .. } => format!("override of {key}"),
        other => format!("other: {other}"),
    };
    assert_eq!(summary, "invalid map of merge_v");

    let err = graph
        .traversal_mut()
        .merge_e(ElementProperty::Null)
        .next()
        .expect_err("no vertices");
    let summary = match err.root_cause() {
        TraverserError::MergeVertexUnresolved { direction, .. } => {
            format!("unresolved {direction}")
        }
        other => format!("other: {other}"),
    };
    assert_eq!(summary, "unresolved OUT");
}

#[cfg(feature = "io")]
#[test]
fn from_file_rejects_an_unknown_extension() {
    match GraphSource::from_file("graph.csv") {
        Ok(_) => panic!("GraphSON and GraphML are the graph file formats"),
        Err(IOError::UnsupportedFormat { path, .. }) => assert_eq!(path, "graph.csv"),
        Err(other) => panic!("expected UnsupportedFormat, got {other:?}"),
    }
    // A `.json` path is GraphSON: a missing file is a read error, not an unsupported format.
    match GraphSource::from_file("definitely-missing-graph.json") {
        Err(IOError::FileRead { .. }) => {}
        Err(other) => panic!("expected FileRead, got {other:?}"),
        Ok(_) => panic!("the file does not exist"),
    }
}

#[cfg(feature = "io")]
#[test]
fn from_file_loads_graphml_by_extension() {
    use graphersal::io::GraphExport;

    let path = std::env::temp_dir().join("api_stability_from_file.GRAPHML");
    let path_text = path.to_str().expect("temp path is UTF-8");
    GraphSource::tinkerpop_modern()
        .export_graphml(path_text)
        .expect("export succeeds");
    let loaded = GraphSource::from_file(&path);
    let _ = std::fs::remove_file(&path);
    let graph = loaded.expect("an upper-case .GRAPHML extension is still GraphML");
    assert_eq!(graph.read().vertex_count(), 6);
}

/// `Merge` is `#[non_exhaustive]` (TinkerPop already extended it once), so a downstream match
/// needs a `_` arm.
fn describe_merge(token: Merge) -> &'static str {
    match token {
        Merge::OnCreate => "create",
        Merge::OnMatch => "match",
        _ => "vertex option",
    }
}

/// `Cardinality` and `Direction` are closed sets: a downstream match is exhaustive without `_`.
fn describe_closed(cardinality: Cardinality, direction: Direction) -> (&'static str, &'static str) {
    let c = match cardinality {
        Cardinality::Single => "single",
        Cardinality::List => "list",
        Cardinality::Set => "set",
    };
    let d = match direction {
        Direction::Out => "out",
        Direction::In => "in",
        Direction::Both => "both",
    };
    (c, d)
}

/// `Pop` is a closed set (TinkerPop's four members): a downstream match is exhaustive without `_`.
fn describe_pop(pop: Pop) -> &'static str {
    match pop {
        Pop::First => "first",
        Pop::Last => "last",
        Pop::All => "all",
        Pop::Mixed => "mixed",
    }
}

#[test]
fn pop_is_a_closed_set() {
    assert_eq!(describe_pop(Pop::Mixed), "mixed");
    assert_eq!(describe_pop(Pop::default()), "last");
}

#[test]
fn token_class_matches_follow_the_non_exhaustive_policy() {
    assert_eq!(describe_merge(Merge::OnCreate), "create");
    assert_eq!(describe_merge(Merge::OutV), "vertex option");
    assert_eq!(
        describe_closed(Cardinality::Set, Direction::Both),
        ("set", "both")
    );
}

/// The topic facade modules name the same items as their home modules, so a downstream crate can
/// import from either path while the downstream switch is in progress.
#[test]
fn the_facade_modules_reach_the_host_and_storage_types() {
    use graphersal::catalog::DefinitionKind;
    use graphersal::{auth, catalog, changes, exec, profile, schema, storage};

    let policy: auth::AccessPolicy = auth::AccessPolicy::read_only();
    let _: &dyn auth::Authorizer = &policy;
    let _: &dyn auth::Authorizer = &auth::AllowAll;
    assert_eq!(catalog::DefinitionKind::QUERY, DefinitionKind::QUERY);
    let _: Option<changes::ChangeSet> = None;
    let _: Option<Box<dyn changes::CommitHook>> = None;
    let _: Option<exec::CancelToken> = None;
    assert!(!exec::traversal_limit_defaults().is_empty());
    let _: Option<profile::TraversalMetrics> = None;
    let _: schema::SchemaMode = SchemaMode::Open;
    assert!(storage::StorageCapabilities::none().is_consistent());
    let _: Order = Order::Desc;
}

/// A storage outside the crate builds the errors its contract asks for through constructors,
/// because the variants are `#[non_exhaustive]`.
#[test]
fn a_storage_outside_the_crate_builds_its_contract_errors() {
    use graphersal::storage::{GraphError, GraphNodeType};

    assert!(matches!(
        GraphError::unsupported("schemas"),
        GraphError::Unsupported {
            operation: "schemas",
            ..
        }
    ));
    assert!(matches!(
        GraphError::element_removed(GraphNodeType::Edge),
        GraphError::ElementRemoved {
            kind: GraphNodeType::Edge,
            ..
        }
    ));
    assert!(matches!(
        GraphError::duplicate_vertex_id("1"),
        GraphError::VertexIdUniqueConstraint { id, .. } if id == "1"
    ));
    assert!(matches!(
        GraphError::duplicate_edge_id("7"),
        GraphError::EdgeIdUniqueConstraint { id, .. } if id == "7"
    ));
    assert!(matches!(
        GraphError::vertex_not_found("x"),
        GraphError::VertexNotFound { id, .. } if id == "x"
    ));
    assert!(matches!(
        GraphError::edge_not_found("y"),
        GraphError::EdgeNotFound { id, .. } if id == "y"
    ));
    assert!(matches!(
        GraphError::invalid_label("a::b"),
        GraphError::InvalidLabel { label, .. } if label == "a::b"
    ));
    assert!(matches!(
        GraphError::invalid_element_state("torn"),
        GraphError::InvalidElementState { reason, .. } if reason == "torn"
    ));
}

/// The constructors of the property errors build exactly what the crate's own write path returns
/// (same text, same help), so a foreign storage answers like `TraversalGraph`.
#[test]
fn the_property_error_constructors_match_the_crate_path() {
    use graphersal::error::PathWriteFault;
    use graphersal::storage::{GraphError, GraphStorage, JsonPath, PropertyMap, TraversalGraph};

    let mut graph = TraversalGraph::new();
    let mut props = PropertyMap::default();
    props.insert(CompactString::from("name"), "ann".into());
    let v = graph
        .add_vertex(None, vec!["person".into()], props)
        .expect("a plain vertex");

    let element = ElementProperty::Vertex(Box::default());
    let crate_err = graph
        .set_vertex_property(v, "friend".into(), element.clone())
        .expect_err("a vertex is not a property value");
    let built = GraphError::invalid_property_value("friend", "vertex");
    assert_eq!(crate_err.to_string(), built.to_string());
    assert_eq!(crate_err.help(), built.help());
    let helper = graphersal::storage::check_property_value("friend", &element).unwrap_err();
    assert_eq!(helper.to_string(), built.to_string());

    let path: JsonPath = "name.x".parse().expect("a path");
    let crate_err = graph
        .set_vertex_property_path(v, &path, 1_i64.into())
        .expect_err("a key on a string");
    let built = GraphError::invalid_property_path(
        &path,
        PathWriteFault::KeyOnNonObject {
            at: "$.name".to_string(),
            found: "string",
        },
    );
    assert_eq!(crate_err.to_string(), built.to_string());
    assert_eq!(crate_err.help(), built.help());
    let helper =
        graphersal::storage::write_path(Some(&"ann".into()), &path, 1_i64.into()).unwrap_err();
    assert_eq!(helper.to_string(), built.to_string());
}

/// `JsonPath::segments()` exposes the parsed path; printing and parsing again gives the same
/// segments for names, bracket names, indices and negative indices.
#[test]
fn json_path_segments_round_trip() {
    use graphersal::storage::{JsonPath, JsonPathSegment};

    let key = |name: &str| JsonPathSegment::Key(name.into());
    let cases: [(&str, Vec<JsonPathSegment>); 4] = [
        ("meta.k", vec![key("meta"), key("k")]),
        ("$.tags[2]", vec![key("tags"), JsonPathSegment::Index(2)]),
        (
            "tags[-1].name",
            vec![key("tags"), JsonPathSegment::Index(-1), key("name")],
        ),
        ("$['a.b'][\"c d\"]", vec![key("a.b"), key("c d")]),
    ];
    for (text, expected) in cases {
        let path: JsonPath = text.parse().expect("a valid path");
        assert_eq!(path.segments(), expected.as_slice(), "{text}");
        let again: JsonPath = path.to_string().parse().expect("the canonical text parses");
        assert_eq!(again.segments(), path.segments(), "{text}");
        let described: Vec<String> = path
            .segments()
            .iter()
            .map(|segment| match segment {
                JsonPathSegment::Key(name) => format!("key {name}"),
                JsonPathSegment::Index(index) => format!("index {index}"),
                _ => String::from("other"),
            })
            .collect();
        assert_eq!(described.len(), expected.len());
    }
}
