//! Hand-written parser for the Gherkin subset used by the TinkerPop feature suite.
//!
//! It turns a `.feature` file into a typed [`Feature`] with one [`Scenario`] per `Scenario:` block.
//! Only the phrases the vendored suite actually uses are accepted; any other step phrase is a
//! [`ParseError`] with the file and line, never silently skipped.
//!
//! Notes on the model (decisions recorded here, as the spec asks):
//! - Tags are stored verbatim, including the leading `@` (e.g. `"@GraphComputerOnly"`). A
//!   scenario's `tags` are its *effective* tags: feature tags followed by its own, deduplicated,
//!   in first-seen order.
//! - Lines starting with `#` (after trimming) are comments and ignored outside doc strings; the
//!   suite contains one fully commented-out scenario (`sideEffect/Group.feature`), which therefore
//!   does not exist in the model.
//! - The step keyword (`Given`/`When`/`Then`/`And`/`But`) is not checked against the phrase:
//!   the suite uses `Then` and `And` interchangeably for assertions.
//! - `Then the result should be a subgraph with the following` appears as two consecutive steps in
//!   the suite, one with an `| edges |` table and one with a `| vertices |` table. Both merge into a
//!   single [`Assertion::Subgraph`].
//! - Comment lines may sit between a step and its opening `"""` (seen in
//!   `sideEffect/Aggregate.feature`); they are skipped.
//! - Doc strings drop their common leading indentation and each line's trailing whitespace; the
//!   lines are joined with `\n` without a trailing newline.
//! - Table rows are single-column in the whole suite; a row with more than one cell is an error.
//!   Cells are kept as raw trimmed text (the expected-value notation is parsed by `notation.rs`).
//! - Quoted step arguments (parameter/side-effect notation, `count of "<traversal>"`, error
//!   message text) are unescaped: `\"` becomes `"`, `\\` becomes `\`; any other backslash
//!   sequence is kept verbatim.
//! - The scenario id is `"<path>::<name>"`; if a file repeats a scenario name, the later
//!   occurrence gets `#<line>` appended (the vendored 3.8.2 suite has no such duplicates).

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

/// One parsed `.feature` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    /// Path relative to the `features/` root, with `/` separators (e.g. `"branch/Repeat.feature"`).
    pub path: String,
    /// Text after `Feature:`.
    pub name: String,
    /// Tags on the lines preceding `Feature:`, including the leading `@`.
    pub tags: Vec<String>,
    /// Scenarios in file order.
    pub scenarios: Vec<Scenario>,
}

/// One `Scenario:` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    /// Stable id: `"<path>::<scenario name>"`.
    pub id: String,
    /// Text after `Scenario:` (e.g. `g_V_outXcreatedX_name`).
    pub name: String,
    /// 1-based line number of the `Scenario:` line.
    pub line: usize,
    /// Effective tags: feature tags ∪ scenario tags.
    pub tags: Vec<String>,
    /// The dataset the scenario starts from.
    pub graph: GraphSetup,
    /// Mutating setup traversal from `And the graph initializer of`.
    pub initializer: Option<String>,
    /// `And using the parameter <name> defined as "<notation>"`, in file order, unescaped.
    pub params: Vec<(String, String)>,
    /// `And using the side effect <name> defined as "<notation>"`, in file order, unescaped.
    pub side_effects: Vec<(String, String)>,
    /// The canonical Gremlin traversal from `And the traversal of`.
    pub traversal: Option<String>,
    /// How the traversal is iterated.
    pub iteration: Option<Iteration>,
    /// Assertions in file order.
    pub assertions: Vec<Assertion>,
}

/// The graph a scenario starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphSetup {
    /// `Given the modern graph`.
    Modern,
    /// `Given the crew graph`.
    Crew,
    /// `Given the sink graph`.
    Sink,
    /// `Given the grateful graph`.
    Grateful,
    /// `Given the empty graph`.
    Empty,
    /// `Given an unsupported test`.
    Unsupported,
}

/// How the traversal result is collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Iteration {
    /// `When iterated to list`.
    ToList,
    /// `When iterated next`.
    Next,
}

