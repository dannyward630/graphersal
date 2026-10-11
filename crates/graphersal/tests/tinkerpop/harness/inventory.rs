//! API inventory check: which parts of TinkerPop's traversal API are not registered in the
//! Graphersal Rhai engine at all. Informational only — never a ratchet.
//!
//! # Inventory file
//!
//! `tests/tinkerpop/api-<version>.txt`, generated from the upstream grammar by
//! `tests/tinkerpop/gen_api_inventory.py` (see `tests/tinkerpop/README.md`). Plain text: `#`
//! comment lines, blank lines, `[section]` headers and one entry per line, sorted and unique within
//! each section. Method sections ([`METHOD_SECTIONS`]) hold bare method names; the
//! [`TOKENS_SECTION`] holds `Class.member` entries. Anything else is a parse error.
//!
//! # What counts as registered
//!
//! - A method is registered if the engine has a function under the TinkerPop spelling or its
//!   snake_case form ([`spellings`]: `hasLabel`/`has_label`, `V`/`v`) whose receiver (first
//!   parameter) is the traversal type — the Rhai type of the scope's `g`. `g` and `__` share that
//!   type, so the check cannot tell source methods from traversal methods; a bare global function of
//!   the same name (such as the predicate shortcut `eq(..)`) does not count.
//! - A token class is registered if the DSL scope (`graph_scope`) holds a global of that name.
//! - A member of a registered class is registered if a property getter or a method of either
//!   spelling exists on the global's Rhai type (`T.id` → `get$id(_: &mut TTokens)`,
//!   `P.eq` → `eq(_: P, ..)`). Members of unregistered classes are counted with their class.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

/// Method sections of the inventory file, in file order.
pub const METHOD_SECTIONS: [&str; 4] = ["GraphTraversalSource", "GraphTraversal", "__", "terminal"];

/// The section of `Class.member` token entries.
pub const TOKENS_SECTION: &str = "tokens";

/// Upstream version of the vendored inventory file.
pub const INVENTORY_VERSION: &str = "3.8.2";

/// Path of the vendored inventory file (relative to the `graphersal` crate).
pub const INVENTORY_FILE: &str = "tests/tinkerpop/api-3.8.2.txt";

/// A parsed inventory file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inventory {
    /// Method names per method section (keys are [`METHOD_SECTIONS`] names).
    pub methods: BTreeMap<String, Vec<String>>,
    /// Token members per class (`T` → `[id, key, label, value]`).
    pub tokens: BTreeMap<String, Vec<String>>,
}

/// An inventory file error, with its 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryError {
    /// Line of the offending entry (0 for whole-file errors).
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parses the inventory file text.
pub fn parse(text: &str) -> Result<Inventory, InventoryError> {
    let err = |line: usize, message: String| InventoryError { line, message };
    let mut inv = Inventory::default();
    let mut section: Option<String> = None;
    let mut seen_sections = BTreeSet::new();
    let mut previous: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if name != TOKENS_SECTION && !METHOD_SECTIONS.contains(&name) {
                return Err(err(n, format!("unknown section [{name}]")));
            }
            if !seen_sections.insert(name.to_string()) {
                return Err(err(n, format!("duplicate section [{name}]")));
            }
            section = Some(name.to_string());
            previous = None;
            continue;
        }
        let Some(current) = &section else {
            return Err(err(n, format!("entry `{line}` before any [section]")));
        };
        if previous.as_deref().is_some_and(|p| p >= line) {
            return Err(err(
                n,
                format!("entry `{line}` is not sorted/unique in [{current}]"),
            ));
        }
        previous = Some(line.to_string());
        if current == TOKENS_SECTION {
            let Some((class, member)) = line.split_once('.') else {
                return Err(err(
                    n,
                    format!("token entry `{line}` is not `Class.member`"),
                ));
            };
            if !is_identifier(class) || !is_identifier(member) {
                return Err(err(
                    n,
                    format!("token entry `{line}` is not `Class.member`"),
                ));
            }
            inv.tokens
                .entry(class.to_string())
                .or_default()
                .push(member.to_string());
        } else {
            if !is_identifier(line) {
                return Err(err(
                    n,
                    format!("method entry `{line}` is not an identifier"),
                ));
            }
            inv.methods
                .entry(current.clone())
                .or_default()
                .push(line.to_string());
        }
    }
    for name in METHOD_SECTIONS.iter().copied().chain([TOKENS_SECTION]) {
        if !seen_sections.contains(name) {
            return Err(err(0, format!("missing section [{name}]")));
        }
    }
    Ok(inv)
}

