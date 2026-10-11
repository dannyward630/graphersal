//! The Rust quick start of the book (`book/src/library/rust.md` includes this file by
//! its `ANCHOR` markers, so the page always shows code that compiles and runs).
//!
//! Run it with `cargo run -p graphersal --example quick_start --features script`.

use std::sync::Arc;

use graphersal::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ANCHOR: read
    // The TinkerPop "modern" sample graph: 4 people, 2 pieces of software, 6 edges.
    let graph = GraphSource::tinkerpop_modern();

    // A read lock gives a traversal source `g`; the traversal is built step by step and run by
    // its terminal (`to_list`, `next`, `iterate`).
    let lock = graph.read();
    let friends = lock
        .traversal()
        .v(None)
        .has("name", "marko")
        .out("knows")
        .values("name")
        .to_list()?;
    println!("marko knows {friends:?}"); // marko knows ["josh", "vadas"]
    drop(lock);
    // ANCHOR_END: read

    // ANCHOR: write
    // Writes take the write lock. Every traversal is one unit: it commits when it succeeds and
    // leaves nothing behind when it fails.
    graph
        .write()
        .traversal_mut()
        .v(None)
        .has("name", "marko")
        .add_e("knows")
        .to_by(__::add_v("person").property("name", "ann"))
        .iterate()?;
    let people = graph
        .read()
        .traversal()
        .v(None)
        .has_label("person")
        .count()
        .next()?;
    println!("people: {people:?}"); // people: Some(5)
    // ANCHOR_END: write

    // ANCHOR: script
    // The same queries as text, in the Gremlin DSL (feature `script`): what the CLI, the
    // playground and Python run. Both spellings work: `has_label` and `hasLabel`.
    let graph = Arc::new(graph);
    let names = graphersal::script::eval_value(
        graph.clone(),
        r#"g.V().has("name", "marko").out("knows").values("name").order().toList()"#,
    )?;
    println!("{names}"); // ["ann", "josh", "vadas"]

    // `eval` renders the result for display: JSON lines, a table with feature `display`.
    let table = graphersal::script::eval(
        graph,
        r#"g.V().hasLabel("software").valueMap("name", "lang")"#,
    )?;
    println!("{table}");
    // ANCHOR_END: script

    #[cfg(feature = "io")]
    {
        let path = std::env::temp_dir().join("graphersal-quick-start.json");
        let path = path.to_str().ok_or("temp path is not UTF-8")?;
        // ANCHOR: files
        let modern = GraphSource::tinkerpop_modern(); // the samples: also file_tree(), large_110k()
        modern.read().traversal().export_graphson(path)?; // GraphSON 3.0, TinkerPop's format
        let copy = GraphSource::from_graphson(path)?; // also from_graphml, from_file
        let two = copy
            .read()
            .traversal()
            .v(vec!["1", "2"])
            .values("name")
            .to_list()?;
        println!("{two:?}"); // ["marko", "vadas"]
        // ANCHOR_END: files
    }
    Ok(())
}