/// One `Then`/`And` assertion.
#[derive(Debug, Clone, PartialEq)]
pub enum Assertion {
    /// `the result should be ordered` + table (raw cells, header removed).
    Ordered(Vec<String>),
    /// `the result should be unordered` + table.
    Unordered(Vec<String>),
    /// `the result should be of` + table: every result is one of the listed values.
    Of(Vec<String>),
    /// `the result should be empty`.
    Empty,
    /// `the result should have a count of N`.
    Count(usize),
    /// `the graph should return N for count of "<traversal>"`.
    GraphCount {
        /// Expected count.
        count: usize,
        /// Unescaped traversal text.
        traversal: String,
    },
    /// `the traversal will raise an error [with message containing text of "<text>"]`.
    RaisesError {
        /// Expected message substring, if any. Parsed but not compared: an expected error is
        /// matched by type only (see `classify.rs`).
        message: Option<String>,
    },
    /// `nothing should happen because` + doc string (the reason).
    NothingHappens(String),
    /// `the result should be a tree with a structure of` + doc string.
    Tree(String),
    /// `the result should be a subgraph with the following` + `| edges |`/`| vertices |` tables.
    Subgraph {
        /// Raw edge cells.
        edges: Vec<String>,
        /// Raw vertex cells.
        vertices: Vec<String>,
    },
}

/// A parse failure with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Feature path (relative to `features/`) or the file system path for I/O errors.
    pub file: String,
    /// 1-based line number, `0` when the error is not tied to a line.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Loads and parses every `*.feature` file below `features_root`, in sorted path order.
pub fn load_all(features_root: &Path) -> Result<Vec<Feature>, ParseError> {
    let mut files = Vec::new();
    collect_feature_files(features_root, &mut files)?;
    let mut rel_paths: Vec<(String, std::path::PathBuf)> = files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(features_root)
                .unwrap_or(&p)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            (rel, p)
        })
        .collect();
    rel_paths.sort();
    rel_paths
        .into_iter()
        .map(|(rel, full)| {
            let text = std::fs::read_to_string(&full).map_err(|e| ParseError {
                file: rel.clone(),
                line: 0,
                message: format!("cannot read file: {e}"),
            })?;
            parse_feature(&rel, &text)
        })
        .collect()
}

fn collect_feature_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), ParseError> {
    let io_err = |e: std::io::Error| ParseError {
        file: dir.display().to_string(),
        line: 0,
        message: format!("cannot read directory: {e}"),
    };
    for entry in std::fs::read_dir(dir).map_err(io_err)? {
        let path = entry.map_err(io_err)?.path();
        if path.is_dir() {
            collect_feature_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "feature") {
            out.push(path);
        }
    }
    Ok(())
}

/// Scenario under construction; `graph` is optional until the scenario ends.
struct Draft {
    name: String,
    line: usize,
    tags: Vec<String>,
    graph: Option<GraphSetup>,
    initializer: Option<String>,
    params: Vec<(String, String)>,
    side_effects: Vec<(String, String)>,
    traversal: Option<String>,
    iteration: Option<Iteration>,
    assertions: Vec<Assertion>,
}

struct Parser<'a> {
    path: &'a str,
    lines: Vec<&'a str>,
    /// Index of the next line to read (0-based).
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, line: usize, message: impl Into<String>) -> ParseError {
        ParseError {
            file: self.path.to_string(),
            line,
            message: message.into(),
        }
    }

    /// Reads a `"""` doc string that must start on the next line. `step_line` is 1-based.
    /// Comment lines between the step and the opening `"""` are skipped (the suite has one in
    /// `sideEffect/Aggregate.feature`).
    fn doc_string(&mut self, step_line: usize) -> Result<String, ParseError> {
        while self
            .lines
            .get(self.pos)
            .is_some_and(|l| l.trim().starts_with('#'))
        {
            self.pos += 1;
        }
        let open = self.lines.get(self.pos).map(|l| l.trim());
        if open != Some("\"\"\"") {
            return Err(self.err(step_line, "expected a `\"\"\"` doc string after this step"));
        }
        self.pos += 1;
        let mut body = Vec::new();
        loop {
            let Some(line) = self.lines.get(self.pos) else {
                return Err(self.err(step_line, "unterminated `\"\"\"` doc string"));
            };
            self.pos += 1;
            if line.trim() == "\"\"\"" {
                break;
            }
            body.push(*line);
        }
        Ok(strip_common_indent(&body))
    }

    /// Reads the `| ... |` table rows that follow a step; returns (header, rows).
    fn table(&mut self, step_line: usize) -> Result<(String, Vec<String>), ParseError> {
        let mut cells = Vec::new();
        while let Some(line) = self.lines.get(self.pos) {
            let t = line.trim();
            if !t.starts_with('|') {
                break;
            }
            let line_no = self.pos + 1;
            self.pos += 1;
            let inner = t
                .strip_prefix('|')
                .and_then(|s| s.strip_suffix('|'))
                .ok_or_else(|| self.err(line_no, "table row must start and end with `|`"))?;
            if inner.contains('|') {
                return Err(self.err(line_no, "multi-column table rows are not supported"));
            }
            cells.push(inner.trim().to_string());
        }
        if cells.is_empty() {
            return Err(self.err(step_line, "expected a table after this step"));
        }
        let header = cells.remove(0);
        Ok((header, cells))
    }
}

