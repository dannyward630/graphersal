//! Integration tests for error handling, cause chains, diagnostic advice, and panic resistance.

#[cfg(any(feature = "io", feature = "script"))]
use graphersal::prelude::*;

#[cfg(feature = "io")]
#[test]
fn test_graphml_parser_malformed_input_resilience() {
    use graphersal::io::GraphImport;
    use std::io::Cursor;

    let malformed_xmls = vec![
        r#"<graphml><node id="1"><unclosed_tag></node></graphml>"#,
        r#"<graphml><graph><node><data key="name">Alice</data></node></graph></graphml>"#,
        r#"<graphml><graph><key id="d0" for="node" attr.name="weight" attr.type="double"/><node id="1"><data key="d0">invalid_float</data></node></graph></graphml>"#,
    ];

    for xml in malformed_xmls {
        let mut graph = GraphSource::empty();
        let res = graph.import_graphml_reader(Cursor::new(xml));
        assert!(res.is_err(), "Expected error for malformed XML: {}", xml);
    }
}

#[cfg(feature = "script")]
#[test]
fn test_script_eval_error_resilience() {
    let graph = std::sync::Arc::new(GraphSource::tinkerpop_modern());
    let eval_res = crate::script_host::eval(graph, "non_existent_function_call()");
    assert!(eval_res.is_err());
}

#[cfg(feature = "script")]
#[test]
fn test_script_numeric_args_resilience() {
    let graph = std::sync::Arc::new(GraphSource::tinkerpop_modern());

    // 1. g.v(1000) integer argument
    let res1 = crate::script_host::eval(graph.clone(), "g.v(1000).to_list()");
    assert!(
        res1.is_ok(),
        "g.v(1000) should evaluate without panic: {:?}",
        res1.err()
    );

    // 2. g.v([1, 2, 3]) array of integers
    let res2 = crate::script_host::eval(graph.clone(), "g.v([1, 2, 3]).to_list()");
    assert!(
        res2.is_ok(),
        "g.v([1, 2, 3]) should evaluate without panic: {:?}",
        res2.err()
    );

    // 3. g.v("1").has_id(1000) integer in step argument
    let res3 = crate::script_host::eval(graph.clone(), "g.v(\"1\").has_id(1000).to_list()");
    assert!(
        res3.is_ok(),
        "has_id(1000) should evaluate without panic: {:?}",
        res3.err()
    );
}

#[cfg(feature = "script")]
#[test]
fn test_script_type_mismatch_errors() {
    let graph = std::sync::Arc::new(GraphSource::tinkerpop_modern());

    // 1. Invalid argument type passed to where_t (integer 123 instead of traversal)
    let res1 = crate::script_host::eval(graph.clone(), "g.v().where_t(123).to_list()");
    assert!(
        res1.is_err(),
        "Expected type mismatch error for where_t(123)"
    );
    let err1_str = res1.err().unwrap().to_string();
    assert!(
        err1_str.contains("Mismatch")
            || err1_str.contains("type")
            || err1_str.contains("data type"),
        "Error message should mention type mismatch: {}",
        err1_str
    );

    // 2. Invalid argument passed to and([123])
    let res2 = crate::script_host::eval(graph.clone(), "g.v().and([123]).to_list()");
    assert!(res2.is_err(), "Expected type mismatch error for and([123])");
}

#[cfg(feature = "script")]
#[test]
fn test_step_failure_includes_step_arguments() {
    let graph = std::sync::Arc::new(GraphSource::tinkerpop_modern());

    let err = crate::script_host::eval(graph.clone(), "g.v().range(1, 0).to_list()")
        .expect_err("a failing traversal must fail the script");
    let output = err.to_string();
    assert!(
        output.contains("range(1, 0)"),
        "Expected step name in error output to include argument details like range(1, 0), got: {}",
        output
    );
}