/// Reads and parses the vendored inventory file.
pub fn load() -> Result<Inventory, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(INVENTORY_FILE);
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The snake_case form of a camelCase name: `hasLabel` → `has_label`, `inV` → `in_v`, `V` → `v`,
/// `toList` → `to_list`. An upper-case run stays one word (`BIGDECIMAL` → `bigdecimal`).
pub fn to_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            let prev_upper = i > 0 && chars[i - 1].is_ascii_uppercase();
            if prev_lower || (prev_upper && next_lower) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The spellings under which an upstream name counts as registered: as is, and snake_case.
pub fn spellings(name: &str) -> Vec<String> {
    let snake = to_snake(name);
    if snake == name {
        vec![name.to_string()]
    } else {
        vec![name.to_string(), snake]
    }
}

/// What the engine registers, as seen by the inventory check.
#[derive(Debug, Clone, Default)]
pub struct Registered {
    /// Rhai type name of the traversal receiver (`g`'s type).
    pub traversal_type: String,
    /// Globals of the DSL scope → their Rhai type name.
    pub globals: BTreeMap<String, String>,
    /// Rhai type name → function and property-getter names whose first parameter is that type.
    pub members: BTreeMap<String, BTreeSet<String>>,
}

impl Registered {
    fn has_member(&self, type_name: &str, name: &str) -> bool {
        self.members
            .get(type_name)
            .is_some_and(|names| spellings(name).iter().any(|s| names.contains(s)))
    }

    /// Whether a method is registered on the traversal type.
    pub fn has_method(&self, name: &str) -> bool {
        self.has_member(&self.traversal_type, name)
    }
}

/// Parses one `gen_fn_signatures` line into (name, receiver type): `get$id(_: &mut TTokens) -> T`
/// → (`id`, `TTokens`); `eq(_: P, _: Dynamic) -> ArgP` → (`eq`, `P`).
fn signature_receiver(sig: &str) -> Option<(String, String)> {
    let (name, rest) = sig.split_once('(')?;
    let name = name.trim();
    let name = name.strip_prefix("get$").unwrap_or(name);
    if !is_identifier(name) {
        return None;
    }
    let first = rest.split([',', ')']).next()?.trim();
    let ty = first.split_once(':').map_or(first, |(_, t)| t).trim();
    let ty = ty.strip_prefix("&mut ").unwrap_or(ty).trim();
    if ty.is_empty() {
        return None;
    }
    Some((name.to_string(), ty.to_string()))
}

/// Collects the registered names from the DSL engine and scope.
pub fn registered() -> Registered {
    let engine = graphersal::script::engine_with_limits(
        &graphersal::script::ScriptLimits::default(),
        std::sync::Arc::new(graphersal::auth::AllowAll),
    );
    let scope = graphersal::script::graph_scope_with_options(
        Arc::new(graphersal::GraphSource::empty()),
        &[],
        std::sync::Arc::new(graphersal::auth::AllowAll),
    );
    let globals: BTreeMap<String, String> = scope
        .iter()
        .map(|(name, _, value)| {
            (
                name.to_string(),
                engine.map_type_name(value.type_name()).to_string(),
            )
        })
        .collect();
    let mut members: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for sig in engine.gen_fn_signatures(false) {
        if let Some((name, ty)) = signature_receiver(&sig) {
            members.entry(ty).or_default().insert(name);
        }
    }
    Registered {
        traversal_type: globals.get("g").cloned().unwrap_or_default(),
        globals,
        members,
    }
}

/// Result of one method section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionResult {
    /// Section name ([`METHOD_SECTIONS`]).
    pub name: String,
    /// Number of upstream methods in the section.
    pub total: usize,
    /// Upstream methods with no registration (sorted).
    pub missing: Vec<String>,
}

/// A token class that is not in the DSL scope at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingClass {
    /// Class name (`Scope`).
    pub class: String,
    /// Its upstream members (`global`, `local`).
    pub members: Vec<String>,
}

/// A member of a registered token class that does not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingMember {
    /// Upstream class name (`GType`).
    pub class: String,
    /// Rhai type name of the class's global (`GTypeTokens`).
    pub type_name: String,
    /// Member name (`BYTE`).
    pub member: String,
}