/// Parses one `.feature` file. `path` is the file path relative to `features/`.
pub fn parse_feature(path: &str, text: &str) -> Result<Feature, ParseError> {
    let mut p = Parser {
        path,
        lines: text.lines().collect(),
        pos: 0,
    };
    let mut pending_tags: Vec<String> = Vec::new();
    let mut feature: Option<(String, Vec<String>)> = None;
    let mut scenarios: Vec<Scenario> = Vec::new();
    let mut draft: Option<Draft> = None;
    let mut seen_names: HashSet<String> = HashSet::new();

    while p.pos < p.lines.len() {
        let line_no = p.pos + 1;
        let t = p.lines[p.pos].trim();
        p.pos += 1;
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t.starts_with('@') {
            for tag in t.split_whitespace() {
                if !tag.starts_with('@') || tag.len() == 1 {
                    return Err(p.err(line_no, format!("malformed tag `{tag}`")));
                }
                pending_tags.push(tag.to_string());
            }
            continue;
        }
        if let Some(name) = t.strip_prefix("Feature:") {
            if feature.is_some() {
                return Err(p.err(line_no, "second `Feature:` in one file"));
            }
            feature = Some((name.trim().to_string(), std::mem::take(&mut pending_tags)));
            continue;
        }
        if let Some(name) = t.strip_prefix("Scenario:") {
            let Some((_, feature_tags)) = &feature else {
                return Err(p.err(line_no, "`Scenario:` before `Feature:`"));
            };
            if let Some(d) = draft.take() {
                scenarios.push(finish(&p, d, &mut seen_names)?);
            }
            let mut tags = feature_tags.clone();
            for tag in std::mem::take(&mut pending_tags) {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
            draft = Some(Draft {
                name: name.trim().to_string(),
                line: line_no,
                tags,
                graph: None,
                initializer: None,
                params: Vec::new(),
                side_effects: Vec::new(),
                traversal: None,
                iteration: None,
                assertions: Vec::new(),
            });
            continue;
        }
        let Some(d) = draft.as_mut() else {
            return Err(p.err(
                line_no,
                format!("unexpected line outside a scenario: `{t}`"),
            ));
        };
        if !pending_tags.is_empty() {
            return Err(p.err(line_no, "tags must precede `Feature:` or `Scenario:`"));
        }
        let phrase = ["Given ", "When ", "Then ", "And ", "But "]
            .iter()
            .find_map(|kw| t.strip_prefix(kw))
            .ok_or_else(|| p.err(line_no, format!("expected a step keyword: `{t}`")))?
            .trim();
        step(&mut p, d, phrase, line_no)?;
    }

    if let Some(d) = draft.take() {
        scenarios.push(finish(&p, d, &mut seen_names)?);
    }
    let Some((name, tags)) = feature else {
        return Err(p.err(0, "no `Feature:` line"));
    };
    if !pending_tags.is_empty() {
        return Err(p.err(p.lines.len(), "dangling tags at end of file"));
    }
    Ok(Feature {
        path: path.to_string(),
        name,
        tags,
        scenarios,
    })
}

/// Applies one step phrase (keyword already stripped) to the scenario draft.
fn step(p: &mut Parser<'_>, d: &mut Draft, phrase: &str, line: usize) -> Result<(), ParseError> {
    let set_graph = |p: &Parser<'_>, d: &mut Draft, g: GraphSetup| {
        if d.graph.replace(g).is_some() {
            return Err(p.err(line, "graph setup given twice"));
        }
        Ok(())
    };
    let strip_colon = |s: &str| s.strip_suffix(':').unwrap_or(s).trim_end().to_string();
    let phrase_nc = strip_colon(phrase);
    match phrase_nc.as_str() {
        "the modern graph" => return set_graph(p, d, GraphSetup::Modern),
        "the crew graph" => return set_graph(p, d, GraphSetup::Crew),
        "the sink graph" => return set_graph(p, d, GraphSetup::Sink),
        "the grateful graph" => return set_graph(p, d, GraphSetup::Grateful),
        "the empty graph" => return set_graph(p, d, GraphSetup::Empty),
        "an unsupported test" => return set_graph(p, d, GraphSetup::Unsupported),
        "the graph initializer of" => {
            let body = p.doc_string(line)?;
            if d.initializer.replace(body).is_some() {
                return Err(p.err(line, "graph initializer given twice"));
            }
            return Ok(());
        }
        "the traversal of" => {
            let body = p.doc_string(line)?;
            if d.traversal.replace(body).is_some() {
                return Err(p.err(line, "traversal given twice"));
            }
            return Ok(());
        }
        "iterated to list" | "iterated next" => {
            let it = if phrase_nc == "iterated next" {
                Iteration::Next
            } else {
                Iteration::ToList
            };
            if d.iteration.replace(it).is_some() {
                return Err(p.err(line, "iteration given twice"));
            }
            return Ok(());
        }
        "the result should be ordered" => {
            let (_, rows) = p.table(line)?;
            d.assertions.push(Assertion::Ordered(rows));
            return Ok(());
        }
        "the result should be unordered" => {
            let (_, rows) = p.table(line)?;
            d.assertions.push(Assertion::Unordered(rows));
            return Ok(());
        }
        "the result should be of" => {
            let (_, rows) = p.table(line)?;
            d.assertions.push(Assertion::Of(rows));
            return Ok(());
        }
        "the result should be empty" => {
            d.assertions.push(Assertion::Empty);
            return Ok(());
        }
        "the traversal will raise an error" => {
            d.assertions.push(Assertion::RaisesError { message: None });
            return Ok(());
        }
        "nothing should happen because" => {
            let body = p.doc_string(line)?;
            d.assertions.push(Assertion::NothingHappens(body));
            return Ok(());
        }
        "the result should be a tree with a structure of" => {
            let body = p.doc_string(line)?;
            d.assertions.push(Assertion::Tree(body));
            return Ok(());
        }
        "the result should be a subgraph with the following" => {
            let (header, rows) = p.table(line)?;
            let existing = d.assertions.iter_mut().find_map(|a| match a {
                Assertion::Subgraph { edges, vertices } => Some((edges, vertices)),
                _ => None,
            });
            let (edges, vertices) = match existing {
                Some(ev) => ev,
                None => {
                    d.assertions.push(Assertion::Subgraph {
                        edges: Vec::new(),
                        vertices: Vec::new(),
                    });
                    match d.assertions.last_mut() {
                        Some(Assertion::Subgraph { edges, vertices }) => (edges, vertices),
                        _ => return Err(p.err(line, "internal: subgraph assertion missing")),
                    }
                }
            };
            match header.as_str() {
                "edges" => edges.extend(rows),
                "vertices" => vertices.extend(rows),
                other => {
                    return Err(p.err(
                        line,
                        format!(
                            "subgraph table header must be `edges` or `vertices`, got `{other}`"
                        ),
                    ));
                }
            }
            return Ok(());
        }
        _ => {}
    }

    if let Some(rest) = phrase.strip_prefix("the result should have a count of ") {
        let n = parse_count(p, rest.trim(), line)?;
        d.assertions.push(Assertion::Count(n));
        return Ok(());
    }
    if let Some(rest) = phrase.strip_prefix("the graph should return ") {
        let (num, rest) = rest
            .split_once(" for count of ")
            .ok_or_else(|| p.err(line, format!("unknown step phrase: `{phrase}`")))?;
        let count = parse_count(p, num.trim(), line)?;
        let traversal = quoted_to_end(p, rest, line)?;
        d.assertions
            .push(Assertion::GraphCount { count, traversal });
        return Ok(());
    }
    if let Some(rest) =
        phrase.strip_prefix("the traversal will raise an error with message containing text of ")
    {
        let message = quoted_to_end(p, rest, line)?;
        d.assertions.push(Assertion::RaisesError {
            message: Some(message),
        });
        return Ok(());
    }
    for (prefix, is_param) in [
        ("using the parameter ", true),
        ("using the side effect ", false),
    ] {
        if let Some(rest) = phrase.strip_prefix(prefix) {
            let (name, value) = rest
                .split_once(" defined as ")
                .ok_or_else(|| p.err(line, format!("expected `defined as` in `{phrase}`")))?;
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(p.err(line, format!("invalid name `{name}`")));
            }
            let value = quoted_to_end(p, value, line)?;
            let list = if is_param {
                &mut d.params
            } else {
                &mut d.side_effects
            };
            list.push((name.to_string(), value));
            return Ok(());
        }
    }
    Err(p.err(line, format!("unknown step phrase: `{phrase}`")))
}

