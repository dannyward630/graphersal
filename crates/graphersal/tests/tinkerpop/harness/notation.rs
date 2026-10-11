//! Parser for TinkerPop 3.8.2's expected-value notation.
//!
//! The notation appears in result tables, in `using the parameter ... defined as "..."` and in
//! `using the side effect ... defined as "..."`. [`parse`] turns one value into a typed,
//! graph-independent [`Expected`] tree; resolving `v[marko]` against a loaded graph happens later.
//!
//! Forms (measured over the vendored suite) and how they are modelled:
//! - `v[name]` / `v[name].id` / `v[name].sid` → [`Expected::Vertex`] / [`Expected::VertexId`] /
//!   [`Expected::VertexSid`] (`.sid` is the id as a string, used by the `hasId("1")`-style
//!   scenarios).
//! - `e[out-label->in]` / `e[...].id` / `e[...].sid` → [`Expected::Edge`] / [`Expected::EdgeId`] /
//!   [`Expected::EdgeSid`].
//! - `vp[vertex-key->value]` → [`Expected::VertexProperty`]; the value is notation itself.
//! - `d[n].i/.l/.s/.b/.n` → [`Expected::Int`]; `d[n].f/.d` → [`Expected::Float`];
//!   `d[n].m` → [`Expected::BigDecimal`]; `d[NaN]`, `d[Infinity]`, `d[-Infinity]` (no suffix in
//!   the suite) → a double [`Expected::Float`].
//! - `l[..]`, `s[..]`, `p[..]` → list / set / path; elements are split on top-level commas only
//!   and are not trimmed. `l[]`, `s[]`, `p[]` are empty.
//! - `m[{..}]` → [`Expected::Map`] in JSON key order; the body is JSON whose keys and string
//!   values are notation. Non-string JSON values map naturally: integers → `Int` (width `Int`
//!   when it fits `i32`, else `Long`), other numbers → double `Float`, booleans →
//!   [`Expected::Bool`], `null` → `Null`, arrays → `List`, objects → `Map`.
//! - `t[id]`, `t[label]` → [`Expected::TokenT`].
//! - `D[OUT]`, `D[IN]`, and their aliases `D[from]`, `D[to]` → [`Expected::Direction`].
//! - `M[outV]`, `M[inV]` (the `Merge` tokens used in `mergeE()` parameter maps) →
//!   [`Expected::Merge`].
//! - `str[...]` → [`Expected::Str`] with the bracket content taken **verbatim** (it is the
//!   upstream way to write a string with significant whitespace or bracket characters, e.g.
//!   `str[  lop]` or `str[v[1]]` — the latter is the literal text `v[1]`, not a vertex).
//! - `dt[...]` → [`Expected::DateTime`], `uuid[...]` → [`Expected::Uuid`] (raw text).
//! - `null` → [`Expected::Null`]; bare `true` / `false` → [`Expected::Bool`]; anything else →
//!   [`Expected::Str`].
//!
//! Table cells are still Gherkin-escaped (`\\`, `\|`); use [`parse_cell`] for them. Parameter
//! and side-effect values are already unescaped by the Gherkin parser; use [`parse`].

use std::fmt;

