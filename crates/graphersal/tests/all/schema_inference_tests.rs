//! Schema inference: the inferred schema admits exactly the kinds seen at each position, a key
//! is required only where every observed object has it (at any depth: F1), every label of a
//! vertex is declared, and the result is valid for the data it came from, in open and closed
//! mode. Rule 7: inference never guesses a type from string content.
use graphersal::prelude::*;
use graphersal::schema::{EdgeConnection, JsonType, PropertySchema};

use crate::schema_support::props;

fn infer(graph: &TraversalGraph) -> GraphSchema {
    graph.traversal().infer_schema().unwrap()
}

fn node<'a>(schema: &'a GraphSchema, label: &str, key: &str) -> &'a PropertySchema {
    &schema.vertices[label].schema.properties[key]
}

/// The inferred schema, applied in open and closed mode, accepts its own data.
fn assert_valid_for_its_data(graph: &TraversalGraph) {
    let inferred = infer(graph);
    for mode in [SchemaMode::Open, SchemaMode::Closed] {
        let validation = graph
            .traversal()
            .validate_schema(&inferred.clone().with_mode(mode))
            .unwrap();
        assert!(
            validation.is_valid(),
            "{mode:?}: {}\n{}",
            validation.violations,
            inferred.to_json_pretty()
        );
    }
}

#[test]
fn test_tinkerpop_modern_schema_inference() {
    let tg = TraversalGraph::tinkerpop_modern();
    let schema = infer(&tg);
    assert_eq!(schema.mode, SchemaMode::None);
    assert!(schema.meta.is_none());

    assert_eq!(schema.vertices.len(), 2);
    let person = &schema.vertices["person"].schema;
    assert_eq!(person.properties.len(), 2);
    assert_eq!(person.properties["name"].type_name(), "string");
    assert_eq!(person.properties["age"].type_name(), "int64");
    assert_eq!(person.required, vec!["name", "age"]);
    let software = &schema.vertices["software"].schema;
    assert_eq!(software.required, vec!["name", "lang"]);

    assert_eq!(schema.edges.len(), 2);
    let knows = &schema.edges["knows"];
    assert_eq!(knows.schema.properties["weight"].type_name(), "float64");
    assert_eq!(
        knows.connections,
        vec![EdgeConnection::new("person", "person")]
    );
    let created = &schema.edges["created"];
    assert_eq!(
        created.connections,
        vec![EdgeConnection::new("person", "software")]
    );
    // No inference statistics in the format.
    let text = schema.to_json();
    assert!(
        !text.contains("count") && !text.contains("occurrences"),
        "{text}"
    );
    assert_valid_for_its_data(&tg);
}

#[test]
fn test_nested_object_property_inference() {
    let mut tg = TraversalGraph::new();
    let mut addr1 = PropertyMap::default();
    addr1.insert("city", ElementProperty::String("New York".into()));
    addr1.insert("zip", ElementProperty::Number(10001.into()));
    let mut addr2 = PropertyMap::default();
    addr2.insert("city", ElementProperty::String("San Francisco".into()));
    tg.add_vertex(
        Some("1".into()),
        vec!["user".into()],
        props(&[
            ("name", "Alice".into()),
            ("address", ElementProperty::Object(Box::new(addr1))),
        ]),
    )
    .unwrap();
    tg.add_vertex(
        Some("2".into()),
        vec!["user".into()],
        props(&[
            ("name", "Bob".into()),
            ("address", ElementProperty::Object(Box::new(addr2))),
        ]),
    )
    .unwrap();

    let schema = infer(&tg);
    let address = node(&schema, "user", "address");
    assert_eq!(address.type_name(), "object");
    assert_eq!(address.properties["city"].type_name(), "string");
    assert_eq!(address.properties["zip"].type_name(), "int64");
    // zip is present in only one of the two address objects
    assert_eq!(address.required, vec!["city"]);
    assert_eq!(
        schema.vertices["user"].schema.required,
        vec!["name", "address"]
    );
    assert_valid_for_its_data(&tg);
}

