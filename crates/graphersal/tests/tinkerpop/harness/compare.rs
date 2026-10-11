//! Expectation resolution and value comparison for the TinkerPop feature suite.
//!
//! Two phases:
//!
//! 1. [`resolve`] turns a parsed [`Expected`] into a [`Resolved`] value against the scenario's
//!    graph *after* the traversal ran (a [`Lookup`]): `v[name]` becomes the vertex id of the one
//!    vertex whose `name` is `name`, `e[a-label->b]` the id of the one edge with that label between
//!    those named vertices. Forms without a faithful Graphersal counterpart fail resolution with a
//!    precise reason (the scenario is then classified `translate`, never passed):
//!    `vp[..]` (Graphersal's `properties()` yields bare values, not vertex properties),
//!    a `BigDecimal` outside `f64`, a `BigInteger` outside `i64`, `dt[..]`, `D[..]`/`M[..]` tokens as values or map
//!    keys, `t[..]` tokens as values. `t[id]` / `t[label]` *map keys* resolve to the `"id"` /
//!    `"label"` keys that `elementMap()` returns.
//! 2. [`matches`] compares a [`Resolved`] value with an actual [`ElementProperty`], on values,
//!    never on rendered text:
//!    - elements by identity: kind (vertex/edge) and the `id` entry of the element;
//!    - numbers by class only: an integer expectation (any width) matches an `i64` number, a
//!      floating point expectation an `f64` number; integer vs. float is a mismatch; two floats
//!      are equal when they differ by at most 1 ulp ([`floats_within_one_ulp`]: `NaN` equals
//!      `NaN`, infinities exact, `+0`/`-0` equal), a normalization of libm vs. Java last-digit
//!      differences that applies to `f64` vs. `f64` result values only;
//!    - `l[..]` and `p[..]` element-wise against an `Array` (`path()` yields an `Array`), `s[..]`
//!      as a multiset against an `Array`, `m[..]` by key set and values against an `Object` (key
//!      order ignored), `null` ↔ `Null`.
//!
//! # Id tolerance
//!
//! Graphersal ids are strings, upstream ids are integers. An integer expectation `d[N].*` also
//! matches a `String` that is exactly `N`'s decimal rendering, but **only in an id position**:
//! - the value under a `t[id]` map key, always;
//! - anywhere in the scenario's result when [`IdMode::Scenario`] is on, which the classifier sets
//!   iff the translated traversal calls `id()` or names `T.id` (see [`script_reads_ids`]).
//!
//! No other value kind is ever compared tolerantly.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use graphersal::ElementProperty;
use graphersal::script::ScriptGraphTraversalSource;
use graphersal::script::rhai::Dynamic;
use graphersal::storage::AnyGraph;

use super::notation::{Dir, Expected, TToken};

/// An expectation resolved against a graph, ready to compare.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    /// A vertex, by id.
    Vertex(String),
    /// An edge, by id.
    Edge(String),
    /// An element id (`v[x].id`, `.sid`, `e[..].id`, `.sid`): our ids are strings.
    Id(String),
    /// Any integral number that fits `i64` (width ignored).
    Int(i64),
    /// A floating point number (width ignored).
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// A string.
    Str(String),
    /// `null`.
    Null,
    /// A UUID.
    Uuid(u128),
    /// `l[..]`: ordered.
    List(Vec<Resolved>),
    /// `s[..]`: a multiset.
    Set(Vec<Resolved>),
    /// `p[..]`: element-wise.
    Path(Vec<Resolved>),
    /// `m[..]`: entries in source order.
    Map(Vec<(Key, Resolved)>),
}

/// A resolved map key.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    /// A string key (also `t[label]` → `"label"`).
    Str(String),
    /// `t[id]` → the `"id"` key; its value is compared in id mode.
    IdToken,
    /// Any other key (number, element, ...), compared as a value with the actual string key.
    Value(Resolved),
}

/// Looks up element ids by the upstream naming conventions.
pub trait Lookup {
    /// The id of the one vertex whose `name` property is `name`.
    fn vertex_id(&self, name: &str) -> Result<String, String>;
    /// The id of the one edge `out -label-> inv` (endpoints by `name`).
    fn edge_id(&self, out: &str, label: &str, inv: &str) -> Result<String, String>;
}

/// A snapshot of a graph's element names and ids.
#[derive(Debug, Clone, Default)]
pub struct GraphIndex {
    /// `name` → ids of the vertices carrying it.
    vertices: HashMap<String, Vec<String>>,
    /// `(out name, label, in name)` → edge ids.
    edges: HashMap<(String, String, String), Vec<String>>,
}

impl GraphIndex {
    /// Builds an index from `(name, id)` vertex pairs and `(out name, label, in name, id)` edges.
    pub fn from_parts(vertices: &[(&str, &str)], edges: &[(&str, &str, &str, &str)]) -> Self {
        let mut index = GraphIndex::default();
        for (name, id) in vertices {
            index
                .vertices
                .entry((*name).to_string())
                .or_default()
                .push((*id).to_string());
        }
        for (out, label, inv, id) in edges {
            index
                .edges
                .entry(((*out).to_string(), (*label).to_string(), (*inv).to_string()))
                .or_default()
                .push((*id).to_string());
        }
        index
    }

