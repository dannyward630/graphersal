//! Turns a scenario's run (step 4) plus its assertions into exactly one [`Outcome`].
//!
//! # Precedence
//!
//! When several classes could apply: `excluded` > `skipped` > `translate` > `panic` > `timeout` > `missing` >
//! `runtime` > `wrong_error` > `wrong_result` (see [`Class::rank`]).
//!
//! # Reading of `wrong_error`
//!
//! The design describes `wrong_error` as "expected an error but got a result, or the reverse".
//! This classifier reads "the reverse" narrowly: an *unexpected* error is counted by what failed
//! (`missing` for Rhai `Function/Variable not found`, `runtime` for everything else), which says
//! more about the backlog than a generic `wrong_error`. `wrong_error` therefore means: an error was
//! expected but values came back. An expected error is matched by *type*: any non-`missing` runtime
//! error satisfies it, whatever its text (the expected `message` is parsed but not compared; our own
//! messages are more detailed and are not chased).
//!
//! A `missing` failure never satisfies an expected error, even one without message text: the
//! step or token that should reject the input does not exist, so the upstream check was not
//! exercised (stricter than "the script failed (any class)"; recorded as a deliberate choice).
//!
//! # Rows
//!
//! `When iterated to list`: the traversal's values are the rows. `When iterated next`: upstream
//! (`IteratorUtils.asList(traversal.next())`) turns the first value into rows: a collection's items
//! (a list, or a `path()`, which is an `Array` in Graphersal), a map's entries (each as a one-entry
//! map), anything else as the single row.
//!
//! # Assertions
//!
//! - `Unordered`: multiset equality. `Ordered`: exact sequence; if only the order differs and the
//!   translated script has no `order(` / `by(` call, it passes with `order_relaxed`.
//! - `Of`: every row equals one of the listed values (vacuously true for no rows, as upstream).
//! - `Empty` / `Count(n)`: zero / `n` rows. `GraphCount`: the count step 4 obtained after the run.
//! - `Tree`: the single result is a map of key to nested map (leaf = empty map); siblings match as
//!   a set (see `compare::compare_tree`).
//! - `Subgraph`: the graph value's `{"vertices", "edges"}` listing is compared with the edge and vertex tables as multisets.
//! - An expectation the runner cannot resolve (see `compare.rs`) → `translate`.
//! - A parameter binding failure (`parameter binding: ...`) → `translate`.
//! - Ids compare tolerantly everywhere iff the translated script reads ids (`id()` or `T.id`),
//!   else only under `t[id]` keys (see `compare.rs`).

use super::compare::{self, IdMode, LazyIndex, Resolved};
use super::execute::{RawOutcome, ScenarioRun, ScriptFailure};
use super::gherkin::{Assertion, Iteration, Scenario};
use super::notation;

use graphersal::ElementProperty;

/// The final class of a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    /// Every assertion holds.
    Pass,
    /// Not run: tag, unsupported test or dataset.
    Skipped,
    /// The translator or the runner cannot express the scenario.
    Translate,
    /// Rhai `Function not found` / `Variable not found`.
    Missing,
    /// Any other script / `TraverserError` failure while executing.
    Runtime,
    /// Executed, but a result assertion does not hold.
    WrongResult,
    /// An error was expected but values came back.
    WrongError,
    /// The engine's or the harness's time budget was exceeded.
    Timeout,
    /// A panic was caught.
    Panic,
    /// Out of the compatibility scope (see `scope.rs`): not run, in no total, not in the baseline.
    Excluded,
}

impl Class {
    /// Every in-scope class, in report order. [`Class::Excluded`] is deliberately not listed: it
    /// never appears in a table, a total or the baseline.
    pub const ALL: [Class; 9] = [
        Class::Pass,
        Class::Skipped,
        Class::Translate,
        Class::Missing,
        Class::Runtime,
        Class::WrongResult,
        Class::WrongError,
        Class::Timeout,
        Class::Panic,
    ];

