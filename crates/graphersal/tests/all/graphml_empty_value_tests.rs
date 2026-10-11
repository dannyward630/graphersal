#![cfg(feature = "io")]

//! An empty `<data>` element on a `string` key is the property `""`, not a missing property.

use graphersal::io::{GraphImport, GraphMLExport};
use graphersal::prelude::*;

const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns">
  <key id="s" for="node" attr.name="s" attr.type="string"/>
  <key id="n" for="node" attr.name="n" attr.type="int"/>
  <key id="d" for="node" attr.name="d" attr.type="string"><default>dflt</default></key>
  <graph id="G" edgedefault="directed">"#;
const TAIL: &str = "</graph></graphml>";

fn import(body: &str) -> Graph {
    let mut g = GraphSource::empty();
    let xml = format!("{HEAD}{body}{TAIL}");
    g.import_graphml_reader(std::io::Cursor::new(xml.into_bytes()))
        .expect("import");
    g
}

#[test]
fn empty_and_self_closing_elements_give_the_empty_string() {
    let g = import(
        r#"<node id="1"><data key="s"></data></node>
           <node id="2"><data key="s"/></node>
           <node id="3"><data key="s">x</data></node>
           <node id="4"></node>"#,
    );
    let lock = g.read();
    for id in ["1", "2"] {
        let v = lock.traversal().v(id).values("s").to_list().unwrap();
        assert_eq!(v, vec![ElementProperty::String("".into())], "vertex {id}");
    }
    let with_key = lock
        .traversal()
        .v(None::<()>)
        .has_key("s")
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(with_key, ElementProperty::from(3));
    let eq_empty = lock
        .traversal()
        .v(None::<()>)
        .has_p("s", P::Eq(""))
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(eq_empty, ElementProperty::from(2));
}

#[test]
fn empty_element_on_an_int_key_stays_absent() {
    let g =
        import(r#"<node id="1"><data key="n"></data></node><node id="2"><data key="n"/></node>"#);
    let lock = g.read();
    let v = lock
        .traversal()
        .v(None::<()>)
        .values("n")
        .to_list()
        .unwrap();
    assert!(v.is_empty());
}

#[test]
fn explicit_empty_string_wins_over_the_default() {
    let g = import(
        r#"<node id="1"><data key="d"></data></node><node id="2"><data key="d"/></node><node id="3"></node>"#,
    );
    let lock = g.read();
    let one = lock.traversal().v("1").values("d").to_list().unwrap();
    let two = lock.traversal().v("2").values("d").to_list().unwrap();
    let three = lock.traversal().v("3").values("d").to_list().unwrap();
    assert_eq!(one, vec![ElementProperty::String("".into())]);
    assert_eq!(two, vec![ElementProperty::String("".into())]);
    assert_eq!(three, vec![ElementProperty::String("dflt".into())]);
}

#[test]
fn empty_string_survives_an_export_import_round_trip() {
    let g = GraphSource::empty();
    g.write()
        .traversal_mut()
        .add_v("t")
        .property("s", "")
        .property("name", "a")
        .next()
        .unwrap();
    let mut buf = Vec::new();
    GraphMLExport::new()
        .export_writer(&*g.read(), &mut buf)
        .unwrap();
    let mut back = GraphSource::empty();
    back.import_graphml_reader(std::io::Cursor::new(buf))
        .unwrap();
    let lock = back.read();
    let v = lock
        .traversal()
        .v(None::<()>)
        .values("s")
        .to_list()
        .unwrap();
    assert_eq!(v, vec![ElementProperty::String("".into())]);
}

#[test]
fn grateful_dead_vertex_7_has_an_empty_song_type() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/tinkerpop/data/grateful-dead.xml"
    );
    let mut g = GraphSource::empty();
    g.import_graphml(path).expect("import grateful");
    let lock = g.read();
    let v = lock
        .traversal()
        .v("7")
        .values("songType")
        .to_list()
        .unwrap();
    assert_eq!(v, vec![ElementProperty::String("".into())]);
}