    /// Reads every vertex and edge of `graph` through the DSL (`g.V()`, `g.E()`).
    pub fn build(graph: &Arc<dyn AnyGraph>) -> Result<Self, String> {
        let engine = graphersal::script::engine_with_limits(
            &graphersal::script::ScriptLimits::default(),
            std::sync::Arc::new(graphersal::auth::AllowAll),
        );
        let elements = |script: &str| -> Result<Vec<ElementProperty>, String> {
            let mut scope = graphersal::script::graph_scope_with_options(
                graph.clone(),
                &[],
                std::sync::Arc::new(graphersal::auth::AllowAll),
            );
            let value = engine
                .eval_with_scope::<Dynamic>(&mut scope, script)
                .map_err(|e| format!("index {script}: {e}"))?;
            if !value.is::<ScriptGraphTraversalSource>() {
                return Err(format!("index {script}: not a traversal"));
            }
            value
                .cast::<ScriptGraphTraversalSource>()
                .to_typed_list()
                .map_err(|e| format!("index {script}: {e}"))
        };
        let text = |element: &graphersal::prelude::PropertyMap, key: &str| -> Option<String> {
            element
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };

        let mut index = GraphIndex::default();
        let mut names: HashMap<String, String> = HashMap::new();
        for vertex in elements("g.V()")? {
            let ElementProperty::Vertex(element) = vertex else {
                continue;
            };
            if let (Some(id), Some(name)) = (text(&element, "id"), text(&element, "name")) {
                names.insert(id.clone(), name.clone());
                index.vertices.entry(name).or_default().push(id);
            }
        }
        for edge in elements("g.E()")? {
            let ElementProperty::Edge(element) = edge else {
                continue;
            };
            let (Some(id), Some(label), Some(from), Some(to)) = (
                text(&element, "id"),
                text(&element, "label"),
                text(&element, "_from"),
                text(&element, "_to"),
            ) else {
                continue;
            };
            if let (Some(out), Some(inv)) = (names.get(&from), names.get(&to)) {
                index
                    .edges
                    .entry((out.clone(), label, inv.clone()))
                    .or_default()
                    .push(id);
            }
        }
        Ok(index)
    }
}

fn single(ids: Option<&Vec<String>>, what: String) -> Result<String, String> {
    match ids.map(Vec::as_slice) {
        Some([id]) => Ok(id.clone()),
        None | Some([]) => Err(format!("cannot resolve {what}: not in the graph")),
        Some(many) => Err(format!(
            "cannot resolve {what}: ambiguous ({} matches)",
            many.len()
        )),
    }
}

impl Lookup for GraphIndex {
    fn vertex_id(&self, name: &str) -> Result<String, String> {
        single(self.vertices.get(name), format!("v[{name}]"))
    }

    fn edge_id(&self, out: &str, label: &str, inv: &str) -> Result<String, String> {
        let key = (out.to_string(), label.to_string(), inv.to_string());
        single(self.edges.get(&key), format!("e[{out}-{label}->{inv}]"))
    }
}

/// A [`Lookup`] over a graph that is indexed only when an expectation first needs it.
pub struct LazyIndex<'a> {
    graph: Option<&'a Arc<dyn AnyGraph>>,
    index: OnceCell<Result<GraphIndex, String>>,
}

impl<'a> LazyIndex<'a> {
    /// A lookup over `graph` (`None`: every lookup fails).
    pub fn new(graph: Option<&'a Arc<dyn AnyGraph>>) -> Self {
        Self {
            graph,
            index: OnceCell::new(),
        }
    }

