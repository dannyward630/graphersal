//! The execution's random number generator and the `random.seed` option: `coin()`, `sample()`
//! and `order().by(Order.shuffle)` draw from one generator per execution (child traversals
//! included), so a seeded query is reproducible; without a seed only sizes and membership are
//! checked, never exact draws.
#![cfg(feature = "script")]

use std::sync::Arc;

use graphersal::prelude::Order;
use graphersal::prelude::*;

fn run(script: &str) -> String {
    let value = crate::script_host::eval_value(Arc::new(GraphSource::tinkerpop_modern()), script)
        .unwrap_or_else(|e| panic!("`{script}` failed: {e}"));
    format!("{value:?}")
}

const RANDOM_QUERIES: &[&str] = &[
    r#"g.with("random.seed", SEED).V().coin(0.5).id().to_list()"#,
    r#"g.with("random.seed", SEED).V().sample(3).id().to_list()"#,
    r#"g.with("random.seed", SEED).E().sample(2).by("weight").id().to_list()"#,
    r#"g.with("random.seed", SEED).V().values("name").fold().sample(Scope.local, 3).to_list()"#,
    r#"g.with("random.seed", SEED).V().values("name").order().by(Order.shuffle).to_list()"#,
    // Child traversals share the execution's generator.
    r#"g.with("random.seed", SEED).V().local(__.both().sample(1)).id().to_list()"#,
    r#"g.with("random.seed", SEED).V().group().by(T.label).by(__.both().coin(0.5).id().fold()).to_list()"#,
];

#[test]
fn a_seeded_query_draws_the_same_on_every_run() {
    for template in RANDOM_QUERIES {
        let query = template.replace("SEED", "1234");
        let first = run(&query);
        for _ in 0..3 {
            assert_eq!(run(&query), first, "{query}");
        }
    }
}

#[test]
fn different_seeds_draw_differently() {
    // For every query, at least one of a few seeds differs from seed 0 (a fixed set of seeds, so
    // the test is deterministic).
    for template in RANDOM_QUERIES {
        let base = run(&template.replace("SEED", "0"));
        assert!(
            (1..16).any(|seed| run(&template.replace("SEED", &seed.to_string())) != base),
            "{template}"
        );
    }
}

#[test]
fn without_a_seed_only_sizes_are_fixed() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();
    for _ in 0..10 {
        let picked = lock.traversal().v(None).sample(2).id().to_list().unwrap();
        assert_eq!(picked.len(), 2);
        assert_ne!(picked[0], picked[1]);
        let shuffled = lock
            .traversal()
            .v(None)
            .values("name")
            .order()
            .by(Order::Shuffle)
            .to_list()
            .unwrap();
        let mut sorted = shuffled.clone();
        sorted.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        assert_eq!(sorted.len(), 6);
        assert_eq!(sorted[0], ElementProperty::from("josh"));
    }
}

#[test]
fn a_negative_seed_is_a_seed_and_a_float_is_rejected() {
    let query = r#"g.with("random.seed", -7).V().sample(2).id().to_list()"#;
    assert_eq!(run(query), run(query));
    let err = crate::script_host::eval_value(
        Arc::new(GraphSource::tinkerpop_modern()),
        r#"g.with("random.seed", 1.5).V().sample(2).to_list()"#,
    )
    .expect_err("a float seed");
    assert!(
        err.to_string().contains("expected an integer seed"),
        "{err}"
    );
}

/// `Order.shuffle` shuffles first and then sorts by the clauses after it, as TinkerPop's
/// `MultiComparator` does.
#[test]
fn a_clause_after_the_shuffle_still_sorts() {
    assert_eq!(
        run(
            r#"g.with("random.seed", 3).V().hasLabel("person").order().by(Order.shuffle).by("age").values("age").to_list()"#
        ),
        "[27, 29, 32, 35]"
    );
    assert_eq!(
        run(
            r#"g.with("random.seed", 3).V().values("age").fold().order(Scope.local).by(Order.shuffle).by(Order.desc).to_list()"#
        ),
        "[[35, 32, 29, 27]]"
    );
}

/// The same seed in the Rust API and in the DSL draws the same values.
#[test]
fn rust_and_dsl_draw_alike() {
    let graph = GraphSource::tinkerpop_modern();
    let rust = {
        let lock = graph.read();
        lock.traversal()
            .with("random.seed", 99)
            .v(None)
            .sample(3)
            .id()
            .to_list()
            .unwrap()
    };
    let dsl = crate::script_host::eval_value(
        Arc::new(GraphSource::tinkerpop_modern()),
        r#"g.with("random.seed", 99).V().sample(3).id().to_list()"#,
    )
    .unwrap();
    let dsl: Vec<String> = dsl
        .into_array()
        .unwrap()
        .into_iter()
        .map(|v| v.into_string().unwrap())
        .collect();
    let rust: Vec<String> = rust
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(rust, dsl);
}