    /// Stable lower-case name (for reports and the baseline).
    pub fn name(self) -> &'static str {
        match self {
            Class::Pass => "pass",
            Class::Skipped => "skipped",
            Class::Translate => "translate",
            Class::Missing => "missing",
            Class::Runtime => "runtime",
            Class::WrongResult => "wrong_result",
            Class::WrongError => "wrong_error",
            Class::Timeout => "timeout",
            Class::Panic => "panic",
            Class::Excluded => "excluded",
        }
    }

    /// Precedence: the higher rank wins when several classes apply.
    pub fn rank(self) -> u8 {
        match self {
            Class::Pass => 0,
            Class::WrongResult => 1,
            Class::WrongError => 2,
            Class::Runtime => 3,
            Class::Missing => 4,
            Class::Timeout => 5,
            Class::Panic => 6,
            Class::Translate => 7,
            Class::Skipped => 8,
            Class::Excluded => 9,
        }
    }
}

/// The classified outcome of one scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The class.
    pub class: Class,
    /// One-line reason: the missing name, the error's first line, the first diff, ...
    pub detail: String,
    /// A pass that needed the ordered-assertion relaxation.
    pub order_relaxed: bool,
    /// The step the scenario exercises (`@Step<Name>` tag, else the feature file name).
    pub exercised_step: Option<String>,
}

/// Maximum length of [`Outcome::detail`].
const DETAIL_MAX: usize = 300;

fn one_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > DETAIL_MAX {
        let mut cut: String = line.chars().take(DETAIL_MAX).collect();
        cut.push('…');
        cut
    } else {
        line.to_string()
    }
}

/// One-line summary of an engine diagnostic: its first line, plus its `Caused by:` line when the
/// diagnostic has one (the first line alone often only names the failing step).
pub fn error_summary(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    match text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("Caused by:"))
    {
        Some(cause) => one_line(&format!("{first} | {cause}")),
        None => one_line(first),
    }
}

/// The step a scenario exercises: a `@Step<Name>` tag (not `@StepClass...`), else the feature
/// file's name with a lower-case first letter (`map/Fold.feature` → `fold`).
pub fn exercised_step(scenario: &Scenario) -> Option<String> {
    let lower_first = |s: &str| {
        let mut chars = s.chars();
        chars
            .next()
            .map(|c| c.to_lowercase().chain(chars).collect::<String>())
    };
    if let Some(name) = scenario
        .tags
        .iter()
        .filter_map(|t| t.strip_prefix("@Step"))
        .find(|rest| !rest.is_empty() && !rest.starts_with("Class"))
    {
        return lower_first(name);
    }
    let path = scenario.id.split("::").next()?;
    let file = path.rsplit('/').next()?;
    lower_first(file.strip_suffix(".feature").unwrap_or(file))
}

/// The top-level category directory of a scenario (`branch`, `filter`, `map`, ...).
pub fn category(scenario: &Scenario) -> &str {
    let path = scenario.id.split("::").next().unwrap_or("");
    match path.split_once('/') {
        Some((dir, _)) => dir,
        None => "(root)",
    }
}

/// Classifies one scenario run.
pub fn classify(scenario: &Scenario, run: &ScenarioRun) -> Outcome {
    let (class, detail, order_relaxed) = decide(scenario, run);
    Outcome {
        class,
        detail: one_line(&detail),
        order_relaxed,
        exercised_step: exercised_step(scenario),
    }
}

/// The outcome of a scenario that is out of the compatibility scope (it is not run). The detail
/// is the strategy that excludes it (its reason comes from [`super::scope::reason_of`]), or
/// `incompat:<code>` for a deliberate incompatibility.
pub fn excluded(scenario: &Scenario, exclusion: &super::scope::Excluded) -> Outcome {
    use super::scope::{ExclusionKind, INCOMPAT_DETAIL_PREFIX};
    let detail = match exclusion.kind {
        ExclusionKind::Strategy => exclusion.strategy.clone(),
        ExclusionKind::Incompatibility => {
            format!("{INCOMPAT_DETAIL_PREFIX}{}", exclusion.strategy)
        }
    };
    Outcome {
        class: Class::Excluded,
        detail,
        order_relaxed: false,
        exercised_step: exercised_step(scenario),
    }
}

fn expected_error(scenario: &Scenario) -> Option<Option<&str>> {
    scenario.assertions.iter().find_map(|a| match a {
        Assertion::RaisesError { message } => Some(message.as_deref()),
        _ => None,
    })
}

