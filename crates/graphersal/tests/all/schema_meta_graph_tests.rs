use graphersal::prelude::GraphSource;
use graphersal::prelude::*;
use graphersal::schema::PropertySchema;

#[test]
fn test_schema_to_meta_graph_tinkerpop_modern() {
    let tg = TraversalGraph::tinkerpop_modern();
    let schema = tg.traversal().infer_schema().unwrap();

    let meta_tg = schema.to_graph();

    // 1. Check graph element counts
    assert_eq!(meta_tg.vertex_count(), 2);
    assert_eq!(meta_tg.edge_count(), 2);

    let g = meta_tg.traversal();

    // 2. Traversal verification: out("created") from "person" should reach "software"
    let created_targets = g
        .v("person")
        .out("created")
        .values("label_name")
        .to_list()
        .expect("traversal failed");
    assert_eq!(
        created_targets,
        vec![ElementProperty::String("software".into())]
    );

    // 3. Traversal verification: out_e("knows") count from "person"
    let knows_count = meta_tg
        .traversal()
        .v("person")
        .out_e("knows")
        .count()
        .to_list()
        .expect("traversal failed");
    assert_eq!(knows_count, vec![ElementProperty::Number(1.into())]);

    // 4. Traversal verification: out("knows") self-loop on "person"
    let knows_targets = meta_tg
        .traversal()
        .v("person")
        .out("knows")
        .values("label_name")
        .to_list()
        .expect("traversal failed");
    assert_eq!(
        knows_targets,
        vec![ElementProperty::String("person".into())]
    );

    // 5. Traversal verification: in("created") on "software" should reach "person"
    let software_creators = meta_tg
        .traversal()
        .v("software")
        .in_("created")
        .values("label_name")
        .to_list()
        .expect("traversal failed");
    assert_eq!(
        software_creators,
        vec![ElementProperty::String("person".into())]
    );

    // 6. The label vertex carries its schema node (the canonical shape).
    let person_schema = meta_tg
        .traversal()
        .v("person")
        .values("schema")
        .to_list()
        .expect("traversal failed");
    assert_eq!(person_schema.len(), 1);
    let node = PropertySchema::from_value(&person_schema[0]).expect("a schema node");
    assert_eq!(node.properties["name"].type_name(), "string");
    assert_eq!(node.properties["age"].type_name(), "int64");
    assert_eq!(node.required, vec!["name", "age"]);

    // 7. The edge carries the edge label's schema node.
    let created_schema = meta_tg
        .traversal()
        .e(None)
        .has_label("created")
        .values("schema")
        .to_list()
        .expect("traversal failed");
    assert_eq!(created_schema.len(), 1);
    let node = PropertySchema::from_value(&created_schema[0]).expect("a schema node");
    assert_eq!(node.properties["weight"].type_name(), "float64");
}

#[test]
fn test_schema_subgraph_filtering() {
    let tg = TraversalGraph::tinkerpop_modern();
    let schema = tg.traversal().infer_schema().unwrap();

    // Filtering subgraph for only "person"
    let person_subgraph_schema = schema.subgraph("person");
    assert_eq!(person_subgraph_schema.vertices.len(), 1);
    assert!(person_subgraph_schema.vertices.contains_key("person"));

    // "knows" connects person->person, so it should be retained
    assert_eq!(person_subgraph_schema.edges.len(), 1);
    assert!(person_subgraph_schema.edges.contains_key("knows"));
    // "created" connects person->software, so it should be excluded
    assert!(!person_subgraph_schema.edges.contains_key("created"));

    // Build meta-graph from filtered schema
    let meta_tg = person_subgraph_schema.to_graph();
    assert_eq!(meta_tg.vertex_count(), 1);
    assert_eq!(meta_tg.edge_count(), 1);

    let knows_count = meta_tg
        .traversal()
        .v("person")
        .out("knows")
        .values("label_name")
        .to_list()
        .expect("traversal failed");
    assert_eq!(knows_count, vec![ElementProperty::String("person".into())]);
}

#[test]
fn test_schema_subgraph_multi_labels() {
    let tg = TraversalGraph::tinkerpop_modern();
    let schema = tg.traversal().infer_schema().unwrap();

    let full_subgraph = schema.subgraph_labels(&["person", "software"]);
    assert_eq!(full_subgraph.vertices.len(), 2);
    assert_eq!(full_subgraph.edges.len(), 2);

    let meta_tg = full_subgraph.to_graph();
    assert_eq!(meta_tg.vertex_count(), 2);
    assert_eq!(meta_tg.edge_count(), 2);
}

#[test]
fn test_schema_traversal_subgraph_cap_step() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let res = g
        .v(None)
        .has_label("person")
        .infer_schema_step()
        .subgraph("meta_schema")
        .cap("meta_schema")
        .to_list()
        .expect("traversal failed");

    assert_eq!(res.len(), 1);
    match &res[0] {
        ElementProperty::Object(obj) => {
            assert!(obj.contains_key("vertices"));
            assert!(obj.contains_key("edges"));
            if let Some(ElementProperty::Object(verts)) = obj.get("vertices") {
                assert!(verts.contains_key("person"));
            } else {
                panic!("Expected vertices map");
            }
        }
        other => panic!(
            "Expected Object for captured schema side effect, got {:?}",
            other
        ),
    }
}
