//! Saved queries in the Rhai DSL (`book/src/users_guide/saved_queries.md`): defining
//! (`define_query`, `define_queries`), calling (`query`: named arguments, defaults, values and
//! continued traversals), read-only enforcement (direct, nested, continued, returned), the call
//! depth bound, the compiled-body cache across redefinition and rollback, management
//! (`queries`, `get_query`, `drop_query`, `move_query`, `describe_query`) and the errors.
#![cfg(feature = "script")]

use std::sync::Arc;

use graphersal::catalog::{DefinitionKind, QueryDefinition, QueryParam};
use graphersal::prelude::*;

use crate::script_host::{ScriptGraphTraversalSource, ScriptLimits, eval_value, eval_value_atomic};
use graphersal::script::rhai::{self, Dynamic};

fn modern() -> Arc<Graph> {
    Arc::new(GraphSource::tinkerpop_modern())
}

fn run(graph: &Arc<Graph>, script: &str) -> Result<Dynamic, String> {
    eval_value(graph.clone(), script).map_err(|err| err.to_string())
}

fn ok(graph: &Arc<Graph>, script: &str) -> Dynamic {
    run(graph, script).unwrap_or_else(|err| panic!("script failed: {script}\n{err}"))
}

fn err(graph: &Arc<Graph>, script: &str) -> String {
    match run(graph, script) {
        Ok(value) => panic!("script must fail: {script}\ngot {value:?}"),
        Err(err) => err,
    }
}

fn assert_contains(text: &str, part: &str) {
    assert!(text.contains(part), "expected `{part}` in:\n{text}");
}

fn text(value: Dynamic) -> String {
    value.to_string()
}

/// The modern graph with `people(min = 0)`: persons older than `min`, in folder `reports/people`.
fn with_people() -> Arc<Graph> {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{
            name: "people",
            folder: "reports/people",
            params: #{min: #{schema: #{type: "integer", minimum: 0, maximum: 200}, default: 0}},
            body: `/// Persons older than min.
                   fn people(min) { g.v().has_label("person").has("age", P.gt(min)) }`
        })"#,
    );
    graph
}

/// Stores a query through the Rust API, which keeps the read-only dialect it is given (a query
/// saved by 0.2.0 or loaded from a catalog file): its writes are denied when it is called.
fn store_read_only(graph: &Arc<Graph>, name: &str, body: &str) {
    graph
        .write()
        .traversal_mut()
        .define_query(name, QueryDefinition::new(body))
        .unwrap();
}

fn query_count(graph: &Arc<Graph>) -> usize {
    graph
        .read()
        .traversal()
        .list_definitions(Some(DefinitionKind::QUERY))
        .unwrap()
        .len()
}

// ---------------------------------------------------------------------------------------------
// Defining and calling
// ---------------------------------------------------------------------------------------------