    fn index(&self) -> Result<&GraphIndex, String> {
        self.index
            .get_or_init(|| match self.graph {
                Some(graph) => GraphIndex::build(graph),
                None => Err("no graph to resolve against".to_string()),
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

impl Lookup for LazyIndex<'_> {
    fn vertex_id(&self, name: &str) -> Result<String, String> {
        self.index()?.vertex_id(name)
    }

    fn edge_id(&self, out: &str, label: &str, inv: &str) -> Result<String, String> {
        self.index()?.edge_id(out, label, inv)
    }
}

/// Resolves `expected` against `lookup`; `Err` carries the reason the runner cannot evaluate it.
pub fn resolve(expected: &Expected, lookup: &dyn Lookup) -> Result<Resolved, String> {
    let all = |items: &[Expected]| -> Result<Vec<Resolved>, String> {
        items.iter().map(|item| resolve(item, lookup)).collect()
    };
    Ok(match expected {
        Expected::Vertex(name) => Resolved::Vertex(lookup.vertex_id(name)?),
        Expected::VertexId(name) | Expected::VertexSid(name) => {
            Resolved::Id(lookup.vertex_id(name)?)
        }
        Expected::Edge { out, label, inv } => Resolved::Edge(lookup.edge_id(out, label, inv)?),
        Expected::EdgeId { out, label, inv } | Expected::EdgeSid { out, label, inv } => {
            Resolved::Id(lookup.edge_id(out, label, inv)?)
        }
        Expected::VertexProperty { vertex, key, .. } => {
            return Err(format!(
                "vp[{vertex}-{key}->..]: no vertex-property values in Graphersal (properties() yields bare values)"
            ));
        }
        Expected::Int { value, width } => match i64::try_from(*value) {
            Ok(v) => Resolved::Int(v),
            Err(_) => {
                return Err(format!(
                    "integer {value} ({width:?}) outside i64: no BigInteger in Graphersal"
                ));
            }
        },
        Expected::Float { value, .. } => Resolved::Float(*value),
        // Test-surface deviation: a BigDecimal collapses to float64 when it fits one.
        Expected::BigDecimal(text) => match text.trim().parse::<f64>() {
            Ok(v) if v.is_finite() => Resolved::Float(v),
            _ => return Err(format!("BigDecimal d[{text}].m: outside float64")),
        },
        Expected::Bool(b) => Resolved::Bool(*b),
        Expected::List(items) => Resolved::List(all(items)?),
        Expected::Set(items) => Resolved::Set(all(items)?),
        Expected::Path(items) => Resolved::Path(all(items)?),
        Expected::Map(entries) => {
            let mut resolved = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let key = match key {
                    Expected::Str(s) => Key::Str(s.clone()),
                    Expected::TokenT(TToken::Id) => Key::IdToken,
                    Expected::TokenT(TToken::Label) => Key::Str("label".to_string()),
                    Expected::Direction(d) => {
                        return Err(format!(
                            "map key D[{}]: Graphersal element maps carry no in/out vertex entries",
                            dir_name(*d)
                        ));
                    }
                    Expected::Merge(_) => {
                        return Err("map key M[..]: no Merge tokens in results".to_string());
                    }
                    other => Key::Value(resolve(other, lookup)?),
                };
                resolved.push((key, resolve(value, lookup)?));
            }
            Resolved::Map(resolved)
        }
        Expected::TokenT(t) => {
            return Err(format!(
                "T token t[{t:?}] as a result value has no counterpart"
            ));
        }
        Expected::Direction(d) => {
            return Err(format!(
                "Direction token D[{}] as a result value has no counterpart",
                dir_name(*d)
            ));
        }
        Expected::Merge(m) => return Err(format!("Merge token {m:?} as a result value")),
        Expected::DateTime(text) => {
            return Err(format!(
                "datetime dt[{text}]: no datetime type in Graphersal"
            ));
        }
        Expected::Uuid(text) => Resolved::Uuid(
            parse_uuid(text).ok_or_else(|| format!("uuid[{text}] is not a canonical UUID"))?,
        ),
        Expected::Null => Resolved::Null,
        Expected::Str(s) => Resolved::Str(s.clone()),
    })
}

fn dir_name(d: Dir) -> &'static str {
    match d {
        Dir::Out => "OUT",
        Dir::In => "IN",
    }
}

/// Parses a canonical (hyphenated or plain, 32 hex digits) UUID.
fn parse_uuid(text: &str) -> Option<u128> {
    let hex: String = text.trim().chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

/// Whether ids compare tolerantly everywhere in a scenario's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdMode {
    /// Only under `t[id]` keys.
    KeysOnly,
    /// Everywhere (the traversal reads ids).
    Scenario,
}

/// Whether `script` calls `id()` or names `T.id` (outside string literals).
pub fn script_reads_ids(script: &str) -> bool {
    identifiers(script).iter().any(|(ident, rest)| {
        (*ident == "id" && rest.trim_start().starts_with('('))
            || (*ident == "T"
                && rest.starts_with(".id")
                && !rest[3..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_'))
    })
}

/// Whether `script` contains an explicit ordering step: `order(` or `by(` (outside strings).
pub fn script_orders(script: &str) -> bool {
    identifiers(script)
        .iter()
        .any(|(ident, rest)| matches!(*ident, "order" | "by") && rest.trim_start().starts_with('('))
}

/// Every identifier of `script` outside string literals, with the text that follows it.
fn identifiers(script: &str) -> Vec<(&str, &str)> {
    let bytes = script.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'\'' {
            i += 1;
            while i < bytes.len() && bytes[i] != c {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push((&script[start..i], &script[i..]));
        } else {
            i += 1;
        }
    }
    out
}

/// Whether the actual value `actual` equals the expectation `expected`.
pub fn matches(expected: &Resolved, actual: &ElementProperty, ids: IdMode) -> bool {
    matches_at(expected, actual, ids, ids == IdMode::Scenario)
}

fn element_id(element: &graphersal::prelude::PropertyMap) -> Option<&str> {
    element.get("id").and_then(|v| v.as_str())
}

/// `true` when two `f64` values are equal or adjacent floats (at most 1 ulp apart).
///
/// `NaN` equals `NaN` (and nothing else), infinities compare exactly, `+0` equals `-0`, and values
/// of different sign are never equal (apart from the zeros). Normalization of libm vs. Java
/// last-digit differences (e.g. `sin(4.0)`), not an arithmetic deviation of Graphersal.
pub fn floats_within_one_ulp(a: f64, b: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return a.is_nan() && b.is_nan();
    }
    if a == b {
        return true;
    }
    if a.is_infinite() || b.is_infinite() || a.is_sign_negative() != b.is_sign_negative() {
        return false;
    }
    a.to_bits().abs_diff(b.to_bits()) <= 1
}

fn matches_at(expected: &Resolved, actual: &ElementProperty, ids: IdMode, id_pos: bool) -> bool {
    match (expected, actual) {
        (Resolved::Vertex(id), ElementProperty::Vertex(e))
        | (Resolved::Edge(id), ElementProperty::Edge(e)) => element_id(e) == Some(id.as_str()),
        (Resolved::Id(id), ElementProperty::String(s)) => s.as_str() == id,
        (Resolved::Int(n), ElementProperty::Number(num)) => num.as_i64() == Some(*n),
        (Resolved::Int(n), ElementProperty::String(s)) => id_pos && s.as_str() == n.to_string(),
        (Resolved::Float(f), ElementProperty::Number(num)) => {
            num.is_f64() && num.as_f64().is_some_and(|a| floats_within_one_ulp(*f, a))
        }
        (Resolved::Bool(b), ElementProperty::Boolean(a)) => a == b,
        (Resolved::Str(s), ElementProperty::String(a)) => a.as_str() == s,
        (Resolved::Null, ElementProperty::Null) => true,
        (Resolved::Uuid(u), ElementProperty::Uuid(a)) => a == u,
        (Resolved::List(items) | Resolved::Path(items), ElementProperty::Array(actual)) => {
            items.len() == actual.len()
                && items
                    .iter()
                    .zip(actual)
                    .all(|(e, a)| matches_at(e, a, ids, id_pos))
        }
        (Resolved::Set(items), ElementProperty::Array(actual)) => {
            multiset_diff_at(items, actual, ids, id_pos).is_none()
        }
        (Resolved::Map(entries), ElementProperty::Map(actual)) => {
            // Typed keys: an expected key matches an actual key by value, never by its text.
            entries.len() == actual.len()
                && entries.iter().all(|(key, value)| {
                    actual.iter().any(|(k, v)| match key {
                        Key::Str(s) => {
                            k.as_str() == Some(s.as_str()) && matches_at(value, v, ids, id_pos)
                        }
                        Key::IdToken => false,
                        Key::Value(expected_key) => {
                            matches_at(expected_key, k, ids, false)
                                && matches_at(value, v, ids, id_pos)
                        }
                    })
                })
        }
        (Resolved::Map(entries), ElementProperty::Object(actual)) => {
            entries.len() == actual.len()
                && entries.iter().all(|(key, value)| {
                    actual.iter().any(|(k, v)| match key {
                        Key::Str(s) => k.as_str() == s && matches_at(value, v, ids, id_pos),
                        Key::IdToken => k.as_str() == "id" && matches_at(value, v, ids, true),
                        Key::Value(expected_key) => {
                            matches_at(
                                expected_key,
                                &ElementProperty::String(k.clone()),
                                ids,
                                false,
                            ) && matches_at(value, v, ids, id_pos)
                        }
                    })
                })
        }
        _ => false,
    }
}

/// The result of a multiset comparison that failed: what could not be paired.
#[derive(Debug, Clone, PartialEq)]
pub struct MultisetDiff {
    /// Expected values without an actual partner.
    pub missing: Vec<Resolved>,
    /// Actual values without an expected partner.
    pub unexpected: Vec<ElementProperty>,
}

/// `None` when `expected` and `actual` are equal as multisets, else what does not pair up.
pub fn multiset_diff(
    expected: &[Resolved],
    actual: &[ElementProperty],
    ids: IdMode,
) -> Option<MultisetDiff> {
    multiset_diff_at(expected, actual, ids, ids == IdMode::Scenario)
}

/// Maximum bipartite matching (greedy first, then augmenting paths, Kuhn's algorithm), because
/// id tolerance makes equality non-transitive (`d[1].i` matches both `1` and `"1"`).
fn multiset_diff_at(
    expected: &[Resolved],
    actual: &[ElementProperty],
    ids: IdMode,
    id_pos: bool,
) -> Option<MultisetDiff> {
    let eq = |e: usize, a: usize| matches_at(&expected[e], &actual[a], ids, id_pos);
    // partner_of_actual[a] = expected index paired with actual a.
    let mut partner_of_actual: Vec<Option<usize>> = vec![None; actual.len()];
    let mut unmatched: Vec<usize> = Vec::new();
    for e in 0..expected.len() {
        match (0..actual.len()).find(|&a| partner_of_actual[a].is_none() && eq(e, a)) {
            Some(a) => partner_of_actual[a] = Some(e),
            None => unmatched.push(e),
        }
    }
    let mut still_unmatched = Vec::new();
    for e in unmatched {
        let mut seen = vec![false; actual.len()];
        if !augment(e, &eq, &mut partner_of_actual, &mut seen) {
            still_unmatched.push(e);
        }
    }
    if still_unmatched.is_empty() && expected.len() == actual.len() {
        return None;
    }
    Some(MultisetDiff {
        missing: still_unmatched
            .into_iter()
            .map(|e| expected[e].clone())
            .collect(),
        unexpected: partner_of_actual
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_none())
            .map(|(a, _)| actual[a].clone())
            .collect(),
    })
}

fn augment(
    e: usize,
    eq: &dyn Fn(usize, usize) -> bool,
    partner_of_actual: &mut [Option<usize>],
    seen: &mut [bool],
) -> bool {
    for a in 0..partner_of_actual.len() {
        if seen[a] || !eq(e, a) {
            continue;
        }
        seen[a] = true;
        let free = match partner_of_actual[a] {
            None => true,
            Some(other) => augment(other, eq, partner_of_actual, seen),
        };
        if free {
            partner_of_actual[a] = Some(e);
            return true;
        }
    }
    false
}

/// Index of the first position where the sequences differ (or the shorter length).
pub fn first_sequence_diff(
    expected: &[Resolved],
    actual: &[ElementProperty],
    ids: IdMode,
) -> Option<usize> {
    let n = expected.len().min(actual.len());
    (0..n)
        .find(|&i| !matches(&expected[i], &actual[i], ids))
        .or((expected.len() != actual.len()).then_some(n))
}

/// Compact diagnostic rendering of an actual value (elements as `v[id]` / `e[id]`).
pub fn short(value: &ElementProperty) -> String {
    let mut out = String::new();
    write_short(&mut out, value);
    out
}

fn write_short(out: &mut String, value: &ElementProperty) {
    match value {
        ElementProperty::Null => out.push_str("null"),
        ElementProperty::String(s) => {
            let _ = write!(out, "{:?}", s.as_str());
        }
        ElementProperty::Number(n) => {
            let _ = match (n.as_i64(), n.as_f64()) {
                (Some(i), _) => write!(out, "{i}"),
                (None, Some(f)) => write!(out, "{f:?}"),
                (None, None) => write!(out, "{n:?}"),
            };
        }
        ElementProperty::Boolean(b) => {
            let _ = write!(out, "{b}");
        }
        ElementProperty::Uuid(u) => {
            let _ = write!(out, "uuid[{u:032x}]");
        }
        ElementProperty::Vertex(e) => {
            let _ = write!(out, "v[{}]", element_id(e).unwrap_or("?"));
        }
        ElementProperty::Edge(e) => {
            let _ = write!(out, "e[{}]", element_id(e).unwrap_or("?"));
        }
        ElementProperty::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_short(out, item);
            }
            out.push(']');
        }
        ElementProperty::Object(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let _ = write!(out, "{:?}: ", k.as_str());
                write_short(out, v);
            }
            out.push('}');
        }
        other => {
            let _ = write!(out, "{other:?}");
        }
    }
}