#[test]
fn test_polymorphic_types_and_null() {
    let mut tg = TraversalGraph::new();
    tg.add_vertex(
        None,
        vec!["item".into()],
        props(&[("val", 42.into()), ("opt", "present".into())]),
    )
    .unwrap();
    tg.add_vertex(
        None,
        vec!["item".into()],
        props(&[("val", "forty-two".into())]),
    )
    .unwrap();
    tg.add_vertex(
        None,
        vec!["item".into()],
        props(&[("val", true.into()), ("opt", ElementProperty::Null)]),
    )
    .unwrap();

    let schema = infer(&tg);
    let val = node(&schema, "item", "val");
    assert_eq!(
        val.types,
        Some(vec![JsonType::Boolean, JsonType::Integer, JsonType::String])
    );
    let opt = node(&schema, "item", "opt");
    // missing in one vertex, null in another: optional and nullable
    assert_eq!(opt.types, Some(vec![JsonType::String, JsonType::Null]));
    assert_eq!(schema.vertices["item"].schema.required, vec!["val"]);
    assert_valid_for_its_data(&tg);
}

/// A string and a uuid at one position cannot share one `type` array (the format applies to
/// every string): they become `anyOf` members.
#[test]
fn test_string_and_uuid_at_one_position_become_any_of() {
    let mut tg = TraversalGraph::new();
    tg.add_vertex(None, vec!["v".into()], props(&[("id", "text".into())]))
        .unwrap();
    tg.add_vertex(
        None,
        vec!["v".into()],
        props(&[("id", ElementProperty::Uuid(7))]),
    )
    .unwrap();
    let schema = infer(&tg);
    let id = node(&schema, "v", "id");
    assert_eq!(id.type_name(), "union<string|uuid>");
    assert!(id.any_of.is_some());
    assert_valid_for_its_data(&tg);
}

#[test]
fn test_multi_topology_connections() {
    let mut tg = TraversalGraph::new();
    let p1 = tg
        .add_vertex(
            Some("1".into()),
            vec!["person".into()],
            PropertyMap::default(),
        )
        .unwrap();
    let p2 = tg
        .add_vertex(
            Some("2".into()),
            vec!["person".into()],
            PropertyMap::default(),
        )
        .unwrap();
    let bot = tg
        .add_vertex(Some("3".into()), vec!["bot".into()], PropertyMap::default())
        .unwrap();
    for (from, to) in [(p1, p2), (p2, p1), (p1, bot)] {
        tg.add_edge(
            None,
            Some("messages".into()),
            from,
            to,
            PropertyMap::default(),
        )
        .unwrap();
    }
    let schema = infer(&tg);
    assert_eq!(
        schema.edges["messages"].connections,
        vec![
            EdgeConnection::new("person", "person"),
            EdgeConnection::new("person", "bot")
        ]
    );
    assert_valid_for_its_data(&tg);
}

#[test]
fn test_empty_graph_schema() {
    let schema = infer(&TraversalGraph::new());
    assert!(schema.is_empty());
}

#[test]
fn test_array_property_inference() {
    let mut tg = TraversalGraph::new();
    tg.add_vertex(
        None,
        vec!["post".into()],
        props(&[(
            "tags",
            ElementProperty::Array(vec!["rust".into(), "graph".into()]),
        )]),
    )
    .unwrap();
    tg.add_vertex(
        None,
        vec!["post".into()],
        props(&[
            (
                "mixed_arr",
                ElementProperty::Array(vec![10.into(), "ten".into()]),
            ),
            ("empty_arr", ElementProperty::Array(vec![])),
        ]),
    )
    .unwrap();
    let schema = infer(&tg);
    assert_eq!(node(&schema, "post", "tags").type_name(), "array<string>");
    assert_eq!(
        node(&schema, "post", "mixed_arr").type_name(),
        "array<union<int64|string>>"
    );
    // An always-empty array observed no item: `{"type": "array"}` without `items`.
    let empty = node(&schema, "post", "empty_arr");
    assert_eq!(empty.type_name(), "array");
    assert!(empty.items.is_none());
    assert!(schema.vertices["post"].schema.required.is_empty());
    assert_valid_for_its_data(&tg);
}