fn decide(scenario: &Scenario, run: &ScenarioRun) -> (Class, String, bool) {
    let prefix = if run.initializer_failed {
        "initializer: "
    } else {
        ""
    };
    match &run.outcome {
        RawOutcome::Skipped(reason) => (Class::Skipped, reason.clone(), false),
        RawOutcome::Untranslatable(reason) => (Class::Translate, reason.clone(), false),
        RawOutcome::Panic(message) => (Class::Panic, message.clone(), false),
        RawOutcome::Timeout => (Class::Timeout, "timeout".to_string(), false),
        RawOutcome::ScriptError(ScriptFailure::Other(m)) if m.starts_with("parameter binding:") => {
            (Class::Translate, m.clone(), false)
        }
        RawOutcome::ScriptError(ScriptFailure::Missing { name, .. }) => {
            (Class::Missing, format!("{prefix}{name}"), false)
        }
        RawOutcome::ScriptError(ScriptFailure::Runtime(m) | ScriptFailure::Other(m)) => {
            match expected_error(scenario) {
                // Policy: an expected error matches by type (any non-`missing` runtime error). The
                // expected message text is parsed but deliberately not compared: Graphersal keeps its
                // own, more detailed messages.
                Some(_) if !run.initializer_failed => (Class::Pass, String::new(), false),
                _ => (
                    Class::Runtime,
                    format!("{prefix}{}", error_summary(m)),
                    false,
                ),
            }
        }
        RawOutcome::Values(values) => {
            if expected_error(scenario).is_some() {
                return (
                    Class::WrongError,
                    format!(
                        "expected an error, got {} value(s): {}",
                        values.len(),
                        short_list(values)
                    ),
                    false,
                );
            }
            check_assertions(scenario, run, values)
        }
    }
}

fn short_list(values: &[ElementProperty]) -> String {
    let items: Vec<String> = values.iter().take(8).map(compare::short).collect();
    let more = if values.len() > 8 { ", …" } else { "" };
    format!("[{}{more}]", items.join(", "))
}

fn short_expected_list(values: &[Resolved]) -> String {
    let items: Vec<String> = values.iter().take(8).map(compare::short_expected).collect();
    let more = if values.len() > 8 { ", …" } else { "" };
    format!("[{}{more}]", items.join(", "))
}

/// The rows an assertion sees (see the module docs for `iterated next`).
pub fn rows(iteration: Option<Iteration>, values: &[ElementProperty]) -> Vec<ElementProperty> {
    match (iteration, values) {
        (Some(Iteration::Next), [ElementProperty::Array(items)]) => items.clone(),
        (Some(Iteration::Next), [ElementProperty::Object(map)]) => map
            .iter()
            .map(|(k, v)| {
                ElementProperty::Object(Box::new(std::iter::once((k.clone(), v.clone())).collect()))
            })
            .collect(),
        (Some(Iteration::Next), [first, ..]) => vec![first.clone()],
        _ => values.to_vec(),
    }
}

/// The failure of one assertion.
struct Failure {
    class: Class,
    detail: String,
}

fn check_assertions(
    scenario: &Scenario,
    run: &ScenarioRun,
    values: &[ElementProperty],
) -> (Class, String, bool) {
    let rows = rows(scenario.iteration, values);
    let script = run.script.as_deref().unwrap_or("");
    let ids = if compare::script_reads_ids(script) {
        IdMode::Scenario
    } else {
        IdMode::KeysOnly
    };
    let lookup = LazyIndex::new(run.graph.as_ref().map(|g| &g.0));
    let mut worst: Option<Failure> = None;
    let mut relaxed = false;
    let mut graph_counts = run.graph_counts.iter();

    for assertion in &scenario.assertions {
        let result = check_one(
            assertion,
            &rows,
            values,
            ids,
            script,
            &lookup,
            &mut graph_counts,
            &mut relaxed,
        );
        if let Err(failure) = result
            && worst
                .as_ref()
                .is_none_or(|w| failure.class.rank() > w.class.rank())
        {
            worst = Some(failure);
        }
    }
    match worst {
        Some(f) => (f.class, f.detail, false),
        None => (Class::Pass, String::new(), relaxed),
    }
}

