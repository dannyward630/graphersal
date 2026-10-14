//! Reports of a feature-suite run: the short summary printed on stderr and the detailed Markdown
//! fix backlog written
//! to `target/tinkerpop-report.md`.
//!
//! Both are pure functions of the run's outcomes (sorted by scenario id) and the baseline gate,
//! so they are deterministic for a deterministic run.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::baseline::GateResult;
use super::classify::{Class, Outcome};
use super::scope;

/// Upstream suite version the vendored features come from.
pub const SUITE_VERSION: &str = "3.8.2";

/// Heading of the API inventory section (the report's last section; body from `inventory::markdown`).
pub const INVENTORY_HEADING: &str = "## Not registered at all (API inventory)";

/// One scenario of a run.
#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    /// Scenario id (`<path>::<name>`).
    pub id: &'a str,
    /// Its classified outcome.
    pub outcome: &'a Outcome,
}

impl Row<'_> {
    /// Feature file path (`map/Fold.feature`).
    fn file(&self) -> &str {
        self.id.split("::").next().unwrap_or(self.id)
    }

    /// Category directory (`map`).
    fn category(&self) -> &str {
        self.file().split_once('/').map_or("(root)", |(dir, _)| dir)
    }

    fn class(&self) -> Class {
        self.outcome.class
    }
}

/// Per-class counts of a group of scenarios.
#[derive(Debug, Clone, Default)]
struct Tally {
    total: usize,
    by_class: BTreeMap<Class, usize>,
}

impl Tally {
    fn add(&mut self, class: Class) {
        self.total += 1;
        *self.by_class.entry(class).or_default() += 1;
    }

    fn get(&self, class: Class) -> usize {
        self.by_class.get(&class).copied().unwrap_or(0)
    }

    fn passed(&self) -> usize {
        self.get(Class::Pass)
    }

    fn skipped(&self) -> usize {
        self.get(Class::Skipped)
    }

    /// Not implemented in Graphersal: the step, overload, token or literal type does not exist
    /// (classes `missing` and `translate`).
    fn unsupported(&self) -> usize {
        self.get(Class::Translate) + self.get(Class::Missing)
    }

    /// Implemented but wrong: executes yet differs from the specification or fails (classes
    /// `runtime`, `wrong_result`, `wrong_error`, `timeout`, `panic`). The number to drive to 0.
    fn wrong(&self) -> usize {
        self.total - self.passed() - self.skipped() - self.unsupported()
    }
}

/// Percentage `part / whole` (0 for an empty whole).
pub fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

fn tally<'a>(rows: impl IntoIterator<Item = &'a Row<'a>>) -> Tally {
    let mut t = Tally::default();
    for row in rows {
        t.add(row.class());
    }
    t
}

fn tally_by<'a, F>(rows: &'a [Row<'a>], key: F) -> BTreeMap<&'a str, Tally>
where
    F: Fn(&'a Row<'a>) -> &'a str,
{
    let mut map: BTreeMap<&str, Tally> = BTreeMap::new();
    for row in rows {
        map.entry(key(row)).or_default().add(row.class());
    }
    map
}

/// The missing name of a `missing` outcome (without the `initializer: ` prefix).
fn missing_name(detail: &str) -> &str {
    detail.strip_prefix("initializer: ").unwrap_or(detail)
}

/// The `missing` ranking as counts: missing name → number of scenarios it blocks.
pub fn missing_counts(rows: &[Row<'_>]) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.class() == Class::Missing) {
        *counts
            .entry(missing_name(&row.outcome.detail).to_string())
            .or_default() += 1;
    }
    counts
}

/// Groups rows of one class by a key, ranked by group size (then key).
fn ranked<'a, F>(rows: &[&'a Row<'a>], key: F) -> Vec<(String, Vec<&'a Row<'a>>)>
where
    F: Fn(&Row<'a>) -> String,
{
    let mut groups: BTreeMap<String, Vec<&Row<'a>>> = BTreeMap::new();
    for row in rows {
        groups.entry(key(row)).or_default().push(row);
    }
    let mut groups: Vec<(String, Vec<&Row<'a>>)> = groups.into_iter().collect();
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    groups
}

/// Splits a run into in-scope rows and rows excluded from the compatibility scope
/// ([`Class::Excluded`]). Every total, percentage and table uses only the in-scope rows.
pub fn split_scope<'a>(rows: &[Row<'a>]) -> (Vec<Row<'a>>, Vec<Row<'a>>) {
    rows.iter().partition(|r| r.class() != Class::Excluded)
}

/// The incompatibility code of an excluded row, `None` for a strategy exclusion.
fn incompat_code<'r>(row: &'r Row<'_>) -> Option<&'r str> {
    row.outcome
        .detail
        .strip_prefix(scope::INCOMPAT_DETAIL_PREFIX)
}

/// Excluded rows of the deliberate-incompatibility family.
fn incompat_rows<'r, 'a>(excluded: &'r [Row<'a>]) -> Vec<&'r Row<'a>> {
    excluded
        .iter()
        .filter(|r| incompat_code(r).is_some())
        .collect()
}

