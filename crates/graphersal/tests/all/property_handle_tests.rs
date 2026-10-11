//! Property elements (`properties()`) have an identity and a `toString` form; lazily read values
//! (`values()`) stay plain values: `dedup()`, `as_string()` and set side effects tell them apart.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn modern(script: &str) -> rhai::Dynamic {
    eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn josh_graph(script: &str) -> rhai::Dynamic {
    let setup = r#"g.addV("p").property("name", "josh").next();
        g.addV("p").property("name", "josh").next();
        g.addV("p").property("name", "josh").next();
        "#;
    eval_value(Arc::new(GraphSource::empty()), &format!("{setup}{script}"))
        .unwrap_or_else(|e| panic!("{script}: {e}"))
}

fn strings(script: &str) -> Vec<String> {
    modern(script)
        .into_array()
        .unwrap_or_else(|_| panic!("{script}: not a list"))
        .into_iter()
        .map(|v| v.to_string())
        .collect()
}

fn int(d: rhai::Dynamic) -> i64 {
    d.as_int().expect("an integer")
}

#[test]
fn dedup_keys_a_property_element_by_identity() {
    for q in [
        r#"g.V().properties("name").dedup().count().next()"#,
        r#"g.v().properties("name").dedup().count().next()"#,
    ] {
        assert_eq!(int(josh_graph(q)), 3, "{q}");
    }
    assert_eq!(
        int(modern(
            r#"g.V().both().properties().dedup().count().next()"#
        )),
        12
    );
}

#[test]
fn dedup_keys_an_edge_property_element_by_key_and_value() {
    assert_eq!(
        int(modern(
            r#"g.E().properties("weight").dedup().count().next()"#
        )),
        4
    );
    assert_eq!(
        int(modern(
            r#"g.V().bothE().properties().dedup().count().next()"#
        )),
        4
    );
}

#[test]
fn dedup_stays_by_value_for_plain_values() {
    assert_eq!(
        int(josh_graph(r#"g.V().values("name").dedup().count().next()"#)),
        1
    );
    assert_eq!(
        int(josh_graph(
            r#"g.V().properties("name").value().dedup().count().next()"#
        )),
        1
    );
    assert_eq!(
        int(modern(
            r#"g.V().both().properties().value().dedup().count().next()"#
        )),
        11
    );
    assert_eq!(
        int(modern(r#"g.V().both().values().dedup().count().next()"#)),
        11
    );
}

#[test]
fn as_string_renders_elements_in_tinkerpop_form() {
    for q in [
        r#"g.V().asString().toList()"#,
        r#"g.V().as_string().to_list()"#,
    ] {
        assert_eq!(
            strings(q),
            ["v[1]", "v[2]", "v[3]", "v[4]", "v[5]", "v[6]"],
            "{q}"
        );
    }
    assert_eq!(
        strings(r#"g.V().properties("name").asString().toList()"#),
        [
            "vp[name->marko]",
            "vp[name->vadas]",
            "vp[name->lop]",
            "vp[name->josh]",
            "vp[name->ripple]",
            "vp[name->peter]"
        ]
    );
    // An integer property prints as `29`, not `29.0`.
    assert_eq!(
        strings(r#"g.V(1).properties("age").as_string().to_list()"#),
        ["vp[age->29]"]
    );
    assert_eq!(
        strings(r#"g.E().properties("weight").asString().toList()"#)[0],
        "p[weight->0.5]"
    );
}

#[test]
fn as_string_renders_an_edge_from_its_ids() {
    let edge = strings(r#"g.E().limit(1).asString().toList()"#);
    let id = strings(r#"g.E().limit(1).id().toList()"#);
    let out = strings(r#"g.E().limit(1).outV().id().toList()"#);
    let label = strings(r#"g.E().limit(1).label().toList()"#);
    let into = strings(r#"g.E().limit(1).inV().id().toList()"#);
    assert_eq!(
        edge,
        [format!(
            "e[{}][{}-{}->{}]",
            id[0], out[0], label[0], into[0]
        )]
    );
}

#[test]
fn as_string_of_plain_values_is_unchanged() {
    assert_eq!(
        strings(r#"g.V().values("name").asString().toList()"#)[0],
        "marko"
    );
    assert_eq!(strings(r#"g.V().id().asString().toList()"#)[0], "1");
    assert_eq!(
        strings(r#"g.V().properties("name").value().asString().toList()"#)[0],
        "marko"
    );
}

#[test]
fn as_string_local_applies_the_rule_per_element() {
    assert_eq!(
        strings(r#"g.V().limit(2).fold().asString(Scope.local).unfold().toList()"#),
        ["v[1]", "v[2]"]
    );
    assert_eq!(
        strings(r#"g.V(1).properties().fold().as_string(Scope.local).unfold().toList()"#),
        ["vp[name->marko]", "vp[age->29]"]
    );
}

#[test]
fn a_set_keeps_property_elements_by_identity_and_values_by_value() {
    let q = r#"g.withSideEffect("a", Set.of()).V().properties("name").aggregate("a").cap("a").next().len()"#;
    assert_eq!(int(josh_graph(q)), 3);
    let q = r#"g.withSideEffect("a", Set.of()).V().values("name").aggregate("a").cap("a").next().len()"#;
    assert_eq!(int(josh_graph(q)), 1);
}

#[test]
fn where_compares_against_a_side_effect_scalar_and_a_bucket() {
    let mut kept =
        strings(r#"g.withSideEffect("a", "2").V("1").out().id().where(P.neq("a")).toList()"#);
    kept.sort();
    assert_eq!(kept, ["3", "4"]);
    assert_eq!(
        strings(r#"g.withSideEffect("a", "2").V("1").out().id().where(P.eq("a")).toList()"#),
        ["2"]
    );
    // A list bucket compares as a whole value for `eq`/`neq` (TinkerPop), `within` is membership.
    assert_eq!(
        int(modern(
            r#"g.V("1").out().id().aggregate("x").where(P.eq("x")).count().next()"#
        )),
        0
    );
    assert_eq!(
        int(modern(
            r#"g.V("1").out().id().aggregate("x").where(P.neq("x")).count().next()"#
        )),
        3
    );
    assert_eq!(
        int(modern(
            r#"g.V("1").out().id().aggregate("x").where(P.within("x")).count().next()"#
        )),
        3
    );
    // A path label still wins over a side effect of the same name.
    assert_eq!(
        strings(
            r#"g.withSideEffect("a", "zzz").V().values("name").as("a").where(P.eq("a")).toList()"#
        )
        .len(),
        6
    );
    // The literal fallback of an unknown name stays.
    assert_eq!(
        strings(r#"g.V().values("name").where("marko").toList()"#),
        ["marko"]
    );
}