/// Compact diagnostic rendering of a resolved expectation.
pub fn short_expected(value: &Resolved) -> String {
    let list = |open: &str, items: &[Resolved], close: &str| {
        let inner: Vec<String> = items.iter().map(short_expected).collect();
        format!("{open}{}{close}", inner.join(", "))
    };
    match value {
        Resolved::Vertex(id) => format!("v[{id}]"),
        Resolved::Edge(id) => format!("e[{id}]"),
        Resolved::Id(id) => format!("id {id:?}"),
        Resolved::Int(i) => format!("{i}"),
        Resolved::Float(f) => format!("{f:?}"),
        Resolved::Bool(b) => format!("{b}"),
        Resolved::Str(s) => format!("{s:?}"),
        Resolved::Null => "null".to_string(),
        Resolved::Uuid(u) => format!("uuid[{u:032x}]"),
        Resolved::List(items) => list("[", items, "]"),
        Resolved::Set(items) => list("s[", items, "]"),
        Resolved::Path(items) => list("p[", items, "]"),
        Resolved::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        Key::Str(s) => format!("{s:?}"),
                        Key::IdToken => "t[id]".to_string(),
                        Key::Value(v) => short_expected(v),
                    };
                    format!("{key}: {}", short_expected(v))
                })
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tree assertions (`the result should be a tree with a structure of`)
// ---------------------------------------------------------------------------------------------