fn resolve_cells(cells: &[String], lookup: &LazyIndex<'_>) -> Result<Vec<Resolved>, Failure> {
    cells
        .iter()
        .map(|cell| {
            let expected = notation::parse_cell(cell).map_err(|e| Failure {
                class: Class::Translate,
                detail: format!("expected value {cell:?}: {e}"),
            })?;
            compare::resolve(&expected, lookup).map_err(|reason| Failure {
                class: Class::Translate,
                detail: reason,
            })
        })
        .collect()
}

fn wrong(detail: String) -> Failure {
    Failure {
        class: Class::WrongResult,
        detail,
    }
}

// The per-scenario state threaded through every assertion check; bundling it into a struct
// for this one private helper would not make the harness clearer.
#[allow(clippy::too_many_arguments)]
fn check_one<'a>(
    assertion: &Assertion,
    rows: &[ElementProperty],
    values: &[ElementProperty],
    ids: IdMode,
    script: &str,
    lookup: &LazyIndex<'_>,
    graph_counts: &mut impl Iterator<Item = &'a (String, Result<i64, String>)>,
    relaxed: &mut bool,
) -> Result<(), Failure> {
    match assertion {
        Assertion::Unordered(cells) => {
            let expected = resolve_cells(cells, lookup)?;
            match compare::multiset_diff(&expected, rows, ids) {
                None => Ok(()),
                Some(diff) => Err(wrong(format!(
                    "unordered: {} expected, {} actual; missing {} unexpected {}",
                    expected.len(),
                    rows.len(),
                    short_expected_list(&diff.missing),
                    short_list(&diff.unexpected)
                ))),
            }
        }
        Assertion::Ordered(cells) => {
            let expected = resolve_cells(cells, lookup)?;
            let Some(at) = compare::first_sequence_diff(&expected, rows, ids) else {
                return Ok(());
            };
            let same_multiset = compare::multiset_diff(&expected, rows, ids).is_none();
            if same_multiset && !compare::script_orders(script) {
                *relaxed = true;
                return Ok(());
            }
            Err(wrong(format!(
                "ordered: first difference at row {at}{}; expected {} actual {}",
                if same_multiset {
                    " (same multiset)"
                } else {
                    ""
                },
                short_expected_list(&expected),
                short_list(rows)
            )))
        }
        Assertion::Of(cells) => {
            let expected = resolve_cells(cells, lookup)?;
            match rows
                .iter()
                .find(|row| !expected.iter().any(|e| compare::matches(e, row, ids)))
            {
                None => Ok(()),
                Some(row) => Err(wrong(format!(
                    "of: {} is none of {}",
                    compare::short(row),
                    short_expected_list(&expected)
                ))),
            }
        }
        Assertion::Empty => {
            if rows.is_empty() {
                Ok(())
            } else {
                Err(wrong(format!(
                    "expected empty, got {}: {}",
                    rows.len(),
                    short_list(rows)
                )))
            }
        }
        Assertion::Count(n) => {
            if rows.len() == *n {
                Ok(())
            } else {
                Err(wrong(format!(
                    "expected count {n}, got {}: {}",
                    rows.len(),
                    short_list(rows)
                )))
            }
        }
        Assertion::GraphCount { count, traversal } => match graph_counts.next() {
            None => Err(Failure {
                class: Class::Runtime,
                detail: format!("graph count {traversal:?} was not run"),
            }),
            Some((_, Ok(actual))) => {
                if i64::try_from(*count).is_ok_and(|c| c == *actual) {
                    Ok(())
                } else {
                    Err(wrong(format!(
                        "graph count of {traversal:?}: expected {count}, got {actual}"
                    )))
                }
            }
            Some((_, Err(reason))) => Err(graph_count_failure(traversal, reason)),
        },
        Assertion::Tree(text) => {
            let expected = compare::parse_tree(text, lookup).map_err(|reason| Failure {
                class: Class::Translate,
                detail: reason,
            })?;
            // The raw values, not `rows`: `iterated next` spreads a map result into one row per
            // entry, but a tree is one value.
            match values {
                [actual] if compare::compare_tree(&expected, actual, ids) => Ok(()),
                [actual] => Err(wrong(format!(
                    "tree: actual {}",
                    compare::short_tree(actual)
                ))),
                _ => Err(wrong(format!(
                    "tree: expected one result, got {}: {}",
                    values.len(),
                    short_list(values)
                ))),
            }
        }
        // The graph value materializes as `{"vertices": [..], "edges": [..]}` (each listed like a
        // `g.V()`/`g.E()` result), compared as multisets of elements.
        Assertion::Subgraph { edges, vertices } => {
            let [ElementProperty::Object(graph)] = values else {
                return Err(wrong(format!(
                    "subgraph: expected one graph value, got {}",
                    short_list(values)
                )));
            };
            let listed = |key: &str| -> Vec<ElementProperty> {
                match graph.get(key) {
                    Some(ElementProperty::Array(items)) => items.clone(),
                    _ => Vec::new(),
                }
            };
            for (what, cells, actual) in [
                ("edges", edges, listed("edges")),
                ("vertices", vertices, listed("vertices")),
            ] {
                let expected = resolve_cells(cells, lookup)?;
                if let Some(diff) = compare::multiset_diff(&expected, &actual, ids) {
                    return Err(wrong(format!(
                        "subgraph {what}: {} expected, {} actual; missing {} unexpected {}",
                        expected.len(),
                        actual.len(),
                        short_expected_list(&diff.missing),
                        short_list(&diff.unexpected)
                    )));
                }
            }
            Ok(())
        }
        // Handled before the rows are checked (`decide`); `NothingHappens` only occurs in
        // unsupported (skipped) scenarios.
        Assertion::RaisesError { .. } | Assertion::NothingHappens(_) => Ok(()),
    }
}

