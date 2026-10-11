//! Checks of the DSL function documentation (`docs.rs`): every function of the completion
//! catalogue has its text, every text is used, and every example gives its documented result.

use super::docs::{DOCS, FnDoc, NOT_RUN, comment_lines, is_undocumented, key, lookup, receiver_of};
use crate::script::{ScriptLimits, engine, eval_value_with_limits};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// `(name, first parameter type, doc)` of every function of the engine's global namespace that
/// the completion catalogue lists (what `graphersal-session`'s `completions` reads).
fn catalogue_functions() -> Vec<(String, String, String)> {
    let metadata: serde_json::Value = serde_json::from_str(
        &engine()
            .gen_fn_metadata_to_json(false)
            .unwrap_or_else(|err| panic!("metadata: {err}")),
    )
    .unwrap_or_else(|err| panic!("metadata json: {err}"));
    let mut out = Vec::new();
    for f in metadata["functions"].as_array().into_iter().flatten() {
        let name = f["name"].as_str().unwrap_or_default().to_string();
        if is_undocumented(&name) {
            continue;
        }
        let first = f["params"][0]["type"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let doc = f["docComments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        out.push((name, first, doc));
    }
    out
}

/// One catalogue entry: its spellings and the distinct docs of its overloads.
#[derive(Default)]
struct Entry {
    names: BTreeSet<String>,
    docs: BTreeSet<String>,
}

/// The catalogue entries by `(receiver, key)`, merged as the completion catalogue merges them.
fn catalogue_entries() -> BTreeMap<(&'static str, String), Entry> {
    let mut entries: BTreeMap<(&'static str, String), Entry> = BTreeMap::new();
    for (name, first, doc) in catalogue_functions() {
        let first = (!first.is_empty()).then_some(first.as_str());
        let entry = entries.entry((receiver_of(first), key(&name))).or_default();
        entry.names.insert(name);
        if !doc.is_empty() {
            entry.docs.insert(doc);
        }
    }
    entries
}

#[test]
fn every_catalogue_function_has_its_doc() {
    let mut problems = Vec::new();
    for ((receiver, _), entry) in catalogue_entries() {
        let Some(name) = entry.names.first() else {
            continue;
        };
        let Some(doc) = lookup(receiver, name) else {
            problems.push(format!(
                "{receiver}: {:?} has no entry in DOCS",
                entry.names
            ));
            continue;
        };
        let expected = comment_lines(doc).join("\n");
        // An overload without a doc is fine (the catalogue shows the entry's one text); a
        // different text is not (a stale `///` of a plugin module function).
        if entry.docs.iter().any(|d| *d != expected) || entry.docs.is_empty() {
            problems.push(format!(
                "{receiver}: {:?} carries {:?}, expected {expected:?}",
                entry.names, entry.docs
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "{} problems (texts live in script/register/docs.rs):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
#[cfg(all(feature = "io", feature = "persist"))]
fn every_doc_entry_is_used() {
    let used: BTreeSet<(&str, &str)> = catalogue_entries()
        .into_iter()
        .filter_map(|((receiver, _), entry)| {
            let name = entry.names.first()?.clone();
            lookup(receiver, &name).map(|doc| (doc.receiver, doc.name))
        })
        .collect();
    let unused: Vec<String> = DOCS
        .iter()
        .filter(|doc| !used.contains(&(doc.receiver, doc.name)))
        .map(|doc| format!("{}.{}", doc.receiver, doc.name))
        .collect();
    assert!(
        unused.is_empty(),
        "DOCS entries for no registered function: {unused:?}"
    );
    let mut seen = BTreeSet::new();
    for doc in DOCS {
        assert!(
            seen.insert((doc.receiver, key(doc.name))),
            "duplicate DOCS entry {}.{}",
            doc.receiver,
            doc.name
        );
    }
}

/// Runs `doc`'s example on a fresh modern graph: its `to_string()` text, or the error text.
fn run_example(doc: &FnDoc) -> Result<String, String> {
    let graph = Arc::new(crate::GraphSource::tinkerpop_modern());
    eval_value_with_limits(
        graph,
        &format!("({{ {} }}).to_string()", doc.example),
        Default::default(),
        &ScriptLimits::default(),
        Arc::new(crate::graph::AllowAll),
    )
    .map(|value| value.to_string())
    .map_err(|err| err.to_string())
}

#[test]
fn every_example_gives_its_documented_result() {
    let mut problems = Vec::new();
    for doc in DOCS.iter().filter(|doc| doc.result != NOT_RUN) {
        let outcome = run_example(doc);
        let ok = match (doc.result.strip_prefix("error: "), &outcome) {
            (Some(expected), Err(err)) => err.contains(expected),
            (Some(_), Ok(_)) => false,
            (None, Ok(text)) => doc.result.is_empty() || text == doc.result,
            (None, Err(_)) => false,
        };
        if !ok {
            problems.push(format!(
                "{}.{}: `{}` gave {outcome:?}, documented {:?}",
                doc.receiver, doc.name, doc.example, doc.result
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn texts_follow_the_house_style() {
    for doc in DOCS {
        let id = format!("{}.{}", doc.receiver, doc.name);
        assert!(
            doc.summary.ends_with('.'),
            "{id}: the summary is one sentence"
        );
        assert!(
            !doc.summary.contains('\n') && !doc.example.contains('\n'),
            "{id}"
        );
        // Canonical snake_case DSL: no camelCase step spelling in examples.
        for camel in [
            "hasLabel(",
            "outE(",
            "inV(",
            "valueMap(",
            "toList(",
            "groupCount(",
        ] {
            assert!(!doc.example.contains(camel), "{id}: {camel} in the example");
        }
    }
}