/// One reason group of excluded rows: the reason and the rows of each strategy within it.
type ExcludedFamily<'a, 'r> = (&'static str, Vec<(String, Vec<&'r Row<'a>>)>);

/// Excluded rows grouped by reason, then strategy, both in [`scope::EXCLUSIONS`] order (a
/// strategy not in the table falls under the catch-all reason and sorts last). Only the strategy
/// family; deliberate incompatibilities have their own section.
fn excluded_groups<'a, 'r>(excluded: &'r [Row<'a>]) -> Vec<ExcludedFamily<'a, 'r>> {
    let mut out: Vec<ExcludedFamily<'a, 'r>> = Vec::new();
    for row in excluded.iter().filter(|r| incompat_code(r).is_none()) {
        let strategy = row.outcome.detail.as_str();
        let reason = scope::reason_of(strategy);
        let group = match out.iter().position(|(r, _)| *r == reason) {
            Some(i) => &mut out[i].1,
            None => {
                out.push((reason, Vec::new()));
                let last = out.len() - 1;
                &mut out[last].1
            }
        };
        match group.iter().position(|(s, _)| s == strategy) {
            Some(i) => group[i].1.push(row),
            None => group.push((strategy.to_string(), vec![row])),
        }
    }
    for (_, group) in &mut out {
        group.sort_by(|(a, _), (b, _)| scope::rank(a).cmp(&scope::rank(b)).then(a.cmp(b)));
    }
    out.sort_by_key(|(_, g)| g.first().map_or(usize::MAX, |(s, _)| scope::rank(s)));
    out
}

