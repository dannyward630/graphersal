//! Integration tests for optimizer rule visibility and per-rule enable/disable
//! (the 2026-09-24 "Optimizer Rule Visibility in .profile() and Per-Rule Enable/Disable" spec):
//! `.profile()`'s "Optimizer rules applied" line, and `g.with("optimizer.disabled"/"optimizer.enabled", [...])`.
//!
//! The load-bearing check in this file is correctness: disabling any subset of the 7 existing
//! rules must never change a query's *result*, only its *plan* — each `..._disabled_keeps_the_result_correct`
//! test below compares the disabled-rule result against the default (all rules on) baseline.
//! Mirrors `timeout_tests.rs`'s "run with and without the option, compare results" convention.

use graphersal::prelude::*;

/// Sorted `id()`/`values()` results as strings, so result-set comparisons ignore order.
fn sorted_strings(values: Vec<ElementProperty>) -> Vec<String> {
    let mut strings: Vec<String> = values
        .into_iter()
        .map(|v| {
            v.as_string()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{v:?}"))
        })
        .collect();
    strings.sort();
    strings
}

// T1: where_unnest
#[test]
fn where_unnest_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .id()
        .to_list()
        .unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["where_unnest"])
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .id()
        .to_list()
        .unwrap();
    assert_eq!(sorted_strings(baseline), sorted_strings(disabled));

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["where_unnest"])
        .v(None::<()>)
        .where_t(__::has_label("person"))
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    // Disabling `where_unnest` also stops every other rule that depended on it having run first
    // (`has_label` stays nested inside `where_t()`, out of reach of `SourceFilterPushdown`/
    // `CountPushdown`), so no rule fires at all here — nothing to assert beyond its own absence.
    assert!(!text.contains("where_unnest"), "{text}");
}

// T2: filter_reorder
#[test]
fn filter_reorder_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .id()
        .to_list()
        .unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["filter_reorder"])
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(sorted_strings(baseline), sorted_strings(disabled.clone()));
    assert_eq!(sorted_strings(disabled), vec!["1".to_string()]);

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["filter_reorder"])
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    let applied = text
        .lines()
        .find(|l| l.starts_with("Optimizer rules applied:"))
        .unwrap_or("");
    assert!(!applied.contains("filter_reorder"), "{text}");
}

// T3: has_id_pushdown
#[test]
fn has_id_pushdown_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .has_id(vec!["1", "4"])
        .id()
        .to_list()
        .unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["has_id_pushdown"])
        .v(None::<()>)
        .has_id(vec!["1", "4"])
        .id()
        .to_list()
        .unwrap();
    assert_eq!(sorted_strings(baseline), sorted_strings(disabled.clone()));
    assert_eq!(
        sorted_strings(disabled),
        vec!["1".to_string(), "4".to_string()]
    );

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["has_id_pushdown"])
        .v(None::<()>)
        .has_id(vec!["1", "4"])
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    let applied = text
        .lines()
        .find(|l| l.starts_with("Optimizer rules applied:"))
        .unwrap_or("");
    assert!(!applied.contains("has_id_pushdown"), "{text}");
}

// T4: source_filter_pushdown
#[test]
fn source_filter_pushdown_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .has_label("person")
        .id()
        .to_list()
        .unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["source_filter_pushdown"])
        .v(None::<()>)
        .has_label("person")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(sorted_strings(baseline), sorted_strings(disabled));

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["source_filter_pushdown"])
        .v(None::<()>)
        .has_label("person")
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    let applied = text
        .lines()
        .find(|l| l.starts_with("Optimizer rules applied:"))
        .unwrap_or("");
    assert!(!applied.contains("source_filter_pushdown"), "{text}");
}

// T5: barrier_pushdown
#[test]
fn barrier_pushdown_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .out_e(None::<()>)
        .has_label("knows")
        .count()
        .next()
        .unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["barrier_pushdown"])
        .v(None::<()>)
        .out_e(None::<()>)
        .has_label("knows")
        .count()
        .next()
        .unwrap();
    assert_eq!(baseline, disabled);
    assert_eq!(disabled.unwrap().as_i64().unwrap(), 2);

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["barrier_pushdown"])
        .v(None::<()>)
        .out_e(None::<()>)
        .has_label("knows")
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    assert!(
        !text.contains("Optimizer rules applied:") || !text.contains("barrier_pushdown"),
        "{text}"
    );
}

// T6: group_count_pushdown
#[test]
fn group_count_pushdown_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    use graphersal::prelude::By;

    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .group_count()
        .by(By::Label)
        .next()
        .unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["group_count_pushdown"])
        .v(None::<()>)
        .group_count()
        .by(By::Label)
        .next()
        .unwrap();
    assert_eq!(baseline, disabled);

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["group_count_pushdown"])
        .v(None::<()>)
        .group_count()
        .by(By::Label)
        .profile()
        .unwrap();
    let text = stats.to_string();
    assert!(
        !text.contains("Optimizer rules applied:") || !text.contains("group_count_pushdown"),
        "{text}"
    );
}