/// One node of an expected tree: a resolved key and its children.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// The node's key (a vertex, an edge, a number, a string, ...).
    pub label: Resolved,
    /// The children; a leaf has none.
    pub children: Vec<TreeNode>,
}

/// Parses the `|--label` outline of a tree assertion into a forest, resolving every label.
/// Depth is the number of leading spaces divided by three; anything malformed is an `Err`
/// (the scenario is then classified `translate`). An empty outline is the empty tree.
pub fn parse_tree(text: &str, lookup: &dyn Lookup) -> Result<Vec<TreeNode>, String> {
    // (depth, node) in document order, then folded into a forest.
    let mut flat: Vec<(usize, TreeNode)> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let body = line.trim_start_matches(' ');
        let spaces = line.len() - body.len();
        let Some(label) = body.strip_prefix("|--") else {
            return Err(format!("tree line {line:?} does not start with `|--`"));
        };
        if spaces % 3 != 0 {
            return Err(format!(
                "tree line {line:?}: indentation is not a multiple of 3"
            ));
        }
        let depth = spaces / 3;
        let max = flat.last().map_or(0, |(d, _)| d + 1);
        if depth > max {
            return Err(format!(
                "tree line {line:?}: indented deeper than its parent allows"
            ));
        }
        let expected = super::notation::parse_cell(label.trim_end())
            .map_err(|e| format!("tree label {label:?}: {e}"))?;
        flat.push((
            depth,
            TreeNode {
                label: resolve(&expected, lookup)?,
                children: Vec::new(),
            },
        ));
    }
    fn fold(
        flat: &mut std::iter::Peekable<std::vec::IntoIter<(usize, TreeNode)>>,
        depth: usize,
    ) -> Vec<TreeNode> {
        let mut out = Vec::new();
        while let Some((d, _)) = flat.peek() {
            if *d != depth {
                break;
            }
            let Some((_, mut node)) = flat.next() else {
                break;
            };
            node.children = fold(flat, depth + 1);
            out.push(node);
        }
        out
    }
    let forest = fold(&mut flat.into_iter().peekable(), 0);
    // An empty outline is the empty tree (a traversal whose paths are all too short).
    Ok(forest)
}

/// The `(key, child)` entries of an actual tree level: a string-keyed `Object` or a typed `Map`.
fn tree_entries(actual: &ElementProperty) -> Option<Vec<(ElementProperty, &ElementProperty)>> {
    match actual {
        ElementProperty::Object(o) => Some(
            o.iter()
                .map(|(k, v)| (ElementProperty::String(k.clone()), v))
                .collect(),
        ),
        ElementProperty::Map(m) => Some(m.iter().map(|(k, v)| (k.clone(), v)).collect()),
        _ => None,
    }
}