/// One line: `Excluded from compatibility scope: N scenarios [reason: n (Strategy n, ...); ...]`.
fn excluded_line(excluded: &[Row<'_>]) -> String {
    let parts: Vec<String> = excluded_groups(excluded)
        .iter()
        .map(|(reason, group)| {
            let n: usize = group.iter().map(|(_, rs)| rs.len()).sum();
            let per: Vec<String> = group
                .iter()
                .map(|(s, rs)| format!("{s} {}", rs.len()))
                .collect();
            format!("{reason}: {n} ({})", per.join(", "))
        })
        .collect();
    let strategies: usize = excluded_groups(excluded)
        .iter()
        .map(|(_, g)| g.iter().map(|(_, rs)| rs.len()).sum::<usize>())
        .sum();
    format!(
        "Excluded from compatibility scope: {strategies} scenarios [{}]",
        parts.join("; ")
    )
}

/// Scenarios of an incompatibility code among the excluded rows, in table order.
fn incompat_counts<'r, 'a>(
    excluded: &'r [Row<'a>],
) -> Vec<(&'static scope::Incompatibility, Vec<&'r Row<'a>>)> {
    scope::INCOMPATIBILITIES
        .iter()
        .map(|i| {
            let rows: Vec<&Row<'a>> = excluded
                .iter()
                .filter(|r| incompat_code(r) == Some(i.code))
                .collect();
            (i, rows)
        })
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

/// One line: `Deliberate incompatibilities: N scenarios [code n, ...]`.
fn incompat_line(excluded: &[Row<'_>]) -> String {
    let per: Vec<String> = incompat_counts(excluded)
        .iter()
        .map(|(i, rows)| format!("{} {}", i.code, rows.len()))
        .collect();
    format!(
        "Deliberate incompatibilities: {} scenarios [{}]",
        incompat_rows(excluded).len(),
        per.join(", ")
    )
}

/// The scope in one line, derived from the rows: in-scope count and both exclusion families.
fn scope_line(in_scope: usize, excluded: &[Row<'_>]) -> String {
    let incompat = incompat_rows(excluded).len();
    format!(
        "{in_scope} in-scope scenarios; {} excluded (strategies), {incompat} excluded (deliberate incompatibilities)",
        excluded.len() - incompat
    )
}

/// The stderr summary.
pub fn summary(rows: &[Row<'_>], report_path: &str) -> String {
    let (in_scope, excluded) = split_scope(rows);
    let rows = &in_scope[..];
    let all = tally(rows);
    let total = all.total;
    let rule = "=".repeat(70);
    let thin = "-".repeat(70);
    let mut out = String::new();
    let _ = writeln!(out, "{rule}");
    let _ = writeln!(
        out,
        "Apache TinkerPop {SUITE_VERSION} Gremlin Feature Suite Compliance Report"
    );
    let _ = writeln!(out, "{rule}");

    let skip_reasons: Vec<String> = ranked(
        &rows
            .iter()
            .filter(|r| r.class() == Class::Skipped)
            .collect::<Vec<_>>(),
        |r| r.outcome.detail.clone(),
    )
    .into_iter()
    .map(|(reason, rs)| format!("{reason}: {}", rs.len()))
    .collect();
    let failure_classes: Vec<String> = Class::ALL
        .iter()
        .filter(|c| !matches!(c, Class::Pass | Class::Skipped))
        .filter(|c| all.get(**c) > 0)
        .map(|c| format!("{} {}", c.name(), all.get(*c)))
        .collect();
    let _ = writeln!(
        out,
        "Passed Scenarios:  {:>5} / {total}  ({:>5.2}%)",
        all.passed(),
        pct(all.passed(), total)
    );
    let _ = writeln!(
        out,
        "Skipped Scenarios: {:>5} / {total}  ({:>5.2}%) [{}]",
        all.skipped(),
        pct(all.skipped(), total),
        skip_reasons.join(", ")
    );
    let _ = writeln!(
        out,
        "Unsupported:       {:>5} / {total}  ({:>5.2}%) [step, overload, token or literal not implemented: {}]",
        all.unsupported(),
        pct(all.unsupported(), total),
        group_classes(&all, &UNSUPPORTED_CLASSES)
    );
    let _ = writeln!(
        out,
        "Wrong behaviour:   {:>5} / {total}  ({:>5.2}%) [supported steps that differ or fail: {}]",
        all.wrong(),
        pct(all.wrong(), total),
        group_classes(&all, &WRONG_CLASSES)
    );
    let _ = writeln!(out, "Per class: {}", failure_classes.join(", "));
    let executed = total - all.skipped();
    let _ = writeln!(
        out,
        "Pass rate of executed (non-skipped) scenarios: {:.2}% ({}/{executed})",
        pct(all.passed(), executed),
        all.passed()
    );
    let _ = writeln!(out, "{}", scope_line(total, &excluded));
    let _ = writeln!(out, "{}", excluded_line(&excluded));
    let _ = writeln!(out, "{}", incompat_line(&excluded));
    let _ = writeln!(out, "{thin}");
    let _ = writeln!(out, "Category Breakdown (pass rate):");
    for (category, t) in tally_by(rows, |r| r.category()) {
        let _ = writeln!(
            out,
            "  • {:<14}: {:>5.1}% ({}/{}), skipped {}, unsupported {}, wrong {}",
            format!("{category}/*"),
            pct(t.passed(), t.total),
            t.passed(),
            t.total,
            t.skipped(),
            t.unsupported(),
            t.wrong()
        );
    }
    let _ = writeln!(out, "{thin}");
    let _ = writeln!(
        out,
        "Full report (per feature file, fix backlog): {report_path}"
    );
    let _ = writeln!(out, "{rule}");
    out
}

/// A Markdown code span that survives backticks in the text.
fn code(text: &str) -> String {
    if text.contains('`') {
        format!("`` {text} ``")
    } else {
        format!("`{text}`")
    }
}

/// Classes of the `Unsupported` group: the step, overload, token or literal does not exist.
const UNSUPPORTED_CLASSES: [Class; 2] = [Class::Translate, Class::Missing];

/// Classes of the `Wrong behaviour` group: executes, but differs from the specification or fails.
const WRONG_CLASSES: [Class; 5] = [
    Class::Runtime,
    Class::WrongResult,
    Class::WrongError,
    Class::Timeout,
    Class::Panic,
];

/// `translate 72, missing 509` for the classes of one group.
fn group_classes(t: &Tally, classes: &[Class]) -> String {
    classes
        .iter()
        .map(|c| format!("{} {}", c.name(), t.get(*c)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn tally_table(out: &mut String, first: &str, groups: &BTreeMap<&str, Tally>, total: &Tally) {
    let cells = |t: &Tally| {
        [t.skipped(), t.unsupported(), t.wrong()]
            .iter()
            .map(|n| match n {
                0 => " · |".to_string(),
                n => format!(" {n} |"),
            })
            .collect::<String>()
    };
    let _ = writeln!(
        out,
        "| {first} | Pass % | Passed | Total | Skipped | Unsupported | Wrong |"
    );
    let _ = writeln!(out, "|---|---:|---:|---:|---:|---:|---:|");
    for (name, t) in groups {
        let _ = writeln!(
            out,
            "| {} | {:.1} % | {} | {} |{}",
            code(name),
            pct(t.passed(), t.total),
            t.passed(),
            t.total,
            cells(t)
        );
    }
    let _ = writeln!(
        out,
        "| **all** | **{:.1} %** | **{}** | **{}** |{}",
        pct(total.passed(), total.total),
        total.passed(),
        total.total,
        cells(total)
    );
}

fn id_list(out: &mut String, rows: &[&Row<'_>], with_detail: bool) {
    for row in rows {
        if with_detail {
            let _ = writeln!(out, "- {}: {}", code(row.id), code(&row.outcome.detail));
        } else {
            let _ = writeln!(out, "- {}", code(row.id));
        }
    }
}

/// Writes ranked groups; long id lists are collapsed into `<details>`.
fn grouped_section(
    out: &mut String,
    groups: &[(String, Vec<&Row<'_>>)],
    with_detail: bool,
    collapse_over: usize,
) {
    for (key, rows) in groups {
        let noun = if rows.len() == 1 {
            "scenario"
        } else {
            "scenarios"
        };
        let _ = writeln!(out, "#### {} — {} {noun}\n", code(key), rows.len());
        if rows.len() > collapse_over {
            let _ = writeln!(
                out,
                "<details><summary>{} scenarios</summary>\n",
                rows.len()
            );
            id_list(out, rows, with_detail);
            let _ = writeln!(out, "\n</details>\n");
        } else {
            id_list(out, rows, with_detail);
            out.push('\n');
        }
    }
}

/// The "Excluded from compatibility scope" section: counts per reason and strategy, collapsed ids.
fn excluded_section(out: &mut String, excluded: &[Row<'_>]) {
    let _ = writeln!(out, "## Excluded from compatibility scope\n");
    let _ = writeln!(
        out,
        "{}. Not run and left out of every total and percentage in this report; a feature file whose \
         scenarios are all excluded is not listed in the per-file table.\n",
        excluded_line(excluded)
            .split(" [")
            .next()
            .unwrap_or_default()
    );
    let groups = excluded_groups(excluded);
    if groups.is_empty() {
        return;
    }
    let _ = writeln!(out, "| Reason | Strategy | Scenarios |\n|---|---|---:|");
    for (reason, group) in &groups {
        for (strategy, rs) in group {
            let _ = writeln!(out, "| {reason} | {} | {} |", code(strategy), rs.len());
        }
    }
    out.push('\n');
    for (_, group) in &groups {
        for (strategy, rs) in group {
            let _ = writeln!(
                out,
                "<details><summary>{} — {} scenarios</summary>\n",
                code(strategy),
                rs.len()
            );
            id_list(out, rs, false);
            let _ = writeln!(out, "\n</details>\n");
        }
    }
}

/// "Where" cell of an incompatibility: the distinct feature files of its scenarios.
fn where_cell(rows: &[&Row<'_>]) -> String {
    let mut files: Vec<&str> = Vec::new();
    for row in rows {
        let file = row.file();
        let name = file.rsplit('/').next().unwrap_or(file);
        let name = name.strip_suffix(".feature").unwrap_or(name);
        if !files.contains(&name) {
            files.push(name);
        }
    }
    files.sort_unstable();
    files.join(", ")
}

/// The Markdown table of the deliberate incompatibilities (one row per code, table order, a
/// total row), shared by the report and the book. `link_prefix` is put before each doc path (the
/// paths are relative to `book/src/`; the book page lives one directory deeper and passes `../`).
pub fn incompat_table(excluded: &[Row<'_>], link_prefix: &str) -> String {
    let mut out = String::from(
        "| Code | Where (feature files) | Why | Docs | Scenarios |\n|---|---|---|---|---:|\n",
    );
    for (i, rows) in incompat_counts(excluded) {
        let _ = writeln!(
            out,
            "| {} | {} | {} | [{}]({link_prefix}{}) | {} |",
            code(i.code),
            where_cell(&rows),
            i.why.replace('|', "\\|"),
            i.doc,
            i.doc,
            rows.len()
        );
    }
    let _ = writeln!(
        out,
        "| **all** | | | | **{}** |",
        incompat_rows(excluded).len()
    );
    out
}

/// The "Deliberate incompatibilities" section: the table, then the collapsed id list per code.
fn incompat_section(out: &mut String, excluded: &[Row<'_>]) {
    let groups = incompat_counts(excluded);
    let _ = writeln!(out, "## Deliberate incompatibilities\n");
    let _ = writeln!(
        out,
        "{} scenarios. Not run, left out of every total and percentage. Where and why Graphersal \
         differs from TinkerPop.\n",
        incompat_rows(excluded).len()
    );
    if groups.is_empty() {
        return;
    }
    out.push_str(&incompat_table(excluded, ""));
    out.push('\n');
    for (i, rows) in &groups {
        let _ = writeln!(
            out,
            "<details><summary>{} — {} scenarios</summary>\n",
            code(i.code),
            rows.len()
        );
        id_list(out, rows, false);
        let _ = writeln!(out, "\n</details>\n");
    }
}

/// The headline table and the per-group class line (body of the report's "Headline" section and
/// of the book's `headline` block): `all` is the tally of the in-scope rows, `total` their count.
fn headline_block(all: &Tally, total: usize) -> String {
    let mut out = String::new();
    let executed = total - all.skipped();
    let _ = writeln!(out, "| | Scenarios | Share |\n|---|---:|---:|");
    let _ = writeln!(
        out,
        "| **Passed** | **{}** / {total} | **{:.1} %** |",
        all.passed(),
        pct(all.passed(), total)
    );
    let _ = writeln!(
        out,
        "| Skipped | {} / {total} | {:.1} % |",
        all.skipped(),
        pct(all.skipped(), total)
    );
    let _ = writeln!(
        out,
        "| Unsupported (not implemented) | {} / {total} | {:.1} % |",
        all.unsupported(),
        pct(all.unsupported(), total)
    );
    let _ = writeln!(
        out,
        "| **Wrong behaviour of supported steps** | **{}** / {total} | **{:.1} %** |",
        all.wrong(),
        pct(all.wrong(), total)
    );
    let _ = writeln!(
        out,
        "| Passed of executed (non-skipped) | {} / {executed} | {:.1} % |\n",
        all.passed(),
        pct(all.passed(), executed)
    );
    let _ = writeln!(
        out,
        "Unsupported = {}. Wrong behaviour = {}.\n",
        group_classes(all, &UNSUPPORTED_CLASSES),
        group_classes(all, &WRONG_CLASSES)
    );
    out
}

/// Markdown of the book block `headline`: the headline table and the class line.
pub fn headline_markdown(rows: &[Row<'_>]) -> String {
    let (in_scope, _) = split_scope(rows);
    let all = tally(&in_scope);
    headline_block(&all, all.total)
}

/// Markdown of the book block `categories`: the per-category table.
pub fn categories_markdown(rows: &[Row<'_>]) -> String {
    let (in_scope, _) = split_scope(rows);
    let mut out = String::new();
    tally_table(
        &mut out,
        "Category",
        &tally_by(&in_scope, |r| r.category()),
        &tally(&in_scope),
    );
    out
}

/// Markdown of the book block `scope`: the scope in one line.
pub fn scope_markdown(rows: &[Row<'_>]) -> String {
    let (in_scope, excluded) = split_scope(rows);
    format!("{}.\n", scope_line(in_scope.len(), &excluded))
}

/// Markdown of the book block `excluded`: the strategy exclusions in one line.
pub fn excluded_markdown(rows: &[Row<'_>]) -> String {
    let (_, excluded) = split_scope(rows);
    format!("{}\n", excluded_line(&excluded))
}

/// Markdown of the book block `incompat`: the deliberate-incompatibility table.
pub fn incompat_markdown(rows: &[Row<'_>]) -> String {
    let (_, excluded) = split_scope(rows);
    incompat_table(&excluded, "../")
}

/// The full Markdown report.
///
/// `gate` is the comparison with the baseline as it was before this run (`None` when there was no
/// baseline); `baseline_note` says how the baseline was used (checked or regenerated);
/// `inventory` is the body of the last section, [`INVENTORY_HEADING`] (the API inventory check).
pub fn markdown(
    rows: &[Row<'_>],
    gate: Option<&GateResult>,
    baseline_note: &str,
    inventory: &str,
) -> String {
    let (in_scope, excluded) = split_scope(rows);
    let rows = &in_scope[..];
    let all = tally(rows);
    let total = all.total;
    let files = tally_by(rows, |r| r.file());
    let mut out = String::new();
    let _ = writeln!(out, "# TinkerPop feature suite report\n");
    let _ = writeln!(
        out,
        "Apache TinkerPop {SUITE_VERSION} Gherkin feature suite ({} feature files, {total} in-scope \
         scenarios; {} excluded from the compatibility scope) run against Graphersal's Rhai DSL. \
         Generated by\n\
         `cargo test --package graphersal --features script,io,display,serde --test tinkerpop_features`.\n\
         This file is the fix backlog: each ranked group below is a candidate task.\n\n\
         Scope: {}.\n",
        files.len(),
        excluded.len(),
        scope_line(total, &excluded)
    );

    // Panics and timeouts first: a panic is a rule-1 bug.
    let urgent: Vec<&Row<'_>> = rows
        .iter()
        .filter(|r| matches!(r.class(), Class::Panic | Class::Timeout))
        .collect();
    if !urgent.is_empty() {
        let _ = writeln!(out, "## Panics and timeouts (top priority)\n");
        let _ = writeln!(
            out,
            "A panic violates CLAUDE.md rule 1 (no panics in step or script paths).\n"
        );
        for row in &urgent {
            let _ = writeln!(
                out,
                "- **{}** {}: {}",
                row.class().name(),
                code(row.id),
                code(&row.outcome.detail)
            );
        }
        out.push('\n');
    }

    let relaxed: Vec<&Row<'_>> = rows.iter().filter(|r| r.outcome.order_relaxed).collect();
    let _ = writeln!(out, "## Headline\n");
    out.push_str(&headline_block(&all, total));
    let _ = writeln!(
        out,
        "Order-relaxed passes (included in Passed): {}. Baseline: {baseline_note}\n",
        relaxed.len()
    );
    excluded_section(&mut out, &excluded);
    incompat_section(&mut out, &excluded);

    let _ = writeln!(out, "## By category\n");
    tally_table(
        &mut out,
        "Category",
        &tally_by(rows, |r| r.category()),
        &all,
    );
    out.push('\n');

    let _ = writeln!(out, "## By feature file\n");
    tally_table(&mut out, "Feature file", &files, &all);
    out.push('\n');

    let _ = writeln!(out, "## Failures by class\n");
    let _ = writeln!(out, "| Class | Scenarios | Share of all |\n|---|---:|---:|");
    for class in Class::ALL.into_iter().filter(|c| *c != Class::Pass) {
        let _ = writeln!(
            out,
            "| {} | {} | {:.1} % |",
            class.name(),
            all.get(class),
            pct(all.get(class), total)
        );
    }
    out.push('\n');

    let of_class =
        |class: Class| -> Vec<&Row<'_>> { rows.iter().filter(|r| r.class() == class).collect() };
    let step_key = |r: &Row<'_>| {
        r.outcome
            .exercised_step
            .clone()
            .unwrap_or_else(|| "?".to_string())
    };

    let missing = of_class(Class::Missing);
    let _ = writeln!(out, "### missing — {} scenarios\n", missing.len());
    let _ = writeln!(
        out,
        "Rhai `Function/Variable not found`, grouped by the missing name and ranked by how many \
         scenarios it blocks.\n"
    );
    let groups = ranked(&missing, |r| missing_name(&r.outcome.detail).to_string());
    let _ = writeln!(
        out,
        "| # | Missing name | Scenarios blocked |\n|---:|---|---:|"
    );
    for (i, (name, rs)) in groups.iter().enumerate() {
        let _ = writeln!(out, "| {} | {} | {} |", i + 1, code(name), rs.len());
    }
    out.push('\n');
    grouped_section(&mut out, &groups, false, 5);

    for (class, blurb) in [
        (
            Class::Runtime,
            "A `TraverserError` or script error while executing, grouped by the step the scenario exercises.",
        ),
        (
            Class::WrongResult,
            "Executed, but the result differs from upstream, grouped by the step the scenario exercises.",
        ),
        (
            Class::WrongError,
            "An error was expected but values came back, or the error text differs.",
        ),
    ] {
        let rs = of_class(class);
        let _ = writeln!(
            out,
            "### {} — {} scenarios\n\n{blurb}\n",
            class.name(),
            rs.len()
        );
        grouped_section(&mut out, &ranked(&rs, step_key), true, 10);
    }

    for (class, blurb) in [
        (
            Class::Translate,
            "The translator or the runner cannot express the scenario, grouped by reason.",
        ),
        (Class::Skipped, "Not run, grouped by reason."),
    ] {
        let rs = of_class(class);
        let _ = writeln!(
            out,
            "### {} — {} scenarios\n\n{blurb}\n",
            class.name(),
            rs.len()
        );
        grouped_section(
            &mut out,
            &ranked(&rs, |r| r.outcome.detail.clone()),
            false,
            5,
        );
    }

    for class in [Class::Timeout, Class::Panic] {
        let rs = of_class(class);
        let _ = writeln!(out, "### {} — {} scenarios\n", class.name(), rs.len());
        id_list(&mut out, &rs, true);
        out.push('\n');
    }

    let _ = writeln!(out, "## Class changed since the baseline\n");
    match gate {
        None => {
            let _ = writeln!(out, "No baseline to compare with.\n");
        }
        Some(g) if g.class_changed.is_empty() => {
            let _ = writeln!(out, "None.\n");
        }
        Some(g) => {
            let _ = writeln!(
                out,
                "Still failing, but under another class than recorded (does not fail the gate; \
                 regenerate the baseline to record the new class).\n"
            );
            let _ = writeln!(
                out,
                "| Scenario | Recorded | Now | Detail |\n|---|---|---|---|"
            );
            for c in &g.class_changed {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    code(&c.id),
                    c.recorded,
                    c.current,
                    code(&c.detail).replace('|', "\\|")
                );
            }
            out.push('\n');
        }
    }

    if let Some(g) = gate.filter(|g| g.failed()) {
        let _ = writeln!(
            out,
            "## Baseline gate failures\n\n```text\n{}```\n",
            g.failure_message()
        );
    }

    let _ = writeln!(out, "## Order-relaxed passes (informational)\n");
    if relaxed.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(
            out,
            "Ordered assertions that passed only because the traversal has no ordering step and \
             the result is the expected multiset.\n"
        );
        id_list(&mut out, &relaxed, false);
        out.push('\n');
    }

    let _ = writeln!(out, "{INVENTORY_HEADING}\n");
    out.push_str(inventory);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(class: Class, detail: &str, step: &str) -> Outcome {
        Outcome {
            class,
            detail: detail.to_string(),
            order_relaxed: false,
            exercised_step: Some(step.to_string()),
        }
    }

    #[test]
    fn summary_and_markdown_shape() {
        let outcomes = [
            ("map/Fold.feature::a", outcome(Class::Pass, "", "fold")),
            (
                "map/Fold.feature::b",
                outcome(Class::WrongResult, "diff", "fold"),
            ),
            (
                "map/Sack.feature::c",
                outcome(Class::Missing, "sack", "sack"),
            ),
            (
                "filter/Has.feature::d",
                outcome(Class::Missing, "initializer: sack", "has"),
            ),
            (
                "filter/Has.feature::e",
                outcome(Class::Timeout, "timeout", "has"),
            ),
            (
                "branch/X.feature::f",
                outcome(Class::Skipped, "@GraphComputerOnly", "x"),
            ),
            (
                "integrated/SubgraphStrategy.feature::g",
                outcome(Class::Excluded, "SubgraphStrategy", "subgraphStrategy"),
            ),
            (
                "map/Fold.feature::h",
                outcome(Class::Excluded, "PartitionStrategy", "fold"),
            ),
            (
                "integrated/HaltedTraverserStrategy.feature::i",
                outcome(Class::Excluded, "HaltedTraverserStrategy", "x"),
            ),
            (
                "map/AsString.feature::g_V_valueMapXnameX_asString",
                outcome(Class::Excluded, "incompat:map-to-string", "asString"),
            ),
            (
                "map/AsString.feature::g_V_valueMapXnameX_order_fold_asStringXlocalX",
                outcome(Class::Excluded, "incompat:map-to-string", "asString"),
            ),
            (
                "filter/Has.feature::g_V_hasXlocationX",
                outcome(Class::Excluded, "incompat:multi-meta-properties", "has"),
            ),
        ];
        let rows: Vec<Row<'_>> = outcomes
            .iter()
            .map(|(id, o)| Row { id, outcome: o })
            .collect();

        let s = summary(&rows, "target/tinkerpop-report.md");
        assert!(s.contains("Passed Scenarios:      1 / 6  (16.67%)"), "{s}");
        assert!(s.contains("[@GraphComputerOnly: 1]"), "{s}");
        assert!(
            s.contains("Unsupported:") && s.contains("Wrong behaviour:"),
            "{s}"
        );
        assert!(s.contains("map/*"), "{s}");
        assert!(s.contains("33.3% (1/3)"), "{s}");
        // Excluded scenarios are in no denominator and no category.
        assert!(
            s.contains("Pass rate of executed (non-skipped) scenarios: 20.00% (1/5)"),
            "{s}"
        );
        assert!(!s.contains("integrated/*"), "{s}");
        // The excluded count of the strategy line leaves out the other family.
        assert!(
            s.contains(
                "6 in-scope scenarios; 3 excluded (strategies), 3 excluded (deliberate incompatibilities)"
            ),
            "{s}"
        );
        assert!(
            s.contains(
                "Deliberate incompatibilities: 3 scenarios [multi-meta-properties 1, map-to-string 2]"
            ),
            "{s}"
        );
        assert!(
            s.contains(&format!(
                "Excluded from compatibility scope: 3 scenarios [{}: 2 (SubgraphStrategy 1, PartitionStrategy 1); {}: 1 (HaltedTraverserStrategy 1)]",
                scope::REASON_ACCESS_CONTROL,
                scope::REASON_OLAP
            )),
            "{s}"
        );

        let md = markdown(&rows, None, "none", "inventory body");
        let panic_pos = md.find("## Panics and timeouts").unwrap_or(usize::MAX);
        assert!(panic_pos < md.find("## Headline").unwrap_or(0), "{md}");
        assert!(md.contains("#### `sack` — 2 scenarios"), "{md}");
        assert!(
            md.contains("| `map/Fold.feature` | 50.0 % | 1 | 2 |"),
            "{md}"
        );
        assert!(
            md.contains("| **Passed** | **1** / 6 | **16.7 %** |"),
            "{md}"
        );
        assert!(
            md.contains("| Unsupported (not implemented) |")
                && md.contains("| **Wrong behaviour of supported steps** |")
                && md.contains("| Skipped | Unsupported | Wrong |"),
            "{md}"
        );
        assert!(md.contains("## Excluded from compatibility scope"), "{md}");
        assert!(md.contains("| `SubgraphStrategy` | 1 |"), "{md}");
        // Deliberate incompatibilities: own table, counts per code, no denominator.
        assert!(md.contains("## Deliberate incompatibilities"), "{md}");
        assert!(
            md.contains("| `map-to-string` | AsString | ") && md.contains(" | 2 |\n"),
            "{md}"
        );
        assert!(md.contains("| `multi-meta-properties` | Has |"), "{md}");
        assert!(md.contains("| **all** | | | | **3** |"), "{md}");
        assert!(!md.contains("| `map/AsString.feature`"), "{md}");
        assert!(
            md.contains("`map/AsString.feature::g_V_valueMapXnameX_asString`"),
            "{md}"
        );
        assert!(!md.contains("| `integrated/"), "{md}");
        assert!(
            md.ends_with(&format!("{INVENTORY_HEADING}\n\ninventory body\n")),
            "{md}"
        );
        assert_eq!(
            md,
            markdown(&rows, None, "none", "inventory body"),
            "deterministic"
        );
        assert_eq!(missing_counts(&rows).get("sack"), Some(&2));
    }

    #[test]
    fn code_spans_survive_backticks() {
        assert_eq!(code("a`b"), "`` a`b ``");
        assert_eq!(code("ab"), "`ab`");
    }
}
