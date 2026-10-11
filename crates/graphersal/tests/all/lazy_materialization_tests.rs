use graphersal::prelude::By::Label;
use graphersal::{ElementProperty, GraphSource, prelude::ExecOptions, profile::ProfileType};

#[test]
fn test_v_group_by_label_vertex_materialization() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let res = g.v(None::<()>).group().by(Label).to_list().unwrap();
    assert_eq!(res.len(), 1);

    match &res[0] {
        ElementProperty::Object(map) => {
            let person_array = map
                .get("person")
                .expect("person entry")
                .as_array()
                .expect("array of persons");
            assert_eq!(person_array.len(), 4);
            for p in person_array {
                match p {
                    ElementProperty::Vertex(v_map) => {
                        assert!(v_map.contains_key("id"));
                        assert_eq!(
                            v_map.get("label").and_then(|l| l.as_string()),
                            Some("person")
                        );
                        assert!(v_map.contains_key("name"));
                    }
                    _ => panic!("Expected ElementProperty::Vertex, got {:?}", p),
                }
            }

            let software_array = map
                .get("software")
                .expect("software entry")
                .as_array()
                .expect("array of softwares");
            assert_eq!(software_array.len(), 2);
            for s in software_array {
                match s {
                    ElementProperty::Vertex(v_map) => {
                        assert!(v_map.contains_key("id"));
                        assert_eq!(
                            v_map.get("label").and_then(|l| l.as_string()),
                            Some("software")
                        );
                        assert!(v_map.contains_key("name"));
                    }
                    _ => panic!("Expected ElementProperty::Vertex, got {:?}", s),
                }
            }
        }
        _ => panic!("Expected ElementProperty::Object, got {:?}", res[0]),
    }
}

#[test]
fn test_e_group_by_label_edge_materialization() {
    let graph = GraphSource::tinkerpop_modern();
    let g_lock = graph.read();
    let g = g_lock.traversal();

    let res = g.e(None::<()>).group().by(Label).to_list().unwrap();
    assert_eq!(res.len(), 1);

    match &res[0] {
        ElementProperty::Object(map) => {
            let knows_array = map
                .get("knows")
                .expect("knows entry")
                .as_array()
                .expect("array of knows edges");
            assert_eq!(knows_array.len(), 2);
            for e in knows_array {
                match e {
                    ElementProperty::Edge(e_map) => {
                        assert!(e_map.contains_key("id"));
                        assert_eq!(
                            e_map.get("label").and_then(|l| l.as_string()),
                            Some("knows")
                        );
                        assert!(e_map.contains_key("_from"));
                        assert!(e_map.contains_key("_to"));
                    }
                    _ => panic!("Expected ElementProperty::Edge, got {:?}", e),
                }
            }

            let created_array = map
                .get("created")
                .expect("created entry")
                .as_array()
                .expect("array of created edges");
            assert_eq!(created_array.len(), 4);
            for e in created_array {
                match e {
                    ElementProperty::Edge(e_map) => {
                        assert!(e_map.contains_key("id"));
                        assert_eq!(
                            e_map.get("label").and_then(|l| l.as_string()),
                            Some("created")
                        );
                        assert!(e_map.contains_key("_from"));
                        assert!(e_map.contains_key("_to"));
                    }
                    _ => panic!("Expected ElementProperty::Edge, got {:?}", e),
                }
            }
        }
        _ => panic!("Expected ElementProperty::Object, got {:?}", res[0]),
    }
}

#[test]
fn test_large_graph_vertex_materialization_and_profile() {
    let graph = GraphSource::empty();
    {
        let mut tg = graph.write();
        let mut g = tg.traversal_mut();
        for i in 0..10_000 {
            let label = if i % 2 == 0 { "user" } else { "device" };
            g = g
                .add_v(label)
                .property("idx", i as i64)
                .property("active", true);
        }
        let _ = g.to_list().unwrap();
    }

    let g_lock = graph.read();
    let stats = g_lock
        .traversal()
        .v(None::<()>)
        .group()
        .by(Label)
        .execute_with(ExecOptions::profile(ProfileType::none()));

    assert_eq!(stats.results().len(), 1);
    match &stats.results()[0] {
        ElementProperty::Object(map) => {
            let users = map.get("user").unwrap().as_array().unwrap();
            let devices = map.get("device").unwrap().as_array().unwrap();
            assert_eq!(users.len(), 5_000);
            assert_eq!(devices.len(), 5_000);
        }
        _ => panic!("Expected Object"),
    }

    // The run is measured.
    assert!(stats.profile().unwrap().duration().as_nanos() > 0);
}