/// F1: keys missing from some objects inside arrays and next to other kinds are optional, so
/// the inferred schema applies to the data it came from.
#[test]
fn test_inferred_schemas_are_valid_for_their_data_f1() {
    let mut tg = TraversalGraph::new();
    let object =
        |entries: &[(&str, ElementProperty)]| ElementProperty::Object(Box::new(props(entries)));
    tg.add_vertex(
        None,
        vec!["svc".into()],
        props(&[
            (
                "config",
                object(&[("runtime", object(&[("cpu", 2.into())]))]),
            ),
            (
                "items",
                ElementProperty::Array(vec![
                    object(&[("a", 1.into()), ("b", 2.into())]),
                    object(&[("a", 3.into())]),
                ]),
            ),
        ]),
    )
    .unwrap();
    tg.add_vertex(
        None,
        vec!["svc".into()],
        props(&[
            (
                "config",
                object(&[(
                    "runtime",
                    object(&[("cpu", 2.into()), ("mem", "1G".into())]),
                )]),
            ),
            (
                "items",
                ElementProperty::Array(vec![object(&[("a", 1.into())])]),
            ),
        ]),
    )
    .unwrap();
    // `config` also as an int (a union of an object and an integer).
    tg.add_vertex(
        None,
        vec!["svc".into()],
        props(&[
            ("config", 5.into()),
            ("items", ElementProperty::Array(vec![])),
        ]),
    )
    .unwrap();
    let schema = infer(&tg);
    let items = node(&schema, "svc", "items").items.as_ref().unwrap();
    assert_eq!(
        items.required,
        vec!["a"],
        "b is missing in two of three items"
    );
    let config = node(&schema, "svc", "config");
    assert_eq!(
        config.types,
        Some(vec![JsonType::Integer, JsonType::Object])
    );
    let runtime = &config.properties["runtime"];
    assert_eq!(runtime.required, vec!["cpu"], "mem is missing once");
    assert_valid_for_its_data(&tg);
}

/// Every label of a multi-label vertex is declared and gets its properties, in the whole-graph
/// inference and in the `infer_schema()` step alike.
#[test]
fn test_multi_label_vertices_count_for_every_label() {
    let mut tg = TraversalGraph::new();
    let a = tg
        .add_vertex(
            None,
            vec!["A".into(), "B".into()],
            props(&[("x", 1.into())]),
        )
        .unwrap();
    let c = tg
        .add_vertex(None, vec!["C".into()], PropertyMap::default())
        .unwrap();
    tg.add_edge(None, Some("r".into()), a, c, PropertyMap::default())
        .unwrap();
    let whole = infer(&tg);
    let step = tg
        .traversal()
        .v(None)
        .infer_schema_step()
        .next()
        .unwrap()
        .expect("one schema");
    let step = GraphSchema::from_value(&step).unwrap();
    for schema in [&whole, &step] {
        assert_eq!(schema.vertices["A"].schema.required, vec!["x"]);
        assert_eq!(schema.vertices["B"].schema.required, vec!["x"]);
        assert!(schema.vertices.contains_key("C"));
    }
    assert_eq!(
        whole.edges["r"].connections,
        vec![EdgeConnection::new("A", "C")]
    );
    assert_valid_for_its_data(&tg);
}

/// The enforcement of CLAUDE.md rule 7: inference reports the logical type of each value
/// exactly as it is stored and never inspects string content; a real uuid is a uuid.
#[test]
fn test_inference_never_guesses_semantic_types() {
    let mut graph = TraversalGraph::new();
    let shaped_values = [
        ("uuid_like", "1e2e6a4f-2a4a-4c0b-9f3f-0d6b4f6f2a11"),
        ("date_iso", "2026-09-22"),
        ("date_ambiguous", "03/02/2024"),
        ("datetime_iso", "2026-09-22T10:00:00Z"),
        ("time_like", "10:00:00"),
        ("ipv4_like", "192.168.0.1"),
        ("decimal_like", "1234.5678"),
        ("bool_like", "true"),
    ];
    let entries: Vec<(&str, ElementProperty)> = shaped_values
        .iter()
        .map(|(k, v)| (*k, ElementProperty::from(*v)))
        .collect();
    graph
        .add_vertex(Some("v1".into()), vec!["Shaped".into()], props(&entries))
        .unwrap();
    graph
        .add_vertex(
            Some("v2".into()),
            vec!["Real".into()],
            props(&[(
                "uid",
                ElementProperty::Uuid(0x550e8400_e29b_41d4_a716_446655440000),
            )]),
        )
        .unwrap();
    let schema = infer(&graph);
    for (key, value) in shaped_values {
        let node = node(&schema, "Shaped", key);
        assert_eq!(node.type_name(), "string", "'{value}' must infer as string");
        assert!(node.format.is_none());
    }
    assert_eq!(node(&schema, "Real", "uid").type_name(), "uuid");
}