/// The whole check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// One entry per method section, in [`METHOD_SECTIONS`] order.
    pub sections: Vec<SectionResult>,
    /// Distinct upstream method names over all method sections.
    pub methods_total: usize,
    /// Distinct method names not registered (sorted).
    pub methods_missing: Vec<String>,
    /// Every distinct upstream method name (registered or not).
    pub method_names: BTreeSet<String>,
    /// Number of upstream token classes.
    pub classes_total: usize,
    /// Token classes not in scope.
    pub missing_classes: Vec<MissingClass>,
    /// Number of members of the classes that are in scope.
    pub members_total: usize,
    /// Members of in-scope classes that do not resolve.
    pub missing_members: Vec<MissingMember>,
}

/// Compares the inventory with the registered names.
pub fn check(inv: &Inventory, reg: &Registered) -> Check {
    let mut sections = Vec::new();
    let mut all_methods = BTreeSet::new();
    for name in METHOD_SECTIONS {
        let methods = inv.methods.get(name).map(Vec::as_slice).unwrap_or(&[]);
        all_methods.extend(methods.iter().cloned());
        sections.push(SectionResult {
            name: name.to_string(),
            total: methods.len(),
            missing: methods
                .iter()
                .filter(|m| !reg.has_method(m))
                .cloned()
                .collect(),
        });
    }
    let methods_missing: Vec<String> = all_methods
        .iter()
        .filter(|m| !reg.has_method(m))
        .cloned()
        .collect();

    let mut missing_classes = Vec::new();
    let mut missing_members = Vec::new();
    let mut members_total = 0;
    for (class, members) in &inv.tokens {
        match reg.globals.get(class) {
            None => missing_classes.push(MissingClass {
                class: class.clone(),
                members: members.clone(),
            }),
            Some(type_name) => {
                members_total += members.len();
                for member in members {
                    if !reg.has_member(type_name, member) {
                        missing_members.push(MissingMember {
                            class: class.clone(),
                            type_name: type_name.clone(),
                            member: member.clone(),
                        });
                    }
                }
            }
        }
    }
    Check {
        sections,
        methods_total: all_methods.len(),
        methods_missing,
        method_names: all_methods,
        classes_total: inv.tokens.len(),
        missing_classes,
        members_total,
        missing_members,
    }
}

/// The one-line stderr summary.
pub fn summary_line(c: &Check) -> String {
    format!(
        "API inventory (TinkerPop {INVENTORY_VERSION}): {} of {} methods, {} of {} token classes, \
         {} of {} members of registered token classes not registered\n",
        c.methods_missing.len(),
        c.methods_total,
        c.missing_classes.len(),
        c.classes_total,
        c.missing_members.len(),
        c.members_total
    )
}

fn count(counts: &BTreeMap<String, usize>, keys: &[String]) -> usize {
    keys.iter().filter_map(|k| counts.get(k)).sum()
}

/// Scenarios the `missing` ranking attributes to a method (either spelling).
pub fn method_count(counts: &BTreeMap<String, usize>, method: &str) -> usize {
    count(counts, &spellings(method))
}