/// A parsed expected value.
#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    /// `v[name]`: the vertex whose `name` property is `name`.
    Vertex(String),
    /// `v[name].id`: that vertex's id.
    VertexId(String),
    /// `v[name].sid`: that vertex's id rendered as a string.
    VertexSid(String),
    /// `e[out-label->in]`: the edge identified by its endpoints' names and its label.
    Edge {
        /// Name of the out-vertex.
        out: String,
        /// Edge label.
        label: String,
        /// Name of the in-vertex.
        inv: String,
    },
    /// `e[out-label->in].id`: that edge's id.
    EdgeId {
        /// Name of the out-vertex.
        out: String,
        /// Edge label.
        label: String,
        /// Name of the in-vertex.
        inv: String,
    },
    /// `e[out-label->in].sid`: that edge's id rendered as a string.
    EdgeSid {
        /// Name of the out-vertex.
        out: String,
        /// Edge label.
        label: String,
        /// Name of the in-vertex.
        inv: String,
    },
    /// `vp[vertex-key->value]`: a vertex property of the named vertex.
    VertexProperty {
        /// Name of the owning vertex.
        vertex: String,
        /// Property key.
        key: String,
        /// Property value (notation).
        value: Box<Expected>,
    },
    /// An integral number with its declared width.
    Int {
        /// The value (`i128` holds every `BigInteger` literal in the suite).
        value: i128,
        /// Declared width.
        width: IntWidth,
    },
    /// A floating point number with its declared width.
    Float {
        /// The value.
        value: f64,
        /// Declared width.
        width: FloatWidth,
    },
    /// `d[x].m`: a `BigDecimal`, kept as its literal text.
    BigDecimal(String),
    /// A boolean (only as a non-string JSON value inside `m[...]`).
    Bool(bool),
    /// `l[...]`.
    List(Vec<Expected>),
    /// `s[...]`.
    Set(Vec<Expected>),
    /// `m[{...}]`, entries in source order.
    Map(Vec<(Expected, Expected)>),
    /// `p[...]`.
    Path(Vec<Expected>),
    /// `t[id]` / `t[label]`.
    TokenT(TToken),
    /// `D[OUT]` / `D[IN]` (and the aliases `D[from]` / `D[to]`).
    Direction(Dir),
    /// `M[outV]` / `M[inV]`.
    Merge(MergeToken),
    /// `dt[...]`: a datetime, raw ISO-8601 text.
    DateTime(String),
    /// `uuid[...]`: raw UUID text.
    Uuid(String),
    /// `null`.
    Null,
    /// A plain string, or the verbatim content of `str[...]`.
    Str(String),
}

/// Declared width of an integral number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntWidth {
    /// `.b`
    Byte,
    /// `.s`
    Short,
    /// `.i`
    Int,
    /// `.l`
    Long,
    /// `.n` (`BigInteger`)
    BigInt,
}

/// Declared width of a floating point number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatWidth {
    /// `.f`
    Float,
    /// `.d` (and the suffix-less `d[NaN]` / `d[Infinity]` forms)
    Double,
}

/// A `T` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TToken {
    /// `t[id]`
    Id,
    /// `t[label]`
    Label,
}

/// A `Direction` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    /// `D[OUT]` or `D[from]`
    Out,
    /// `D[IN]` or `D[to]`
    In,
}

/// A `Merge` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeToken {
    /// `M[outV]`
    OutV,
    /// `M[inV]`
    InV,
}

/// Why a notation value could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotationError {
    /// What went wrong, including the offending text.
    pub message: String,
}

impl NotationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for NotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NotationError {}