// T7: count_pushdown
#[test]
fn count_pushdown_disabled_keeps_the_result_correct_and_drops_out_of_profile() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock.traversal().v(None::<()>).count().next().unwrap();
    let disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["count_pushdown"])
        .v(None::<()>)
        .count()
        .next()
        .unwrap();
    assert_eq!(baseline, disabled);
    assert_eq!(disabled.unwrap().as_i64().unwrap(), 6);

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["count_pushdown"])
        .v(None::<()>)
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    assert!(
        !text.contains("Optimizer rules applied:") || !text.contains("count_pushdown"),
        "{text}"
    );
}

// T8: unknown rule name
#[test]
fn an_unknown_rule_name_fails_with_invalid_option_at_execution() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .with("optimizer.disabled", vec!["not_a_real_rule"])
        .v(None::<()>)
        .count()
        .next()
        .expect_err("unknown rule name must be rejected");
    assert!(matches!(
        *err,
        TraverserError::InvalidOption { ref key, .. } if key == "optimizer.disabled"
    ));

    let err = lock
        .traversal()
        .with("optimizer.enabled", vec!["not_a_real_rule"])
        .v(None::<()>)
        .count()
        .next()
        .expect_err("unknown rule name must be rejected");
    assert!(matches!(
        *err,
        TraverserError::InvalidOption { ref key, .. } if key == "optimizer.enabled"
    ));
}

// T9: a name in both lists at once
#[test]
fn a_rule_named_in_both_lists_fails_as_contradictory() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .with("optimizer.disabled", vec!["filter_reorder"])
        .with("optimizer.enabled", vec!["filter_reorder"])
        .v(None::<()>)
        .count()
        .next()
        .expect_err("a rule cannot be both enabled and disabled");
    assert!(matches!(
        *err,
        TraverserError::InvalidOption { ref key, .. } if key == "optimizer.enabled"
    ));
}

// T10: nothing fires
#[test]
fn profile_omits_the_applied_rules_line_when_nothing_fired() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    // A bare v() has nothing for any rule to fold, reorder or push down.
    let stats = lock.traversal().v(None::<()>).profile().unwrap();
    let text = stats.to_string();
    assert!(!text.contains("Optimizer rules applied:"), "{text}");
}

// T12: the reserved "all" token disables every rule at once, without listing them.
#[test]
fn optimizer_disabled_all_turns_off_every_rule_but_keeps_the_result_correct() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let baseline = lock
        .traversal()
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .id()
        .to_list()
        .unwrap();
    let all_disabled = lock
        .traversal()
        .with("optimizer.disabled", vec!["all"])
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .id()
        .to_list()
        .unwrap();
    assert_eq!(sorted_strings(baseline), sorted_strings(all_disabled));

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["all"])
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    // Nothing fired (every rule was disabled), so the summary line is absent — same as T10, just
    // reached through "all" instead of there being nothing to fold.
    assert!(!text.contains("Optimizer rules applied:"), "{text}");
    // has_label()/has_id() stayed their own physical steps instead of folding into StepV, proving
    // has_id_pushdown/source_filter_pushdown genuinely did not run.
    assert!(text.contains("has_label(\"person\")"), "{text}");
    assert!(text.contains("has_id(\"1\")"), "{text}");
}

// T13: "all" also expands inside optimizer.enabled, and still conflicts with optimizer.disabled
// per-rule the same way a spelled-out name would.
#[test]
fn optimizer_enabled_all_conflicts_with_a_disabled_rule() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let err = lock
        .traversal()
        .with("optimizer.disabled", vec!["filter_reorder"])
        .with("optimizer.enabled", vec!["all"])
        .v(None::<()>)
        .count()
        .next()
        .expect_err("\"all\" expands to include the already-disabled rule");
    assert!(matches!(
        *err,
        TraverserError::InvalidOption { ref key, .. } if key == "optimizer.enabled"
    ));
}

// T11: disabling one rule does not suppress the others.
#[test]
fn disabling_one_rule_leaves_the_others_reported() {
    let graph = GraphSource::tinkerpop_modern();
    let lock = graph.read();

    let stats = lock
        .traversal()
        .with("optimizer.disabled", vec!["filter_reorder"])
        .v(None::<()>)
        .has_label("person")
        .has_id("1")
        .count()
        .profile()
        .unwrap();
    let text = stats.to_string();
    let applied = text
        .lines()
        .find(|l| l.starts_with("Optimizer rules applied:"))
        .unwrap();
    assert!(applied.contains("has_id_pushdown"), "{applied}");
    assert!(applied.contains("source_filter_pushdown"), "{applied}");
    assert!(applied.contains("count_pushdown"), "{applied}");
    assert!(!applied.contains("filter_reorder"), "{applied}");
}