fn parse_count(p: &Parser<'_>, s: &str, line: usize) -> Result<usize, ParseError> {
    s.parse()
        .map_err(|_| p.err(line, format!("expected a non-negative count, got `{s}`")))
}

/// Parses `"..."` (with `\"`/`\\` escapes) that must be the rest of the step line.
fn quoted_to_end(p: &Parser<'_>, s: &str, line: usize) -> Result<String, ParseError> {
    let s = s.trim();
    let inner = s
        .strip_prefix('"')
        .ok_or_else(|| p.err(line, format!("expected a double-quoted string, got `{s}`")))?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, other)) => {
                    out.push('\\');
                    out.push(other);
                }
                None => return Err(p.err(line, "unterminated escape in quoted string")),
            },
            '"' => {
                let trailing = inner[i + 1..].trim();
                if !trailing.is_empty() {
                    return Err(p.err(
                        line,
                        format!("unexpected text after quoted string: `{trailing}`"),
                    ));
                }
                return Ok(out);
            }
            other => out.push(other),
        }
    }
    Err(p.err(line, "unterminated quoted string"))
}

/// Removes the common leading whitespace and each line's trailing whitespace.
fn strip_common_indent(lines: &[&str]) -> String {
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| l.get(indent..).unwrap_or("").trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Validates a finished draft and assigns its unique id.
fn finish(p: &Parser<'_>, d: Draft, seen: &mut HashSet<String>) -> Result<Scenario, ParseError> {
    let graph = d
        .graph
        .ok_or_else(|| p.err(d.line, format!("scenario `{}` has no graph setup", d.name)))?;
    if graph != GraphSetup::Unsupported {
        if d.traversal.is_none() {
            return Err(p.err(d.line, format!("scenario `{}` has no traversal", d.name)));
        }
        if d.assertions.is_empty() {
            return Err(p.err(d.line, format!("scenario `{}` has no assertion", d.name)));
        }
    }
    let mut id = format!("{}::{}", p.path, d.name);
    if !seen.insert(id.clone()) {
        id = format!("{id}#{}", d.line);
        seen.insert(id.clone());
    }
    Ok(Scenario {
        id,
        name: d.name,
        line: d.line,
        tags: d.tags,
        graph,
        initializer: d.initializer,
        params: d.params,
        side_effects: d.side_effects,
        traversal: d.traversal,
        iteration: d.iteration,
        assertions: d.assertions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Feature {
        match parse_feature("x/T.feature", text) {
            Ok(f) => f,
            Err(e) => panic!("parse failed: {e}"),
        }
    }

    fn one(body: &str) -> Scenario {
        let text = format!("Feature: F\n\n  Scenario: s\n{body}");
        let mut f = parse(&text);
        assert_eq!(f.scenarios.len(), 1);
        f.scenarios.remove(0)
    }

    const TRAV: &str = "    Given the modern graph\n    And the traversal of\n      \"\"\"\n      g.V()\n      \"\"\"\n    When iterated to list\n";

    #[test]
    fn tags_are_inherited_and_deduplicated() {
        let f = parse(&format!(
            "# license\n@A @B\nFeature: F\n\n  @C\n  @B @D\n  Scenario: s1\n{TRAV}    Then the result should be empty\n\n  Scenario: s2\n{TRAV}    Then the result should be empty\n"
        ));
        assert_eq!(f.tags, vec!["@A", "@B"]);
        assert_eq!(f.scenarios[0].tags, vec!["@A", "@B", "@C", "@D"]);
        assert_eq!(f.scenarios[1].tags, vec!["@A", "@B"]);
        assert_eq!(f.scenarios[0].id, "x/T.feature::s1");
        assert_eq!(f.scenarios[0].line, 7);
    }

    #[test]
    fn doc_string_strips_common_indentation() {
        let s = one(
            "    Given the empty graph\n    And the graph initializer of\n      \"\"\"\n      g.addV(\"a\").\n        addV(\"b\")   \n      \"\"\"\n    And the traversal of\n        \"\"\"\n        g.V()\n        \"\"\"\n    When iterated next\n    Then the result should be a tree with a structure of\n      \"\"\"\n      |--a\n         |--b\n      \"\"\"\n",
        );
        assert_eq!(s.graph, GraphSetup::Empty);
        assert_eq!(
            s.initializer.as_deref(),
            Some("g.addV(\"a\").\n  addV(\"b\")")
        );
        assert_eq!(s.traversal.as_deref(), Some("g.V()"));
        assert_eq!(s.iteration, Some(Iteration::Next));
        assert_eq!(s.assertions, vec![Assertion::Tree("|--a\n   |--b".into())]);
    }

    #[test]
    fn table_keeps_raw_cells_with_escaped_quotes_and_nested_maps() {
        let s = one(&format!(
            "{TRAV}    Then the result should be unordered\n      | result |\n      | m[{{\"a\":\"d[1].i\", \"b\":\"l[\\\"x\\\"]\"}}] |\n      |   v[marko]   |\n"
        ));
        assert_eq!(
            s.assertions,
            vec![Assertion::Unordered(vec![
                r#"m[{"a":"d[1].i", "b":"l[\"x\"]"}]"#.to_string(),
                "v[marko]".to_string(),
            ])]
        );
    }

    #[test]
    fn parameters_and_side_effects_are_unescaped() {
        let s = one(&format!(
            "    And using the parameter xx1 defined as \"m[{{\\\"name\\\":\\\"marko\\\"}}]\"\n    And using the side effect a defined as \"l[\\\\d]\"\n{TRAV}    Then the result should be empty\n"
        ));
        assert_eq!(
            s.params,
            vec![("xx1".to_string(), r#"m[{"name":"marko"}]"#.to_string())]
        );
        assert_eq!(
            s.side_effects,
            vec![("a".to_string(), r"l[\d]".to_string())]
        );
    }

    #[test]
    fn every_assertion_form_parses() {
        let s = one(&format!(
            "{TRAV}    Then the result should be ordered\n      | result |\n      | a |\n    Then the result should be of\n      | result |\n      | b |\n    Then the result should be empty\n    Then the result should have a count of 3\n    And the graph should return 2 for count of \"g.V().has(\\\"name\\\",\\\"x\\\")\"\n    Then the traversal will raise an error\n    Then the traversal will raise an error with message containing text of \"The \\\"x\\\" failed\"\n    Then the result should be a subgraph with the following\n      | edges |\n      | e[a-k->b] |\n    And the result should be a subgraph with the following\n      | vertices |\n      | v[a] |\n      | v[b] |\n"
        ));
        assert_eq!(
            s.assertions,
            vec![
                Assertion::Ordered(vec!["a".into()]),
                Assertion::Of(vec!["b".into()]),
                Assertion::Empty,
                Assertion::Count(3),
                Assertion::GraphCount {
                    count: 2,
                    traversal: r#"g.V().has("name","x")"#.into()
                },
                Assertion::RaisesError { message: None },
                Assertion::RaisesError {
                    message: Some(r#"The "x" failed"#.into())
                },
                Assertion::Subgraph {
                    edges: vec!["e[a-k->b]".into()],
                    vertices: vec!["v[a]".into(), "v[b]".into()],
                },
            ]
        );
    }

    #[test]
    fn unsupported_test_with_reason() {
        let s = one(
            "    Given an unsupported test\n    Then nothing should happen because\n      \"\"\"\n      Not testable.\n      \"\"\"\n",
        );
        assert_eq!(s.graph, GraphSetup::Unsupported);
        assert_eq!(s.traversal, None);
        assert_eq!(
            s.assertions,
            vec![Assertion::NothingHappens("Not testable.".into())]
        );
    }

    #[test]
    fn unknown_phrase_is_an_error_with_line() {
        let text =
            format!("Feature: F\n\n  Scenario: s\n{TRAV}    Then the result should be purple\n");
        let err = parse_feature("x/T.feature", &text).err();
        let Some(err) = err else {
            panic!("expected an error")
        };
        assert_eq!(err.file, "x/T.feature");
        assert_eq!(err.line, 10);
        assert!(
            err.message.contains("unknown step phrase"),
            "{}",
            err.message
        );
    }

    #[test]
    fn duplicate_scenario_names_get_line_suffix() {
        let f = parse(&format!(
            "Feature: F\n  Scenario: s\n{TRAV}    Then the result should be empty\n  Scenario: s\n{TRAV}    Then the result should be empty\n"
        ));
        assert_eq!(f.scenarios[0].id, "x/T.feature::s");
        assert_eq!(f.scenarios[1].id, "x/T.feature::s#10");
    }
}