/// Undoes Gherkin table-cell escaping: `\\` → `\`, `\|` → `|`, `\n` → newline. Any other
/// backslash sequence is kept verbatim.
pub fn unescape_cell(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('|') => out.push('|'),
            Some('n') => out.push('\n'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Parses a raw (still Gherkin-escaped) table cell.
pub fn parse_cell(raw: &str) -> Result<Expected, NotationError> {
    parse(&unescape_cell(raw))
}

/// Parses one notation value.
pub fn parse(text: &str) -> Result<Expected, NotationError> {
    match text {
        "null" => return Ok(Expected::Null),
        // Upstream runners read bare `true` / `false` as booleans (e.g. `AsBool.feature`,
        // `Orderability.feature` result tables); a string would be written `str[true]`.
        "true" => return Ok(Expected::Bool(true)),
        "false" => return Ok(Expected::Bool(false)),
        _ => {}
    }
    let Some(open) = text.find('[') else {
        return Ok(Expected::Str(text.to_string()));
    };
    let prefix = &text[..open];
    if !matches!(
        prefix,
        "v" | "e" | "vp" | "d" | "l" | "s" | "m" | "p" | "t" | "D" | "M" | "str" | "dt" | "uuid"
    ) {
        return Ok(Expected::Str(text.to_string()));
    }
    let close = matching_bracket(text, open)
        .ok_or_else(|| NotationError::new(format!("unbalanced brackets in `{text}`")))?;
    let body = &text[open + 1..close];
    let suffix = &text[close + 1..];
    let no_suffix = |value: Expected| -> Result<Expected, NotationError> {
        if suffix.is_empty() {
            Ok(value)
        } else {
            Err(NotationError::new(format!(
                "unexpected trailing `{suffix}` in `{text}`"
            )))
        }
    };
    match prefix {
        "v" => match suffix {
            "" => Ok(Expected::Vertex(body.to_string())),
            ".id" => Ok(Expected::VertexId(body.to_string())),
            ".sid" => Ok(Expected::VertexSid(body.to_string())),
            _ => Err(NotationError::new(format!(
                "unexpected trailing `{suffix}` in `{text}`"
            ))),
        },
        "e" => {
            let (out, label, inv) = split_edge(body)
                .ok_or_else(|| NotationError::new(format!("malformed edge `{text}`")))?;
            match suffix {
                "" => Ok(Expected::Edge { out, label, inv }),
                ".id" => Ok(Expected::EdgeId { out, label, inv }),
                ".sid" => Ok(Expected::EdgeSid { out, label, inv }),
                _ => Err(NotationError::new(format!(
                    "unexpected trailing `{suffix}` in `{text}`"
                ))),
            }
        }
        "vp" => {
            let (vertex, key, value) = split_edge(body)
                .ok_or_else(|| NotationError::new(format!("malformed vertex property `{text}`")))?;
            no_suffix(Expected::VertexProperty {
                vertex,
                key,
                value: Box::new(parse(&value)?),
            })
        }
        "d" => parse_number(body, suffix, text),
        "l" => no_suffix(Expected::List(parse_elements(body)?)),
        "s" => no_suffix(Expected::Set(parse_elements(body)?)),
        "p" => no_suffix(Expected::Path(parse_elements(body)?)),
        "m" => no_suffix(parse_map(body)?),
        "t" => match body {
            "id" => no_suffix(Expected::TokenT(TToken::Id)),
            "label" => no_suffix(Expected::TokenT(TToken::Label)),
            _ => Err(NotationError::new(format!("unknown T token `{text}`"))),
        },
        "D" => match body {
            "OUT" | "from" => no_suffix(Expected::Direction(Dir::Out)),
            "IN" | "to" => no_suffix(Expected::Direction(Dir::In)),
            _ => Err(NotationError::new(format!("unknown Direction `{text}`"))),
        },
        "M" => match body {
            "outV" => no_suffix(Expected::Merge(MergeToken::OutV)),
            "inV" => no_suffix(Expected::Merge(MergeToken::InV)),
            _ => Err(NotationError::new(format!("unknown Merge token `{text}`"))),
        },
        "str" => no_suffix(Expected::Str(body.to_string())),
        "dt" => no_suffix(Expected::DateTime(body.to_string())),
        "uuid" => no_suffix(Expected::Uuid(body.to_string())),
        _ => Ok(Expected::Str(text.to_string())),
    }
}

/// Byte index of the `]` matching the `[` at `open`. Brackets inside JSON strings (within `{}`)
/// are ignored.
fn matching_bracket(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut braces = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in text.char_indices().skip_while(|(i, _)| *i < open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' if braces > 0 => in_string = true,
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits `a-b->c` into `(a, b, c)`, taking the first `->` and then the first `-` before it.
fn split_edge(body: &str) -> Option<(String, String, String)> {
    let (left, right) = body.split_once("->")?;
    let (a, b) = left.split_once('-')?;
    if a.is_empty() || b.is_empty() || right.is_empty() {
        return None;
    }
    Some((a.to_string(), b.to_string(), right.to_string()))
}

/// Parses the body of `d[...]` with its suffix.
fn parse_number(body: &str, suffix: &str, text: &str) -> Result<Expected, NotationError> {
    let int = |width: IntWidth| -> Result<Expected, NotationError> {
        body.parse::<i128>()
            .map(|value| Expected::Int { value, width })
            .map_err(|e| NotationError::new(format!("bad integer in `{text}`: {e}")))
    };
    let float = |width: FloatWidth| -> Result<Expected, NotationError> {
        body.parse::<f64>()
            .map(|value| Expected::Float { value, width })
            .map_err(|e| NotationError::new(format!("bad float in `{text}`: {e}")))
    };
    match suffix {
        ".b" => int(IntWidth::Byte),
        ".s" => int(IntWidth::Short),
        ".i" => int(IntWidth::Int),
        ".l" => int(IntWidth::Long),
        ".n" => int(IntWidth::BigInt),
        ".f" => float(FloatWidth::Float),
        ".d" => float(FloatWidth::Double),
        ".m" => {
            if body.parse::<f64>().is_err() {
                return Err(NotationError::new(format!("bad BigDecimal in `{text}`")));
            }
            Ok(Expected::BigDecimal(body.to_string()))
        }
        "" if matches!(body, "NaN" | "Infinity" | "-Infinity") => float(FloatWidth::Double),
        _ => Err(NotationError::new(format!(
            "unknown numeric suffix `{suffix}` in `{text}`"
        ))),
    }
}

/// Splits a collection body on top-level commas and parses each element.
fn parse_elements(body: &str) -> Result<Vec<Expected>, NotationError> {
    if body.is_empty() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut braces = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in body.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' if braces > 0 => in_string = true,
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '[' => depth += 1,
            ']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| NotationError::new(format!("unbalanced brackets in `{body}`")))?
            }
            ',' if depth == 0 && braces == 0 => {
                items.push(parse(&body[start..i])?);
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(NotationError::new(format!(
            "unbalanced brackets in `{body}`"
        )));
    }
    items.push(parse(&body[start..])?);
    Ok(items)
}

/// Parses the JSON body of `m[...]`.
fn parse_map(body: &str) -> Result<Expected, NotationError> {
    let node: Json = serde_json::from_str(body)
        .map_err(|e| NotationError::new(format!("bad map JSON `{body}`: {e}")))?;
    match node {
        Json::Object(_) => json_to_expected(node),
        _ => Err(NotationError::new(format!(
            "map body is not a JSON object: `{body}`"
        ))),
    }
}

/// Converts a JSON node to [`Expected`], parsing strings as notation.
fn json_to_expected(node: Json) -> Result<Expected, NotationError> {
    Ok(match node {
        Json::Null => Expected::Null,
        Json::Bool(b) => Expected::Bool(b),
        Json::Int(v) => Expected::Int {
            value: v,
            width: if i32::try_from(v).is_ok() {
                IntWidth::Int
            } else {
                IntWidth::Long
            },
        },
        Json::Float(value) => Expected::Float {
            value,
            width: FloatWidth::Double,
        },
        Json::String(s) => parse(&s)?,
        Json::Array(items) => Expected::List(
            items
                .into_iter()
                .map(json_to_expected)
                .collect::<Result<_, _>>()?,
        ),
        Json::Object(entries) => Expected::Map(
            entries
                .into_iter()
                .map(|(k, v)| Ok((parse(&k)?, json_to_expected(v)?)))
                .collect::<Result<_, NotationError>>()?,
        ),
    })
}

/// A JSON value that keeps object entries in source order (and duplicates), unlike
/// `serde_json::Value` without the `preserve_order` feature.
enum Json {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl<'de> serde::Deserialize<'de> for Json {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;

        impl<'de> serde::de::Visitor<'de> for JsonVisitor {
            type Value = Json;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Int(i128::from(v)))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Int(i128::from(v)))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
                Ok(Json::Float(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::String(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<Json, E> {
                Ok(Json::String(v))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Json::Array(items))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
                let mut entries = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Json>()? {
                    entries.push((k, v));
                }
                Ok(Json::Object(entries))
            }
        }

        deserializer.deserialize_any(JsonVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> Expected {
        parse(text).unwrap_or_else(|e| panic!("parse `{text}`: {e}"))
    }

    fn s(text: &str) -> Expected {
        Expected::Str(text.to_string())
    }

    fn v(name: &str) -> Expected {
        Expected::Vertex(name.to_string())
    }

    fn int(value: i128, width: IntWidth) -> Expected {
        Expected::Int { value, width }
    }

    #[test]
    fn elements() {
        assert_eq!(p("v[marko]"), v("marko"));
        assert_eq!(p("v[marko].id"), Expected::VertexId("marko".into()));
        let edge = || ("marko".to_string(), "knows".to_string(), "josh".to_string());
        let (out, label, inv) = edge();
        assert_eq!(
            p("e[marko-knows->josh]"),
            Expected::Edge { out, label, inv }
        );
        let (out, label, inv) = edge();
        assert_eq!(
            p("e[marko-knows->josh].id"),
            Expected::EdgeId { out, label, inv }
        );
        let (out, label, inv) = edge();
        assert_eq!(
            p("e[marko-knows->josh].sid"),
            Expected::EdgeSid { out, label, inv }
        );
        assert_eq!(p("v[marko].sid"), Expected::VertexSid("marko".into()));
    }

    #[test]
    fn vertex_properties() {
        assert_eq!(
            p("vp[marko-name->marko]"),
            Expected::VertexProperty {
                vertex: "marko".into(),
                key: "name".into(),
                value: Box::new(s("marko")),
            }
        );
        assert_eq!(
            p("vp[marko-age->d[29].i]"),
            Expected::VertexProperty {
                vertex: "marko".into(),
                key: "age".into(),
                value: Box::new(int(29, IntWidth::Int)),
            }
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(p("d[29].i"), int(29, IntWidth::Int));
        assert_eq!(p("d[-2147483649].l"), int(-2147483649, IntWidth::Long));
        assert_eq!(p("d[-129].s"), int(-129, IntWidth::Short));
        assert_eq!(p("d[1].b"), int(1, IntWidth::Byte));
        assert_eq!(p("d[1123].n"), int(1123, IntWidth::BigInt));
        assert_eq!(
            p("d[0.5].d"),
            Expected::Float {
                value: 0.5,
                width: FloatWidth::Double
            }
        );
        assert_eq!(
            p("d[4.294967294e+09].f"),
            Expected::Float {
                value: 4294967294.0,
                width: FloatWidth::Float
            }
        );
        assert_eq!(p("d[0.5].m"), Expected::BigDecimal("0.5".into()));
        match p("d[NaN]") {
            Expected::Float { value, width } => {
                assert!(value.is_nan());
                assert_eq!(width, FloatWidth::Double);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(
            p("d[-Infinity]"),
            Expected::Float {
                value: f64::NEG_INFINITY,
                width: FloatWidth::Double
            }
        );
        assert_eq!(
            p("d[Infinity]"),
            Expected::Float {
                value: f64::INFINITY,
                width: FloatWidth::Double
            }
        );
    }

    #[test]
    fn collections() {
        assert_eq!(p("l[a,b,c]"), Expected::List(vec![s("a"), s("b"), s("c")]));
        assert_eq!(p("s[x,null]"), Expected::Set(vec![s("x"), Expected::Null]));
        assert_eq!(
            p("p[v[marko],v[josh],v[lop]]"),
            Expected::Path(vec![v("marko"), v("josh"), v("lop")])
        );
        assert_eq!(
            p("l[l[1,2],l[3]]"),
            Expected::List(vec![
                Expected::List(vec![s("1"), s("2")]),
                Expected::List(vec![s("3")]),
            ])
        );
        assert_eq!(
            p("l[p[v[marko],v[josh]],p[v[lop]]]"),
            Expected::List(vec![
                Expected::Path(vec![v("marko"), v("josh")]),
                Expected::Path(vec![v("lop")]),
            ])
        );
        assert_eq!(
            p(r#"l[m[{"a":"b","c":"d"}],x]"#),
            Expected::List(vec![
                Expected::Map(vec![(s("a"), s("b")), (s("c"), s("d"))]),
                s("x"),
            ])
        );
    }

    #[test]
    fn empty_collections() {
        assert_eq!(p("l[]"), Expected::List(vec![]));
        assert_eq!(p("s[]"), Expected::Set(vec![]));
        assert_eq!(p("p[]"), Expected::Path(vec![]));
        assert_eq!(p("m[{}]"), Expected::Map(vec![]));
    }

    #[test]
    fn maps() {
        assert_eq!(
            p(r#"m[{"t[id]": "v[josh].id", "t[label]": "person", "age": "d[32].i"}]"#),
            Expected::Map(vec![
                (
                    Expected::TokenT(TToken::Id),
                    Expected::VertexId("josh".into())
                ),
                (Expected::TokenT(TToken::Label), s("person")),
                (s("age"), int(32, IntWidth::Int)),
            ])
        );
        assert_eq!(
            p(r#"m[{"a":"a", "b":false, "c":null, "n":[3.0, 1], "o":{"k":"d[1].l"}}]"#),
            Expected::Map(vec![
                (s("a"), s("a")),
                (s("b"), Expected::Bool(false)),
                (s("c"), Expected::Null),
                (
                    s("n"),
                    Expected::List(vec![
                        Expected::Float {
                            value: 3.0,
                            width: FloatWidth::Double
                        },
                        int(1, IntWidth::Int),
                    ])
                ),
                (
                    s("o"),
                    Expected::Map(vec![(s("k"), int(1, IntWidth::Long))])
                ),
            ])
        );
    }

    #[test]
    fn nested_map_with_double_escaping() {
        // As written in a feature table cell.
        let cell = r#"m[{"a":"m[{\\"d[32].i\\":\\"d[1].l\\"}]"}]"#;
        assert_eq!(
            parse_cell(cell).expect("parse"),
            Expected::Map(vec![(
                s("a"),
                Expected::Map(vec![(int(32, IntWidth::Int), int(1, IntWidth::Long))])
            )])
        );
    }

    #[test]
    fn tokens() {
        assert_eq!(p("t[id]"), Expected::TokenT(TToken::Id));
        assert_eq!(p("t[label]"), Expected::TokenT(TToken::Label));
        assert_eq!(p("D[OUT]"), Expected::Direction(Dir::Out));
        assert_eq!(p("D[IN]"), Expected::Direction(Dir::In));
        assert_eq!(p("D[from]"), Expected::Direction(Dir::Out));
        assert_eq!(p("D[to]"), Expected::Direction(Dir::In));
        assert_eq!(p("M[outV]"), Expected::Merge(MergeToken::OutV));
        assert_eq!(p("M[inV]"), Expected::Merge(MergeToken::InV));
    }

    #[test]
    fn strings_and_scalars() {
        assert_eq!(p("str[v[1]]"), s("v[1]"));
        assert_eq!(p("str[  lop]"), s("  lop"));
        assert_eq!(p("str[]"), s(""));
        assert_eq!(p("str[e[10][4-created->5]]"), s("e[10][4-created->5]"));
        assert_eq!(
            p("l[str[ marko],null,str[]]"),
            Expected::List(vec![s(" marko"), Expected::Null, s("")])
        );
        assert_eq!(
            p("dt[2023-01-01T00:00:00Z]"),
            Expected::DateTime("2023-01-01T00:00:00Z".into())
        );
        assert_eq!(
            p("uuid[f47af10b-58cc-4372-a567-0f02b2f3d479]"),
            Expected::Uuid("f47af10b-58cc-4372-a567-0f02b2f3d479".into())
        );
        assert_eq!(p("null"), Expected::Null);
        assert_eq!(p("true"), Expected::Bool(true));
        assert_eq!(p("false"), Expected::Bool(false));
        assert_eq!(p("str[true]"), Expected::Str("true".into()));
        assert_eq!(p("marko"), s("marko"));
        assert_eq!(p("1"), s("1"));
        assert_eq!(p(""), s(""));
    }

    #[test]
    fn errors() {
        assert!(parse("v[marko").is_err());
        assert!(parse("l[a,b").is_err());
        assert!(parse("l[a]]").is_err());
        assert!(parse("d[1].x").is_err());
        assert!(parse("d[1]").is_err());
        assert!(parse("d[abc].i").is_err());
        assert!(parse("v[marko].name").is_err());
        assert!(parse("t[key]").is_err());
        assert!(parse("e[marko]").is_err());
        assert!(parse("m[{\"a\":}]").is_err());
        assert!(parse("m[[1]]").is_err());
    }

    #[test]
    fn unescape() {
        assert_eq!(unescape_cell(r"a\|b\\c\nd\x"), "a|b\\c\nd\\x");
    }
}