#[test]
fn a_defined_query_is_called_with_named_arguments_and_defaults() {
    let graph = with_people();
    assert_eq!(
        text(ok(
            &graph,
            r#"g.query("people", #{min: 30}).values("name").order().to_list()"#
        )),
        r#"["josh", "peter"]"#
    );
    // The default (0) is substituted when the argument is left out.
    assert_eq!(text(ok(&graph, r#"g.query("people").count().next()"#)), "4");
    assert_eq!(
        text(ok(&graph, r#"g.query("people", #{}).count().next()"#)),
        "4"
    );
}

#[test]
fn the_camel_case_spellings_work_alike() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.defineQuery(#{name: "names", body: "fn names() { g.v().values(\"name\") }"});
           g.defineQueries("x", "fn one() { 1 }");
           g.moveQuery("names", "y");
           g.describeQuery("names", "All names.");"#,
    );
    let entry = ok(&graph, r#"g.getQuery("names")"#).cast::<rhai::Map>();
    assert_eq!(entry["folder"].to_string(), "y");
    assert_eq!(entry["description"].to_string(), "All names.");
    assert_eq!(text(ok(&graph, r#"g.dropQuery("one")"#)), "true");
    assert_eq!(query_count(&graph), 1);
}

#[test]
fn a_query_may_return_a_value() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "stats", body: `fn stats() {
            #{people: g.v().has_label("person").count().next(),
              names: g.v().values("name").order().limit(2).to_list()}
        }`})"#,
    );
    let value = ok(&graph, r#"g.query("stats")"#).cast::<rhai::Map>();
    assert_eq!(value["people"].as_int().unwrap(), 4);
    assert_eq!(value["names"].to_string(), r#"["josh", "lop"]"#);
}

#[test]
fn a_returned_traversal_is_continued_and_optimized_with_the_caller_steps() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "everything", body: "fn everything() { g.v() }"})"#,
    );
    assert_eq!(
        text(ok(
            &graph,
            r#"g.query("everything").has_label("person").count().next()"#
        )),
        "4"
    );
    // The caller's filter is pushed into the body's v(): one fused source step.
    let profile = text(ok(
        &graph,
        r#"g.query("everything").has_label("person").count().profile().to_string()"#,
    ));
    assert_contains(&profile, r#"v(labels: ["person"]).count()"#);
    assert_contains(&profile, "source_filter_pushdown");
}

#[test]
fn query_works_in_loops_functions_and_bodies() {
    let graph = with_people();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "older_people", params: #{min: "integer"},
            body: "fn older_people(min) { g.query(\"people\", #{min: min}).values(\"name\") }"})"#,
    );
    assert_eq!(
        text(ok(
            &graph,
            r#"let counts = [];
               for min in [0, 30] { counts.push(g.query("people", #{min: min}).count().next()); }
               counts"#
        )),
        "[4, 2]"
    );
    assert_eq!(
        text(ok(
            &graph,
            r#"g.query("older_people", #{min: 31}).order().to_list()"#
        )),
        r#"["josh", "peter"]"#
    );
}

#[test]
fn an_int64_argument_is_accepted_for_a_number_parameter() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "half", params: #{x: "number"}, body: "fn half(x) { x / 2.0 }"})"#,
    );
    assert_eq!(text(ok(&graph, r#"g.query("half", #{x: 3})"#)), "1.5");
    assert_eq!(text(ok(&graph, r#"g.query("half", #{x: 3.0})"#)), "1.5");
}

#[test]
fn a_uuid_parameter_takes_a_uuid_value() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "echo", params: #{id: "uuid"}, body: "fn echo(id) { id }"})"#,
    );
    let echoed = ok(
        &graph,
        r#"g.query("echo", #{id: UUID.from_string("123e4567-e89b-12d3-a456-426614174000")}).to_string()"#,
    );
    assert_eq!(echoed.to_string(), "123e4567-e89b-12d3-a456-426614174000");
}

#[test]
fn defining_again_replaces_the_query() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "n", body: "fn n() { 1 }"})"#,
    );
    assert_eq!(text(ok(&graph, r#"g.query("n")"#)), "1");
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "n", body: "fn n() { 2 }"})"#,
    );
    assert_eq!(text(ok(&graph, r#"g.query("n")"#)), "2");
    assert_eq!(query_count(&graph), 1);
}

#[test]
fn the_description_comes_from_the_doc_comment_unless_given() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "a", body: "/// From the comment.\nfn a() { 1 }"});
           g.define_query(#{name: "b", description: "Given.", body: "/// Comment.\nfn b() { 1 }"})"#,
    );
    assert_eq!(
        text(ok(&graph, r#"g.get_query("a")["description"]"#)),
        "From the comment."
    );
    assert_eq!(
        text(ok(&graph, r#"g.get_query("b")["description"]"#)),
        "Given."
    );
}

#[test]
fn define_queries_stores_every_function_with_its_doc_comment() {
    let graph = modern();
    let names = ok(
        &graph,
        r#"g.define_queries("reports/sales", `
            /// Persons, oldest first.
            /// At most n of them.
            fn oldest(n) { g.v().has_label("person").order().by("age", Order.desc).limit(n) }

            /** Software names. */
            fn software() { g.v().has_label("software").values("name") }
        `, #{oldest: #{n: #{schema: #{type: "integer", minimum: 1}, default: 2}}})"#,
    );
    assert_eq!(names.to_string(), r#"["oldest", "software"]"#);
    let oldest = ok(&graph, r#"g.get_query("oldest")"#).cast::<rhai::Map>();
    assert_eq!(oldest["folder"].to_string(), "reports/sales");
    assert_eq!(
        oldest["description"].to_string(),
        "Persons, oldest first.\nAt most n of them."
    );
    // Each query stores the text of its own function only.
    let body = oldest["body"].to_string();
    assert!(body.starts_with("fn oldest(n) {"), "{body}");
    assert!(!body.contains("software"), "{body}");
    assert_eq!(
        text(ok(&graph, r#"g.query("oldest").values("name").to_list()"#)),
        r#"["peter", "josh"]"#
    );
    assert_eq!(
        text(ok(&graph, r#"g.query("software").order().to_list()"#)),
        r#"["lop", "ripple"]"#
    );
    assert_eq!(
        text(ok(&graph, r#"g.get_query("software")["description"]"#)),
        "Software names."
    );
}

#[test]
fn define_queries_stores_all_or_nothing() {
    let graph = modern();
    let message = err(
        &graph,
        r#"g.define_queries("", `fn good() { 1 } fn bad(x) { x }`)"#,
    );
    assert_contains(&message, "Invalid body of saved query 'bad'");
    assert_contains(&message, "'x' is not declared in params");
    assert_eq!(query_count(&graph), 0);
    // A catalog rule broken by one query (a name longer than 128 bytes) stores none either:
    // the queries are stored in one unit, rolled back as a whole.
    let message = err(
        &graph,
        "g.define_queries(\"\", `fn good() { 1 } fn qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx() { 2 }`)",
    );
    assert_contains(&message, "Invalid query name");
    assert_eq!(query_count(&graph), 0);
}

// ---------------------------------------------------------------------------------------------
// Checks at definition
// ---------------------------------------------------------------------------------------------

#[test]
fn a_body_must_be_exactly_one_function_named_like_the_query() {
    let graph = modern();
    let cases = [
        (
            r#"#{name: "q", body: "fn q( { 1 }"}"#,
            "it does not compile",
        ),
        (
            r#"#{name: "q", body: "fn q() { 1 } fn r() { 2 }"}"#,
            "it defines 2 functions",
        ),
        (
            r#"#{name: "q", body: "fn other() { 1 }"}"#,
            "it defines the function 'other', but the query is named 'q'",
        ),
        (
            r#"#{name: "q", body: "let x = 1; fn q() { x }"}"#,
            "statements outside the function",
        ),
        (r#"#{name: "q", body: "1 + 1"}"#, "statements outside"),
        (
            r#"#{name: "q", body: "fn q(a) { a }"}"#,
            "'a' is not declared",
        ),
        (
            r#"#{name: "q", params: #{b: "integer"}, body: "fn q() { 1 }"}"#,
            "params declares 'b', which the function does not take",
        ),
    ];
    for (spec, expected) in cases {
        let message = err(&graph, &format!("g.define_query({spec})"));
        assert_contains(&message, "Invalid body of saved query 'q'");
        assert_contains(&message, expected);
        assert_contains(
            &message,
            "Help: The body of a saved query is the text of ONE",
        );
    }
    assert_eq!(query_count(&graph), 0);
}

#[test]
fn the_catalog_rules_apply_to_a_dsl_definition() {
    let graph = modern();
    // A parameter default that violates its schema.
    let message = err(
        &graph,
        r#"g.define_query(#{name: "q", params: #{n: #{schema: #{type: "integer", minimum: 1}, default: 0}}, body: "fn q(n) { n }"})"#,
    );
    assert_contains(&message, "Invalid parameter 'n'");
    // A parameter named like a DSL global is refused by the parser already (it is a Rhai
    // function parameter), a reserved query name by the catalog.
    let message = err(
        &graph,
        r#"g.define_query(#{name: "q", folder: "/a", body: "fn q() { 1 }"})"#,
    );
    assert_contains(&message, "Invalid folder '/a'");
    let message = err(
        &graph,
        r#"g.define_query(#{name: "q", body: "fn q() { 1 }", colour: "red"})"#,
    );
    assert_contains(&message, "the key 'colour'");
    let message = err(
        &graph,
        r#"g.define_query(#{name: "loop", body: "fn q() { 1 }"})"#,
    );
    assert_contains(&message, "Invalid query name 'loop': it is a reserved word");
    let message = err(&graph, r#"g.define_query(#{body: "fn q() { 1 }"})"#);
    assert_contains(&message, "no name");
    assert_eq!(query_count(&graph), 0);
}

#[test]
fn the_short_parameter_form_declares_a_bare_type() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "q", params: #{n: "integer", s: "string"}, body: "fn q(s, n) { s + n }"})"#,
    );
    assert_eq!(text(ok(&graph, r#"g.query("q", #{n: 1, s: "a"})"#)), "a1");
    // The parameters are listed in signature order, not in the (sorted) map order.
    assert_eq!(
        text(ok(&graph, r#"g.get_query("q")["params"].map(|p| p.name)"#)),
        r#"["s", "n"]"#
    );
}

// ---------------------------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------------------------

#[test]
fn arguments_are_checked_against_the_parameters() {
    let graph = with_people();
    let unknown = err(&graph, r#"g.query("people", #{minimum: 3})"#);
    assert_contains(&unknown, "Saved query 'people' has no parameter 'minimum'");
    assert_contains(&unknown, "parameters of 'people': min");

    let wrong_type = err(&graph, r#"g.query("people", #{min: "30"})"#);
    assert_contains(
        &wrong_type,
        "Invalid argument 'min' for saved query 'people'",
    );
    assert_contains(&wrong_type, "expected int64, got string");

    let constraint = err(&graph, r#"g.query("people", #{min: 5000})"#);
    assert_contains(&constraint, "Invalid argument 'min'");
    assert_contains(&constraint, "200");

    let traversal = err(&graph, r#"g.query("people", #{min: g.v().count()})"#);
    assert_contains(&traversal, "is not a data value");

    let not_a_map = err(&graph, r#"g.query("people", 30)"#);
    assert_contains(&not_a_map, "named arguments as a map");
}

#[test]
fn a_required_parameter_must_be_passed() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "q", params: #{n: "integer"}, body: "fn q(n) { n }"})"#,
    );
    let message = err(&graph, r#"g.query("q")"#);
    assert_contains(
        &message,
        "Saved query 'q' needs the argument 'n' (int64), it has no default",
    );
    assert_contains(&message, r#"g.query("q", #{n: ..})"#);
    // `()` is a value, not an omitted argument.
    let message = err(&graph, r#"g.query("q", #{n: ()})"#);
    assert_contains(&message, "Invalid argument 'n'");
}

// ---------------------------------------------------------------------------------------------
// Read-only
// ---------------------------------------------------------------------------------------------

#[test]
fn a_query_cannot_write_directly() {
    let graph = modern();
    store_read_only(&graph, "writer", r#"fn writer() { g.add_v("x").next() }"#);
    let message = err(&graph, r#"g.query("writer")"#);
    assert_contains(&message, "Permission denied for 'add_v'");
    assert_contains(&message, "a saved query is read-only");
    assert_contains(&message, r#"in saved query: "writer""#);
    assert_contains(&message, "Help: A read-only saved query");
    assert_eq!(graph.read().vertex_count(), 6);
}

#[test]
fn a_query_cannot_write_the_schema_or_the_catalog() {
    let graph = modern();
    store_read_only(
        &graph,
        "schema",
        "fn schema() { g.set_schema(g.infer_schema()) }",
    );
    store_read_only(
        &graph,
        "definer",
        r#"fn definer() { g.drop_query("schema") }"#,
    );
    assert_contains(
        &err(&graph, r#"g.query("schema")"#),
        "a saved query is read-only",
    );
    assert_contains(
        &err(&graph, r#"g.query("definer")"#),
        "a saved query is read-only",
    );
    assert!(graph.read().get_schema().is_none());
    assert_eq!(query_count(&graph), 2);
}

#[test]
fn a_nested_query_cannot_write_either() {
    let graph = modern();
    store_read_only(
        &graph,
        "inner",
        r#"fn inner() { g.v().property("k", 1).to_list() }"#,
    );
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "outer", body: "fn outer() { g.query(\"inner\") }"});"#,
    );
    let message = err(&graph, r#"g.query("outer")"#);
    assert_contains(&message, "a saved query is read-only");
    // One context line per call, innermost first.
    let inner = message
        .find(r#"in saved query: "inner""#)
        .expect("inner named");
    let outer = message
        .find(r#"in saved query: "outer""#)
        .expect("outer named");
    assert!(inner < outer, "{message}");
    assert_eq!(text(ok(&graph, r#"g.v().has("k").count().next()"#)), "0");
}

#[test]
fn a_returned_traversal_cannot_be_continued_with_a_writing_step() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "people", body: "fn people() { g.v().has_label(\"person\") }"})"#,
    );
    let message = err(
        &graph,
        r#"g.query("people").property("seen", true).to_list()"#,
    );
    assert_contains(&message, "Permission denied for 'property'");
    assert_contains(&message, "a saved query is read-only");
    let message = err(&graph, r#"g.query("people").drop().to_list()"#);
    assert_contains(&message, "a saved query is read-only");
    assert_eq!(text(ok(&graph, r#"g.v().has("seen").count().next()"#)), "0");
    // The caller writes in a traversal of its own.
    let _ = ok(
        &graph,
        r#"let ids = g.query("people").id().to_list(); g.v(ids).property("seen", true).to_list()"#,
    );
    assert_eq!(text(ok(&graph, r#"g.v().has("seen").count().next()"#)), "4");
}

#[test]
fn nothing_built_from_a_query_result_is_mutating() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "people", body: "fn people() { g.v().has_label(\"person\") }"})"#,
    );
    let continued =
        ok(&graph, r#"g.query("people").property("k", 1)"#).cast::<ScriptGraphTraversalSource>();
    assert!(!continued.is_mutating());
    let plain = ok(&graph, r#"g.v().property("k", 1)"#).cast::<ScriptGraphTraversalSource>();
    assert!(plain.is_mutating());
}

#[test]
fn a_query_cannot_return_a_writing_traversal() {
    let graph = modern();
    // Read-only queries (a query that writes runs a returned writing traversal of the graph,
    // `saved_query_effects_tests.rs`).
    store_read_only(
        &graph,
        "sneaky",
        r#"fn sneaky() { [1, g.v().property("k", 1)] }"#,
    );
    store_read_only(&graph, "anon", r#"fn anon() { __.add_v("x") }"#);
    let message = err(&graph, r#"g.query("sneaky")"#);
    assert_contains(
        &message,
        "Saved query 'sneaky' returned a traversal with a writing step (result[1])",
    );
    let message = err(
        &graph,
        r#"g.inject(1).side_effect(g.query("anon")).to_list()"#,
    );
    assert_contains(&message, "returned a traversal with a writing step");
    assert_eq!(graph.read().vertex_count(), 6);
}

#[test]
fn a_query_may_write_a_graph_it_builds_itself() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "scratch", body: `fn scratch() {
            let s = GraphSource::empty();
            s.add_v("x").next();
            s.v().count().next()
        }`})"#,
    );
    assert_eq!(text(ok(&graph, r#"g.query("scratch")"#)), "1");
    assert_eq!(graph.read().vertex_count(), 6);
}

// ---------------------------------------------------------------------------------------------
// Position, depth, missing queries, errors
// ---------------------------------------------------------------------------------------------

#[test]
fn query_only_starts_a_traversal() {
    let graph = with_people();
    let message = err(&graph, r#"g.v().query("people")"#);
    assert_contains(
        &message,
        r#"query("people") can only start a traversal, it cannot follow other steps"#,
    );
    assert_contains(&message, r#"g.query("people")"#);
    let message = err(&graph, r#"g.v().where(__.query("people")).to_list()"#);
    assert_contains(&message, "it cannot follow __ (an anonymous traversal)");
}

#[test]
fn the_call_depth_is_bounded() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "r", params: #{n: "integer"},
            body: "fn r(n) { if n >= 16 { n } else { g.query(\"r\", #{n: n + 1}) } }"})"#,
    );
    // Sixteen levels (n = 1 ..= 16) are allowed, a seventeenth is not. (They fit the 2 MiB
    // stack of a test thread in a debug build.)
    assert_eq!(text(ok(&graph, r#"g.query("r", #{n: 1})"#)), "16");
    let message = err(&graph, r#"g.query("r", #{n: 0})"#);
    assert_contains(
        &message,
        "Saved query calls nested deeper than 16 levels (calling 'r')",
    );
    assert_contains(
        &message,
        "Help: Saved queries may call each other at most 16",
    );
    // The repeated stack lines of the recursion are counted, not printed 16 times.
    assert_eq!(
        message.matches(r#"in saved query: "r""#).count(),
        1,
        "{message}"
    );
    assert_contains(&message, "(×");
}

#[test]
fn a_missing_query_fails_with_its_name() {
    let graph = modern();
    let message = err(&graph, r#"g.query("nope")"#);
    assert_contains(&message, "There is no query named 'nope'");
    assert_contains(&message, "g.queries()");
    // A missing callee fails at call time, located in the caller.
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "caller", body: "fn caller() { g.query(\"gone\") }"})"#,
    );
    let message = err(&graph, r#"g.query("caller")"#);
    assert_contains(&message, "There is no query named 'gone'");
    assert_contains(&message, r#"in saved query: "caller" (line 1, position"#);
}

#[test]
fn an_error_in_the_body_names_the_query() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "broken", body: "fn broken() {\n  g.v().values(\"name\").as_number(GType.LONG).to_list()\n}"});
           g.define_query(#{name: "thrower", body: "fn thrower() { throw \"custom\" }"});
           g.define_query(#{name: "unknown_var", body: "fn unknown_var() { missing + 1 }"});"#,
    );
    let message = err(&graph, r#"g.query("broken")"#);
    assert_contains(&message, "as_number");
    assert_contains(&message, r#"in saved query: "broken" (line 2"#);
    let message = err(&graph, r#"g.query("thrower")"#);
    assert_contains(&message, "custom");
    assert_contains(&message, r#"in saved query: "thrower""#);
    let message = err(&graph, r#"g.query("unknown_var")"#);
    assert_contains(&message, "missing");
    assert_contains(&message, r#"in saved query: "unknown_var""#);
}

#[test]
fn a_stored_body_that_no_longer_compiles_is_listed_with_its_error() {
    let graph = modern();
    // Stored through the Rust API, which does not compile bodies.
    graph
        .write()
        .traversal_mut()
        .define_query(
            "stale",
            QueryDefinition::new("fn stale(x) { x + }")
                .with_param("x", QueryParam::from_value("x", &"integer".into()).unwrap()),
        )
        .unwrap();
    graph
        .write()
        .traversal_mut()
        .define_query("fine", QueryDefinition::new("fn fine() { 1 }"))
        .unwrap();
    let listed = ok(&graph, r#"g.queries()"#).cast::<rhai::Array>();
    let stale = listed[1].clone().cast::<rhai::Map>();
    assert_eq!(stale["name"].to_string(), "stale");
    assert_eq!(stale["status"].to_string(), "error");
    assert_contains(&stale["error"].to_string(), "it does not compile");
    let message = err(&graph, r#"g.query("stale", #{x: 1})"#);
    assert_contains(&message, "Invalid body of saved query 'stale'");
    let fine = listed
        .iter()
        .map(|entry| entry.clone().cast::<rhai::Map>())
        .find(|entry| entry["name"].to_string() == "fine")
        .expect("listed");
    assert_eq!(fine["status"].to_string(), "ok");
    assert!(!fine.contains_key("error"));
}

// ---------------------------------------------------------------------------------------------
// The compiled-body cache
// ---------------------------------------------------------------------------------------------

#[test]
fn a_rolled_back_redefinition_restores_the_old_body() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "v", body: "fn v() { 1 }"})"#,
    );
    assert_eq!(text(ok(&graph, r#"g.query("v")"#)), "1");
    // A whole-script unit redefines the query, calls the new body, then fails: rolled back.
    let failed = eval_value_atomic(
        graph.clone(),
        r#"g.define_query(#{name: "v", body: "fn v() { 2 }"});
           if g.query("v") != 2 { throw "the new body did not run" }
           throw "roll back""#,
        Default::default(),
        &ScriptLimits::default(),
    );
    assert!(failed.unwrap_err().to_string().contains("roll back"));
    // The old body runs again (the cache is keyed by the stored text).
    assert_eq!(text(ok(&graph, r#"g.query("v")"#)), "1");
    // A transaction of the Rust API that is rolled back too.
    struct Abort;
    impl From<graphersal::error::GraphError> for Abort {
        fn from(_: graphersal::error::GraphError) -> Self {
            Abort
        }
    }
    let result: Result<(), Abort> = graph.write().transaction(|unit| {
        unit.traversal_mut()
            .define_query("v", QueryDefinition::new("fn v() { 3 }"))
            .map_err(|_| Abort)?;
        assert_eq!(unit.traversal().list_definitions(None).unwrap().len(), 1);
        Err(Abort)
    });
    assert!(result.is_err());
    assert_eq!(text(ok(&graph, r#"g.query("v")"#)), "1");
}

// ---------------------------------------------------------------------------------------------
// Management
// ---------------------------------------------------------------------------------------------

#[test]
fn queries_lists_by_name_and_filters_by_folder() {
    let graph = modern();
    let _ = ok(
        &graph,
        r#"g.define_query(#{name: "b", folder: "reports/sales", body: "fn b() { 1 }"});
           g.define_query(#{name: "a", folder: "reports", body: "fn a() { 1 }"});
           g.define_query(#{name: "c", folder: "reportsx", body: "fn c() { 1 }"});
           g.define_query(#{name: "d", body: "fn d() { 1 }"});"#,
    );
    let names = |script: &str| text(ok(&graph, &format!("{script}.map(|q| q.name)")));
    assert_eq!(names("g.queries()"), r#"["a", "b", "c", "d"]"#);
    assert_eq!(names(r#"g.queries("reports")"#), r#"["a", "b"]"#);
    assert_eq!(names(r#"g.queries("reports/")"#), r#"["a", "b"]"#);
    assert_eq!(names(r#"g.queries("reports/sales")"#), r#"["b"]"#);
    let entry = ok(&graph, r#"g.queries()[3]"#).cast::<rhai::Map>();
    assert!(entry["folder"].is_unit());
    assert!(entry["description"].is_unit());
    assert_eq!(entry["status"].to_string(), "ok");
    assert_eq!(entry["params"].to_string(), "[]");
    assert!(!entry.contains_key("body"));
}

#[test]
fn get_query_returns_the_full_definition() {
    let graph = with_people();
    let entry = ok(&graph, r#"g.get_query("people")"#).cast::<rhai::Map>();
    assert_eq!(entry["name"].to_string(), "people");
    assert_eq!(entry["folder"].to_string(), "reports/people");
    assert_eq!(entry["description"].to_string(), "Persons older than min.");
    assert_eq!(entry["dialect"].to_string(), "graphersal-rhai/1");
    assert_contains(&entry["body"].to_string(), "fn people(min)");
    let params = entry["params"].clone().cast::<rhai::Array>();
    let min = params[0].clone().cast::<rhai::Map>();
    assert_eq!(min["name"].to_string(), "min");
    assert_eq!(min["default"].as_int().unwrap(), 0);
    assert!(!min["required"].as_bool().unwrap());
    assert!(ok(&graph, r#"g.get_query("nope")"#).is_unit());
}

#[test]
fn drop_move_and_describe_change_the_catalog() {
    let graph = with_people();
    let _ = ok(
        &graph,
        r#"g.move_query("people", "archive"); g.describe_query("people", "Old.")"#,
    );
    let entry = ok(&graph, r#"g.get_query("people")"#).cast::<rhai::Map>();
    assert_eq!(entry["folder"].to_string(), "archive");
    assert_eq!(entry["description"].to_string(), "Old.");
    // Moving does not break a caller: the name stays.
    assert_eq!(text(ok(&graph, r#"g.query("people").count().next()"#)), "4");
    let _ = ok(
        &graph,
        r#"g.move_query("people", ()); g.describe_query("people", "")"#,
    );
    let entry = ok(&graph, r#"g.get_query("people")"#).cast::<rhai::Map>();
    assert!(entry["folder"].is_unit());
    assert!(entry["description"].is_unit());
    assert_contains(
        &err(&graph, r#"g.move_query("nope", "x")"#),
        "There is no query named 'nope'",
    );
    assert_eq!(text(ok(&graph, r#"g.drop_query("people")"#)), "true");
    assert_eq!(text(ok(&graph, r#"g.drop_query("people")"#)), "false");
    assert_eq!(query_count(&graph), 0);
}

#[test]
fn catalog_changes_are_reported_by_execute() {
    let graph = modern();
    let changes = ok(&graph, r#"g.v().execute().changes"#).cast::<rhai::Map>();
    assert!(!changes["definitions"].as_bool().unwrap());
    assert!(!changes["data"].as_bool().unwrap());
}

// ---------------------------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------------------------

/// A query defined through the DSL on a Store survives a reopen (the journal records it) and a
/// rolled-back whole-script definition leaves nothing in it.
#[cfg(feature = "persist")]
#[test]
fn a_query_defined_on_a_store_survives_a_reopen() {
    use graphersal::persist::{MemDir, Store, StoreDir, StoreOptions};

    let dir: Arc<dyn StoreDir> = Arc::new(MemDir::named("saved-query-store"));
    Store::create(&dir, StoreOptions::new())
        .unwrap()
        .close()
        .unwrap();
    {
        let store = Store::open(&dir, StoreOptions::new()).unwrap();
        let graph = store.graph().clone();
        let _ = ok(
            &graph,
            r#"g.define_query(#{name: "answer", params: #{x: #{schema: #{type: "integer"}, default: 41}},
                body: "fn answer(x) { x + 1 }"})"#,
        );
        let failed = eval_value_atomic(
            graph.clone(),
            r#"g.define_query(#{name: "gone", body: "fn gone() { 1 }"}); throw "roll back""#,
            Default::default(),
            &ScriptLimits::default(),
        );
        assert!(failed.is_err());
        store.close().unwrap();
    }
    let store = Store::open(&dir, StoreOptions::new()).unwrap();
    let graph = store.graph().clone();
    assert_eq!(text(ok(&graph, r#"g.query("answer")"#)), "42");
    assert_eq!(
        text(ok(&graph, r#"g.queries().map(|q| q.name)"#)),
        r#"["answer"]"#
    );
    store.close().unwrap();
}