/// Whether `actual` (a map of key to nested map, a leaf being an empty map) has exactly the shape
/// of `expected`. Siblings are matched as a set: TinkerPop trees are unordered.
pub fn compare_tree(expected: &[TreeNode], actual: &ElementProperty, ids: IdMode) -> bool {
    let Some(entries) = tree_entries(actual) else {
        return false;
    };
    if entries.len() != expected.len() {
        return false;
    }
    let eq = |e: usize, a: usize| {
        matches_at(&expected[e].label, &entries[a].0, ids, false)
            && compare_tree(&expected[e].children, entries[a].1, ids)
    };
    let mut partner: Vec<Option<usize>> = vec![None; entries.len()];
    (0..expected.len()).all(|e| {
        let mut seen = vec![false; entries.len()];
        augment(e, &eq, &mut partner, &mut seen)
    })
}

/// Compact outline of an actual tree for failure texts, one line `a(b, c(d))` form.
pub fn short_tree(actual: &ElementProperty) -> String {
    match tree_entries(actual) {
        None => short(actual),
        Some(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    let child = short_tree(v);
                    if child == "()" || child.is_empty() {
                        short(k)
                    } else {
                        format!("{}{child}", short(k))
                    }
                })
                .collect();
            format!("({})", inner.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::notation;
    use graphersal::prelude::{Number, PropertyMap};

    fn modern() -> GraphIndex {
        GraphIndex::from_parts(
            &[
                ("marko", "1"),
                ("vadas", "2"),
                ("lop", "3"),
                ("dup", "8"),
                ("dup", "9"),
            ],
            &[("marko", "knows", "vadas", "7")],
        )
    }

    fn r(text: &str) -> Result<Resolved, String> {
        resolve(&notation::parse_cell(text).expect("notation"), &modern())
    }

    fn rr(text: &str) -> Resolved {
        r(text).expect("resolves")
    }

    fn s(v: &str) -> ElementProperty {
        ElementProperty::String(v.into())
    }

    fn i(v: i64) -> ElementProperty {
        ElementProperty::Number(Number::from_i64(v))
    }

    fn f(v: f64) -> ElementProperty {
        ElementProperty::Number(Number::from_f64(v))
    }

    fn element(pairs: &[(&str, ElementProperty)]) -> Box<PropertyMap> {
        Box::new(pairs.iter().map(|(k, v)| ((*k), v.clone())).collect())
    }

    fn vertex(id: &str) -> ElementProperty {
        ElementProperty::Vertex(element(&[("id", s(id)), ("label", s("person"))]))
    }

    fn edge(id: &str) -> ElementProperty {
        ElementProperty::Edge(element(&[("id", s(id)), ("label", s("knows"))]))
    }

    fn arr(items: Vec<ElementProperty>) -> ElementProperty {
        ElementProperty::Array(items)
    }

    fn obj(pairs: &[(&str, ElementProperty)]) -> ElementProperty {
        ElementProperty::Object(element(pairs))
    }

    const K: IdMode = IdMode::KeysOnly;
    const S: IdMode = IdMode::Scenario;

    #[test]
    fn elements_resolve_by_name_and_compare_by_kind_and_id() {
        assert_eq!(rr("v[marko]"), Resolved::Vertex("1".into()));
        assert_eq!(rr("e[marko-knows->vadas]"), Resolved::Edge("7".into()));
        assert!(matches(&rr("v[marko]"), &vertex("1"), K));
        assert!(!matches(&rr("v[marko]"), &vertex("2"), K));
        assert!(!matches(&rr("v[marko]"), &edge("1"), K), "kind matters");
        assert!(matches(&rr("e[marko-knows->vadas]"), &edge("7"), K));
        assert!(
            !matches(&rr("v[marko]"), &s("1"), S),
            "a vertex is not its id"
        );
    }

    #[test]
    fn ids_resolve_to_strings() {
        assert!(matches(&rr("v[marko].id"), &s("1"), K));
        assert!(matches(&rr("v[marko].sid"), &s("1"), K));
        assert!(matches(&rr("e[marko-knows->vadas].id"), &s("7"), K));
        assert!(!matches(&rr("v[marko].id"), &i(1), K));
    }

    #[test]
    fn unresolvable_names_are_reported() {
        assert!(
            r("v[nobody]")
                .unwrap_err()
                .contains("cannot resolve v[nobody]")
        );
        assert!(r("v[dup]").unwrap_err().contains("ambiguous"));
        assert!(
            r("e[marko-created->vadas]")
                .unwrap_err()
                .contains("cannot resolve e[")
        );
        assert!(
            r("l[v[marko],v[nobody]]").is_err(),
            "nested failures propagate"
        );
    }

    #[test]
    fn forms_without_counterpart_do_not_resolve() {
        assert!(r("vp[marko-name->marko]").unwrap_err().contains("vp["));
        // Test-surface deviation: widths collapse to int64/float64.
        assert!(matches!(r("d[1.5].m"), Ok(Resolved::Float(v)) if v == 1.5));
        assert!(matches!(r("d[1].b"), Ok(Resolved::Int(1))));
        assert!(matches!(r("d[1].s"), Ok(Resolved::Int(1))));
        assert!(matches!(r("d[1].n"), Ok(Resolved::Int(1))));
        assert!(
            r("d[99999999999999999999].n")
                .unwrap_err()
                .contains("BigInteger")
        );
        assert!(
            r("dt[2023-08-01T00:00:00Z]")
                .unwrap_err()
                .contains("datetime")
        );
        assert!(r("D[OUT]").is_err());
        assert!(r("t[id]").is_err());
        let edge_map = r(r#"m[{"t[id]": "d[7].i", "D[IN]": "v[vadas]"}]"#);
        assert!(edge_map.unwrap_err().contains("D[IN]"));
        assert_eq!(
            rr("d[5].n"),
            Resolved::Int(5),
            "in-range BigInteger is an integer"
        );
    }

    #[test]
    fn numbers_compare_by_class_only() {
        for text in ["d[1].b", "d[1].s", "d[1].i", "d[1].l", "d[1].n"] {
            assert!(matches(&rr(text), &i(1), K), "{text}");
            assert!(!matches(&rr(text), &f(1.0), K), "{text} vs float");
        }
        for text in ["d[1.0].f", "d[1.0].d"] {
            assert!(matches(&rr(text), &f(1.0), K), "{text}");
            assert!(!matches(&rr(text), &i(1), K), "{text} vs int");
        }
        assert!(
            !matches(&rr("d[0.5].d"), &f(0.5000001), K),
            "floats beyond 1 ulp differ"
        );
        let nan = Resolved::Float(f64::NAN);
        assert!(matches(&nan, &f(f64::NAN), K), "NaN equals NaN");
        assert!(matches(&rr("d[NaN]"), &f(f64::NAN), K));
        assert!(matches(&rr("d[-Infinity]"), &f(f64::NEG_INFINITY), K));
        assert!(
            !matches(&rr("d[1].i"), &s("1"), K),
            "a number is not a string"
        );
    }

    #[test]
    fn one_ulp_float_tolerance() {
        let up = |x: f64, n: u64| f64::from_bits(x.to_bits() + n);
        for x in [
            1.0,
            -0.7568024953079283_f64.abs(),
            1e300,
            5e-324,
            f64::MAX / 2.0,
        ] {
            assert!(floats_within_one_ulp(x, x));
            assert!(floats_within_one_ulp(x, up(x, 1)), "{x} adjacent");
            assert!(floats_within_one_ulp(up(x, 1), x), "{x} adjacent reversed");
            assert!(!floats_within_one_ulp(x, up(x, 2)), "{x} 2 ulp apart");
            assert!(!floats_within_one_ulp(x, -x), "{x} sign differs");
        }
        assert!(floats_within_one_ulp(f64::NAN, f64::NAN));
        assert!(!floats_within_one_ulp(f64::NAN, 1.0));
        assert!(!floats_within_one_ulp(f64::INFINITY, f64::NAN));
        assert!(floats_within_one_ulp(f64::INFINITY, f64::INFINITY));
        assert!(!floats_within_one_ulp(f64::INFINITY, f64::NEG_INFINITY));
        assert!(!floats_within_one_ulp(f64::INFINITY, f64::MAX));
        assert!(floats_within_one_ulp(0.0, -0.0));
        assert!(
            !floats_within_one_ulp(5e-324, -5e-324),
            "tiny values of opposite sign"
        );
        assert!(floats_within_one_ulp(
            -0.7568024953079283,
            -0.7568024953079282
        ));
        // Through `matches`: only float vs float, integers stay exact.
        assert!(matches(&Resolved::Float(1.0), &f(up(1.0, 1)), K));
        assert!(!matches(&Resolved::Float(1.0), &f(up(1.0, 2)), K));
        assert!(!matches(&Resolved::Int(1), &f(1.0), K));
        assert!(!matches(&Resolved::Int(i64::MAX - 1), &i(i64::MAX), K));
    }

    #[test]
    fn id_tolerance_applies_only_in_id_positions() {
        // Scenario-wide id mode: an integer matches its decimal string.
        assert!(matches(&rr("d[1].i"), &s("1"), S));
        assert!(matches(&rr("d[7].l"), &s("7"), S));
        assert!(!matches(&rr("d[1].i"), &s("01"), S));
        assert!(
            !matches(&rr("d[1.0].d"), &s("1"), S),
            "floats are never ids"
        );
        // Keys-only mode: only under t[id].
        assert!(!matches(&rr("d[1].i"), &s("1"), K));
        let map = rr(r#"m[{"t[id]": "d[1].i", "t[label]": "person", "age": "d[29].i"}]"#);
        let actual = obj(&[("id", s("1")), ("label", s("person")), ("age", i(29))]);
        assert!(matches(&map, &actual, K));
        let wrong_age = obj(&[("id", s("1")), ("label", s("person")), ("age", s("29"))]);
        assert!(
            !matches(&map, &wrong_age, K),
            "tolerance does not leak to other keys"
        );
    }

    #[test]
    fn collections() {
        let list = rr("l[d[1].i,d[2].i]");
        assert!(matches(&list, &arr(vec![i(1), i(2)]), K));
        assert!(
            !matches(&list, &arr(vec![i(2), i(1)]), K),
            "lists are ordered"
        );
        let set = rr("s[d[1].i,d[2].i,d[2].i]");
        assert!(matches(&set, &arr(vec![i(2), i(1), i(2)]), K));
        assert!(
            !matches(&set, &arr(vec![i(2), i(1), i(1)]), K),
            "multiset counts"
        );
        let path = rr("p[v[marko],v[vadas],vadas]");
        assert!(matches(
            &path,
            &arr(vec![vertex("1"), vertex("2"), s("vadas")]),
            K
        ));
        assert!(!matches(
            &path,
            &arr(vec![vertex("2"), vertex("1"), s("vadas")]),
            K
        ));
        assert!(matches(&rr("null"), &ElementProperty::Null, K));
        assert!(!matches(&rr("null"), &s("null"), K));
    }

    #[test]
    fn maps_compare_by_key_set_and_values() {
        let map = rr(r#"m[{"a": "d[1].i", "b": "x"}]"#);
        assert!(
            matches(&map, &obj(&[("b", s("x")), ("a", i(1))]), K),
            "order ignored"
        );
        assert!(!matches(&map, &obj(&[("a", i(1))]), K), "key set");
        assert!(!matches(
            &map,
            &obj(&[("a", i(1)), ("b", s("x")), ("c", i(1))]),
            K
        ));
        assert!(!matches(&map, &obj(&[("a", i(2)), ("b", s("x"))]), K));
        // A non-string key is compared as a value against the actual (string) key.
        let by_age = rr(r#"m[{"d[29].i": "d[1].l"}]"#);
        assert!(
            !matches(&by_age, &obj(&[("29", i(1))]), K),
            "29 is not \"29\""
        );
        let by_vertex = rr(r#"m[{"v[marko]": "d[1].l"}]"#);
        assert!(
            !matches(&by_vertex, &obj(&[("1", i(1))]), S),
            "no rendered keys"
        );
    }

    #[test]
    fn multiset_matching_finds_a_perfect_pairing() {
        // Greedy would pair d[1].i with "1" and then fail on id "1"; augmenting fixes it.
        let expected = vec![rr("d[1].i"), rr("v[marko].id")];
        assert!(multiset_diff(&expected, &[s("1"), i(1)], S).is_none());
        let diff = multiset_diff(&[rr("d[1].i")], &[i(2)], K).expect("differs");
        assert_eq!(diff.missing, vec![Resolved::Int(1)]);
        assert_eq!(diff.unexpected, vec![i(2)]);
    }

    #[test]
    fn uuids_and_scalars() {
        let u = rr("uuid[00000000-0000-0000-0000-00000000002a]");
        assert!(matches(&u, &ElementProperty::Uuid(42), K));
        assert!(matches(&rr("marko"), &s("marko"), K));
        assert!(matches(&rr("str[  lop]"), &s("  lop"), K));
        assert!(matches(
            &Resolved::Bool(true),
            &ElementProperty::Boolean(true),
            K
        ));
    }

    #[test]
    fn script_scans_ignore_string_literals() {
        assert!(script_reads_ids("g.V().id()"));
        assert!(script_reads_ids("g.V().elementMap().by(T.id)"));
        assert!(!script_reads_ids("g.V().values(\"id()\")"));
        assert!(!script_reads_ids("g.V().values(\"idx\")"));
        assert!(!script_reads_ids("g.V().has(T.identity, 1)"));
        assert!(script_orders("g.V().order().by(\"name\")"));
        assert!(script_orders("g.V().group().by (\"name\")"));
        assert!(!script_orders("g.V().values(\"order(\")"));
        assert!(!script_orders("g.V().values(\"name\")"));
    }

    #[test]
    fn short_renderings() {
        assert_eq!(
            short(&arr(vec![vertex("1"), i(2), f(0.5), s("x")])),
            "[v[1], 2, 0.5, \"x\"]"
        );
        assert_eq!(short_expected(&rr("s[v[marko],d[1].i]")), "s[v[1], 1]");
    }

    fn forest(text: &str) -> Result<Vec<TreeNode>, String> {
        parse_tree(text, &modern())
    }

    fn map(entries: Vec<(ElementProperty, ElementProperty)>) -> ElementProperty {
        ElementProperty::from_entries(entries)
    }

    fn leaf() -> ElementProperty {
        obj(&[])
    }

    #[test]
    fn tree_outline_parses_into_a_forest() {
        let t = forest("|--v[marko]\n   |--v[vadas]\n   |--v[lop]\n|--v[vadas]").expect("parses");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].children.len(), 2);
        assert!(t[1].children.is_empty());
    }

    #[test]
    fn malformed_tree_outlines_are_rejected() {
        assert!(forest("marko").is_err());
        assert!(forest("|--marko\n  |--x").is_err());
        assert!(forest("|--marko\n         |--x").is_err());
        assert_eq!(forest("").expect("empty tree"), vec![]);
        assert!(forest("|--v[nobody]").is_err());
    }

    #[test]
    fn string_keyed_tree_matches_and_ignores_sibling_order() {
        let t = forest("|--marko\n   |--vadas\n   |--lop").expect("parses");
        let a = obj(&[("marko", obj(&[("lop", leaf()), ("vadas", leaf())]))]);
        assert!(compare_tree(&t, &a, IdMode::KeysOnly));
    }

    #[test]
    fn tree_mismatches_are_detected() {
        let t = forest("|--marko\n   |--vadas").expect("parses");
        let extra = obj(&[("marko", obj(&[("vadas", leaf()), ("lop", leaf())]))]);
        let missing = obj(&[("marko", leaf())]);
        let wrong = obj(&[("marko", obj(&[("lop", leaf())]))]);
        let deeper = obj(&[("marko", obj(&[("vadas", obj(&[("x", leaf())]))]))]);
        for a in [extra, missing, wrong, deeper, leaf(), i(1)] {
            assert!(
                !compare_tree(&t, &a, IdMode::KeysOnly),
                "{}",
                short_tree(&a)
            );
        }
    }

    #[test]
    fn typed_keys_match_vertices_and_numbers() {
        let t = forest("|--v[marko]\n   |--d[2].i\n|--d[7].i").expect("parses");
        let a = map(vec![
            (i(7), leaf()),
            (vertex("1"), map(vec![(i(2), leaf())])),
        ]);
        assert!(compare_tree(&t, &a, IdMode::KeysOnly));
        let float = map(vec![
            (f(7.0), leaf()),
            (vertex("1"), map(vec![(i(2), leaf())])),
        ]);
        assert!(!compare_tree(&t, &float, IdMode::KeysOnly));
    }
}