/// Classifies the failure text `execute` recorded for a `GraphCount` traversal.
fn graph_count_failure(traversal: &str, reason: &str) -> Failure {
    let context = format!("graph count {traversal:?}");
    if let Some(r) = reason.strip_prefix("untranslatable: ") {
        Failure {
            class: Class::Translate,
            detail: format!("{context}: {r}"),
        }
    } else if reason == "timeout" {
        Failure {
            class: Class::Timeout,
            detail: format!("{context}: timeout"),
        }
    } else if let Some(signature) = reason.strip_prefix("missing: ") {
        let name = signature.split(" (").next().unwrap_or(signature).trim();
        Failure {
            class: Class::Missing,
            detail: name.to_string(),
        }
    } else {
        Failure {
            class: Class::Runtime,
            detail: format!("{context}: {}", error_summary(reason)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::execute::RunGraph;
    use crate::harness::gherkin;
    use graphersal::prelude::Number;

    fn scenario(path: &str, tags: &str, body: &str) -> Scenario {
        let text = format!("Feature: T\n\n  {tags}\n  Scenario: s\n{body}");
        let feature = gherkin::parse_feature(path, &text).expect("parses");
        feature.scenarios.into_iter().next().expect("one scenario")
    }

    fn body(iteration: &str, then: &str) -> String {
        format!(
            "    Given the modern graph\n    And the traversal of\n      \"\"\"\n      g.V()\n      \"\"\"\n    When {iteration}\n{then}"
        )
    }

    fn run(outcome: RawOutcome, script: &str) -> ScenarioRun {
        ScenarioRun {
            outcome,
            script: Some(script.to_string()),
            graph_counts: Vec::new(),
            native_result: false,
            fresh_graph: false,
            initializer_failed: false,
            harness_error: None,
            graph: None,
        }
    }

    fn i(v: i64) -> ElementProperty {
        ElementProperty::Number(Number::from_i64(v))
    }

    fn s(v: &str) -> ElementProperty {
        ElementProperty::String(v.into())
    }

    #[test]
    fn exercised_step_from_tag_or_file() {
        let sc = scenario(
            "map/Fold.feature",
            "",
            &body("iterated to list", "    Then the result should be empty\n"),
        );
        assert_eq!(exercised_step(&sc).as_deref(), Some("fold"));
        let sc = scenario(
            "filter/HasLabel.feature",
            "@StepClassFilter",
            &body("iterated to list", "    Then the result should be empty\n"),
        );
        assert_eq!(exercised_step(&sc).as_deref(), Some("hasLabel"));
        let sc = scenario(
            "filter/X.feature",
            "@StepIs",
            &body("iterated to list", "    Then the result should be empty\n"),
        );
        assert_eq!(exercised_step(&sc).as_deref(), Some("is"));
        assert_eq!(category(&sc), "filter");
    }

    #[test]
    fn precedence_order() {
        let order = [
            Class::Excluded,
            Class::Skipped,
            Class::Translate,
            Class::Panic,
            Class::Timeout,
            Class::Missing,
            Class::Runtime,
            Class::WrongError,
            Class::WrongResult,
            Class::Pass,
        ];
        for pair in order.windows(2) {
            assert!(pair[0].rank() > pair[1].rank(), "{pair:?}");
        }
    }

    #[test]
    fn raw_outcomes_map_to_classes() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body("iterated to list", "    Then the result should be empty\n"),
        );
        let class = |o: RawOutcome| classify(&sc, &run(o, "g.V()")).class;
        assert_eq!(class(RawOutcome::Skipped("x".into())), Class::Skipped);
        assert_eq!(
            class(RawOutcome::Untranslatable("x".into())),
            Class::Translate
        );
        assert_eq!(class(RawOutcome::Panic("x".into())), Class::Panic);
        assert_eq!(class(RawOutcome::Timeout), Class::Timeout);
        let missing = classify(
            &sc,
            &run(
                RawOutcome::ScriptError(ScriptFailure::Missing {
                    name: "sack".into(),
                    signature: "sack (Graph)".into(),
                }),
                "",
            ),
        );
        assert_eq!(
            (missing.class, missing.detail.as_str()),
            (Class::Missing, "sack")
        );
        let runtime = RawOutcome::ScriptError(ScriptFailure::Runtime("boom\nhelp".into()));
        let out = classify(&sc, &run(runtime, ""));
        assert_eq!((out.class, out.detail.as_str()), (Class::Runtime, "boom"));
        let caused = "Error: Step #1 failed\n  at #1\nCaused by: bad cast\nHelp: x";
        assert_eq!(
            error_summary(caused),
            "Error: Step #1 failed | Caused by: bad cast"
        );
        let binding = RawOutcome::ScriptError(ScriptFailure::Other(
            "parameter binding: v[x] not found in the graph".into(),
        ));
        assert_eq!(class(binding), Class::Translate);
        assert_eq!(class(RawOutcome::Values(vec![])), Class::Pass);
        assert_eq!(class(RawOutcome::Values(vec![i(1)])), Class::WrongResult);
    }

    #[test]
    fn expected_errors() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body(
                "iterated to list",
                "    Then the traversal will raise an error with message containing text of \"bad input\"\n",
            ),
        );
        let rt = |m: &str| RawOutcome::ScriptError(ScriptFailure::Runtime(m.into()));
        assert_eq!(
            classify(&sc, &run(rt("a bad input here"), "")).class,
            Class::Pass
        );
        // An error with other text still satisfies the expectation (matched by type).
        assert_eq!(classify(&sc, &run(rt("other"), "")).class, Class::Pass);
        assert_eq!(
            classify(&sc, &run(RawOutcome::Values(vec![]), "")).class,
            Class::WrongError
        );
        let missing = RawOutcome::ScriptError(ScriptFailure::Missing {
            name: "x".into(),
            signature: "x".into(),
        });
        assert_eq!(classify(&sc, &run(missing, "")).class, Class::Missing);
    }

    #[test]
    fn ordered_relaxation_only_without_ordering_step() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body(
                "iterated to list",
                "    Then the result should be ordered\n      | result |\n      | d[1].i |\n      | d[2].i |\n",
            ),
        );
        let swapped = RawOutcome::Values(vec![i(2), i(1)]);
        let out = classify(&sc, &run(swapped.clone(), "g.V().values(\"x\")"));
        assert_eq!((out.class, out.order_relaxed), (Class::Pass, true));
        let out = classify(&sc, &run(swapped, "g.V().order().by(\"x\")"));
        assert_eq!(out.class, Class::WrongResult);
        let exact = classify(
            &sc,
            &run(RawOutcome::Values(vec![i(1), i(2)]), "g.V().order()"),
        );
        assert_eq!((exact.class, exact.order_relaxed), (Class::Pass, false));
        let wrong = classify(&sc, &run(RawOutcome::Values(vec![i(1), i(3)]), "g.V()"));
        assert_eq!(wrong.class, Class::WrongResult);
    }

    #[test]
    fn of_count_and_id_mode() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body(
                "iterated to list",
                "    Then the result should be of\n      | result |\n      | d[1].i |\n      | d[2].i |\n    And the result should have a count of 2\n",
            ),
        );
        let values = RawOutcome::Values(vec![s("1"), s("1")]);
        assert_eq!(
            classify(&sc, &run(values.clone(), "g.V().id()")).class,
            Class::Pass
        );
        assert_eq!(
            classify(&sc, &run(values, "g.V().values(\"x\")")).class,
            Class::WrongResult
        );
        let three = RawOutcome::Values(vec![i(1), i(1), i(2)]);
        assert_eq!(classify(&sc, &run(three, "")).class, Class::WrongResult);
    }

    #[test]
    fn iterated_next_uses_collection_items_as_rows() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body(
                "iterated next",
                "    Then the result should be unordered\n      | result |\n      | d[1].i |\n      | d[2].i |\n",
            ),
        );
        let folded = RawOutcome::Values(vec![ElementProperty::Array(vec![i(2), i(1)])]);
        assert_eq!(classify(&sc, &run(folded, "")).class, Class::Pass);
        let map =
            ElementProperty::Object(Box::new([("a", i(1)), ("b", i(2))].into_iter().collect()));
        assert_eq!(rows(Some(Iteration::Next), &[map]).len(), 2);
        assert_eq!(rows(Some(Iteration::Next), &[i(1), i(2)]), vec![i(1)]);
        assert_eq!(rows(Some(Iteration::ToList), &[i(1), i(2)]).len(), 2);
    }

    #[test]
    fn unresolvable_expectation_is_a_translate_gap_and_tree_needs_one_result() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body(
                "iterated to list",
                "    Then the result should be unordered\n      | result |\n      | v[nobody] |\n",
            ),
        );
        let graph = crate::harness::storage::wrap(graphersal::TraversalGraph::tinkerpop_modern());
        let mut r = run(RawOutcome::Values(vec![]), "g.V()");
        r.graph = Some(RunGraph(graph));
        let out = classify(&sc, &r);
        assert_eq!(out.class, Class::Translate);
        assert!(
            out.detail.contains("cannot resolve v[nobody]"),
            "{}",
            out.detail
        );

        let sc = scenario(
            "sideEffect/Tree.feature",
            "",
            &body(
                "iterated to list",
                "    Then the result should be a tree with a structure of\n      \"\"\"\n      |--marko\n      \"\"\"\n",
            ),
        );
        // No result at all is a wrong result (a tree assertion needs exactly one value).
        let out = classify(&sc, &run(RawOutcome::Values(vec![]), "g.V().tree()"));
        assert_eq!(out.class, Class::WrongResult);
        let failed = RawOutcome::ScriptError(ScriptFailure::Runtime("x".into()));
        assert_eq!(classify(&sc, &run(failed, "")).class, Class::Runtime);
    }

    #[test]
    fn graph_count_failures() {
        let sc = scenario(
            "map/A.feature",
            "",
            &body(
                "iterated to list",
                "    Then the result should be empty\n    And the graph should return 2 for count of \"g.V()\"\n",
            ),
        );
        let mut r = run(RawOutcome::Values(vec![]), "g.V()");
        r.graph_counts = vec![("g.V()".into(), Ok(2))];
        assert_eq!(classify(&sc, &r).class, Class::Pass);
        r.graph_counts = vec![("g.V()".into(), Ok(3))];
        assert_eq!(classify(&sc, &r).class, Class::WrongResult);
        r.graph_counts = vec![("g.V()".into(), Err("missing: foo (Graph)".into()))];
        let out = classify(&sc, &r);
        assert_eq!((out.class, out.detail.as_str()), (Class::Missing, "foo"));
        r.graph_counts = vec![("g.V()".into(), Err("untranslatable: byte".into()))];
        assert_eq!(classify(&sc, &r).class, Class::Translate);
    }
}