/// Missing methods ranked by attributed scenarios (then name).
pub fn ranked_methods(c: &Check, counts: &BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = c
        .methods_missing
        .iter()
        .map(|m| (m.clone(), method_count(counts, m)))
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

/// The body of the report's inventory section (without its heading).
///
/// `counts` is the `missing` ranking of the run (missing name → scenarios blocked); each missing
/// method, class and member is annotated with the scenarios attributed to it (0 if none). A member
/// is attributed both its `Type.member` property misses and, when the member name is not also a
/// traversal method, its bare function-call misses (`P.typeOf(..)` is reported as `typeOf`).
pub fn markdown(c: &Check, counts: &BTreeMap<String, usize>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Informational, no ratchet. Compares `{INVENTORY_FILE}` (TinkerPop {INVENTORY_VERSION} \
         grammar and `__`) with what the Rhai engine registers: a method counts under its \
         TinkerPop or snake_case spelling on the traversal type, a token class if the DSL scope \
         holds a global of that name, a member if its class is in scope and the member resolves. \
         \"Scenarios\" is how many scenarios the `missing` ranking above attributes to the name.\n"
    );
    let _ = writeln!(out, "`{}`\n", summary_line(c).trim_end());

    let _ = writeln!(
        out,
        "| Section | Upstream | Registered | Not registered |\n|---|---:|---:|---:|"
    );
    for s in &c.sections {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            s.name,
            s.total,
            s.total - s.missing.len(),
            s.missing.len()
        );
    }
    let _ = writeln!(
        out,
        "| token classes | {} | {} | {} |",
        c.classes_total,
        c.classes_total - c.missing_classes.len(),
        c.missing_classes.len()
    );
    let _ = writeln!(
        out,
        "| members of registered token classes | {} | {} | {} |\n",
        c.members_total,
        c.members_total - c.missing_members.len(),
        c.missing_members.len()
    );

    let _ = writeln!(
        out,
        "### Methods not registered — {}\n",
        c.methods_missing.len()
    );
    let _ = writeln!(out, "| Method | Sections | Scenarios |\n|---|---|---:|");
    for (method, n) in ranked_methods(c, counts) {
        let sections: Vec<&str> = c
            .sections
            .iter()
            .filter(|s| s.missing.contains(&method))
            .map(|s| s.name.as_str())
            .collect();
        let _ = writeln!(out, "| `{method}` | {} | {n} |", sections.join(", "));
    }
    out.push('\n');

    let _ = writeln!(
        out,
        "### Token classes not in scope — {}\n",
        c.missing_classes.len()
    );
    let _ = writeln!(out, "| Class | Members | Scenarios |\n|---|---|---:|");
    let mut classes: Vec<(&MissingClass, usize)> = c
        .missing_classes
        .iter()
        .map(|m| (m, counts.get(&m.class).copied().unwrap_or(0)))
        .collect();
    classes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.class.cmp(&b.0.class)));
    for (m, n) in classes {
        let _ = writeln!(out, "| `{}` | {} | {n} |", m.class, m.members.join(", "));
    }
    out.push('\n');

    let _ = writeln!(
        out,
        "### Members of registered token classes not registered — {}\n",
        c.missing_members.len()
    );
    let _ = writeln!(out, "| Member | Scenarios |\n|---|---:|");
    let mut members: Vec<(&MissingMember, usize)> = c
        .missing_members
        .iter()
        .map(|m| {
            let mut n = counts
                .get(&format!("{}.{}", m.type_name, m.member))
                .copied()
                .unwrap_or(0);
            if !c.method_names.contains(&m.member) {
                n += counts.get(&m.member).copied().unwrap_or(0);
            }
            (m, n)
        })
        .collect();
    members.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.0.class.cmp(&b.0.class))
            .then(a.0.member.cmp(&b.0.member))
    });
    for (m, n) in members {
        let _ = writeln!(out, "| `{}.{}` | {n} |", m.class, m.member);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_spellings() {
        assert_eq!(to_snake("V"), "v");
        assert_eq!(to_snake("E"), "e");
        assert_eq!(to_snake("inV"), "in_v");
        assert_eq!(to_snake("hasId"), "has_id");
        assert_eq!(to_snake("hasLabel"), "has_label");
        assert_eq!(to_snake("toList"), "to_list");
        assert_eq!(to_snake("withSideEffect"), "with_side_effect");
        assert_eq!(to_snake("lTrim"), "l_trim");
        assert_eq!(to_snake("BIGDECIMAL"), "bigdecimal");
        assert_eq!(to_snake("bigDecimal"), "big_decimal");
        assert_eq!(to_snake("out"), "out");
        assert_eq!(spellings("out"), vec!["out"]);
        assert_eq!(spellings("inV"), vec!["inV", "in_v"]);
    }

    fn reg(
        traversal: &[&str],
        globals: &[(&str, &str)],
        members: &[(&str, &[&str])],
    ) -> Registered {
        let mut m: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        m.insert(
            "Traversal".to_string(),
            traversal.iter().map(|s| s.to_string()).collect(),
        );
        for (ty, names) in members {
            m.insert(
                ty.to_string(),
                names.iter().map(|s| s.to_string()).collect(),
            );
        }
        Registered {
            traversal_type: "Traversal".to_string(),
            globals: globals
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            members: m,
        }
    }

    #[test]
    fn either_spelling_counts() {
        let r = reg(&["v", "E", "in_v", "hasId", "to_list"], &[], &[]);
        for name in ["V", "E", "inV", "hasId", "toList"] {
            assert!(r.has_method(name), "{name}");
        }
        assert!(!r.has_method("outV"));
        // A function of the same name on another receiver does not count.
        let r = reg(&[], &[("P", "P")], &[("P", &["eq"])]);
        assert!(!r.has_method("eq"));
    }

    const SAMPLE: &str = "\
# comment

[GraphTraversalSource]
V
withStrategies

[GraphTraversal]
hasLabel
out

[__]
out

[terminal]
toList

[tokens]
P.eq
P.typeOf
Scope.global
Scope.local
T.id
";

    #[test]
    fn check_reports_missing_methods_classes_and_members() {
        let inv = parse(SAMPLE).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(inv.tokens["Scope"], vec!["global", "local"]);
        let r = reg(
            &["v", "has_label", "out", "toList"],
            &[("g", "Traversal"), ("P", "P"), ("T", "TTokens")],
            &[("P", &["eq"]), ("TTokens", &["id"])],
        );
        let c = check(&inv, &r);
        assert_eq!(c.methods_total, 5);
        assert_eq!(c.methods_missing, vec!["withStrategies"]);
        assert_eq!(c.sections[0].missing, vec!["withStrategies"]);
        assert!(c.sections[1].missing.is_empty());
        assert_eq!(c.classes_total, 3);
        assert_eq!(c.missing_classes.len(), 1);
        assert_eq!(c.missing_classes[0].class, "Scope");
        assert_eq!(c.members_total, 3);
        assert_eq!(c.missing_members.len(), 1);
        assert_eq!(c.missing_members[0].member, "typeOf");
        assert_eq!(
            summary_line(&c),
            "API inventory (TinkerPop 3.8.2): 1 of 5 methods, 1 of 3 token classes, 1 of 3 members of registered token classes not registered\n"
        );

        let counts: BTreeMap<String, usize> = [("Scope", 7), ("typeOf", 2), ("withStrategies", 3)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let md = markdown(&c, &counts);
        assert!(
            md.contains("| `withStrategies` | GraphTraversalSource | 3 |"),
            "{md}"
        );
        assert!(md.contains("| `Scope` | global, local | 7 |"), "{md}");
        assert!(md.contains("| `P.typeOf` | 2 |"), "{md}");
    }

    #[test]
    fn parse_errors() {
        assert!(parse("V\n").is_err(), "entry before a section");
        assert!(parse("[foo]\n").is_err(), "unknown section");
        let unsorted = SAMPLE.replace("hasLabel\nout", "out\nhasLabel");
        let e = parse(&unsorted).err().map(|e| e.line);
        assert_eq!(e, Some(9));
        assert!(parse(&SAMPLE.replace("T.id", "Tid")).is_err());
        assert!(parse(&SAMPLE.replace("[terminal]\ntoList\n", "")).is_err());
    }

    #[test]
    fn signature_receivers() {
        assert_eq!(
            signature_receiver("get$id(_: &mut TTokens) -> T"),
            Some(("id".to_string(), "TTokens".to_string()))
        );
        assert_eq!(
            signature_receiver("eq(_: P, _: types::dynamic::Dynamic) -> ArgP"),
            Some(("eq".to_string(), "P".to_string()))
        );
        assert_eq!(
            signature_receiver("out(s: &mut GraphTraversalSource) -> GraphTraversalSource"),
            Some(("out".to_string(), "GraphTraversalSource".to_string()))
        );
        assert_eq!(signature_receiver("+(a: i64, b: i64) -> i64"), None);
    }

    #[test]
    fn vendored_inventory_against_engine() {
        let inv = load().unwrap_or_else(|e| panic!("{e}"));
        assert!(
            inv.methods["GraphTraversal"]
                .iter()
                .any(|m| m == "hasLabel")
        );
        assert!(inv.tokens.contains_key("Scope"));
        let r = registered();
        assert!(!r.traversal_type.is_empty(), "no `g` in scope");
        for name in ["V", "E", "inV", "hasId", "hasLabel", "toList", "out"] {
            assert!(r.has_method(name), "{name} should be registered");
        }
        assert_eq!(
            r.globals.get("T").map(|t| r.has_member(t, "id")),
            Some(true)
        );
        let c = check(&inv, &r);
        assert!(c.methods_missing.len() < c.methods_total);
    }
}
