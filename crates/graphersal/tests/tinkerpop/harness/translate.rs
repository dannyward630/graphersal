//! Translator from the canonical Gremlin of the TinkerPop 3.8.2 feature files to a script the
//! Graphersal Rhai DSL can evaluate.
//!
//! # Principle
//!
//! Only **lexical** differences between the Gremlin grammar (`gremlin-language`, `Gremlin.g4`) and
//! Rhai are translated. A missing step, overload or token is never renamed, dropped or emulated: it
//! reaches the engine unchanged and fails there (`Function not found: ...` /
//! `Variable not found: ...`), so that it is counted against that name. A literal of a type
//! Graphersal does not have yields [`Translation::Unsupported`].
//!
//! The input is tokenized first (strings, numbers with suffixes, identifiers, punctuation,
//! whitespace, comments); rules operate on tokens, so the content of a string literal is never
//! rewritten. Whitespace and line breaks are preserved.
//!
//! # Rules
//!
//! - **Bare anonymous steps.** A bare identifier followed by `(` at expression position (not after
//!   a `.`) is a step of the grammar's static import of `__` and becomes `__.name(`. Exceptions,
//!   also from the grammar: bare predicates (`gt(`, `within(`, ... → `P.`; `containing(`,
//!   `regex(`, ... → `TextP.`), `single(`/`set(`/`list(` (the `Cardinality.x(value)` form →
//!   `Cardinality.`), and `not(`, which is `P.not(` when its first argument is a predicate
//!   (`P.`/`TextP.`/a bare predicate) and the `__.not(` step otherwise. An identifier starting
//!   with an upper-case letter followed by `(` is a class (strategy) constructor and stays as is.
//! - **Bare enum tokens.** A bare identifier *not* followed by `(` that is a static-import token of
//!   the grammar is qualified with its class; see [`static_import_class`] for the table. The table
//!   was taken from the 3.8.2 grammar (`gremlin-language/src/main/antlr4/Gremlin.g4` at tag
//!   `3.8.2`, rules `traversalScope`, `traversalBarrier`, `traversalTShort`, `traversalMerge`,
//!   `traversalOrder`, `traversalDirectionShort`, `traversalCardinality`, `traversalColumn`,
//!   `traversalPop`, `traversalOperator`, `traversalPick`, `traversalDT`, `traversalGType`). The
//!   ambiguous `set`/`list` (`Cardinality` and `GType`) resolve to `GType` inside `typeOf(...)`
//!   and to `Cardinality` everywhere else. `null` is always the null literal (the grammar's
//!   `genericLiteral` alternative order makes it so), never `GType.null`.
//! - **Qualified tokens** (`Scope.local`, `Pop.all`, `T.id`, ...) pass through literally.
//! - **Numbers.** `i`/`l`/`L` suffixes are dropped (Rhai integer); `d`/`D`/`f`/`F` suffixes are
//!   dropped and a decimal point is ensured (`1d` → `1.0`, `1E+308d` → `1.0e+308`). `b` (byte),
//!   `s` (short), `n` (BigInteger), `m` (BigDecimal) and unsuffixed integers outside `int64` are
//!   unsupported literal types. Octal (`017`) and hex (`0x1F`) integers are rewritten to decimal,
//!   because Rhai reads a leading `0` as decimal.
//! - **Strings.** Single- and double-quoted strings are unescaped with the grammar's escape rules
//!   and re-emitted double-quoted with Rhai escaping.
//! - **Collections.** `[1, 2]` stays a Rhai array. `[a: 1]`, `["a": 1]`, `[("a"): 1]` →
//!   `#{"a": 1}`; `[:]` → `#{}`. A naked map key is a string in the grammar (`keyword | nakedKey`),
//!   never a variable, so it is not subject to parameter renaming. Token keys become the DSL's
//!   reserved string keys: `T.label`, `(T.id)`, `(label)` → `"T.label"`/`"T.id"` (`T.key`/`T.value`
//!   likewise, which the engine rejects); `Direction.OUT`, `(OUT)`, `(from)`, `(Direction.to)` →
//!   `"Direction.OUT"`/`"Direction.IN"` (`BOTH` → `"Direction.BOTH"`). Any other key that is not
//!   a string (`Merge.outV`, numbers, collections) cannot be a Rhai object-map key →
//!   `Unsupported("map literal with non-string key")`. `{..}` is a set literal →
//!   `Unsupported("unsupported literal type: set")`.
//! - **Other literals.** `null` → `()`. `NaN`/`Infinity` stay identifiers (bound as scope
//!   constants by the executor). `datetime(..)`/`DateTime(..)` stay as they are (the DSL's
//!   datetime constructors, also inside a child traversal: never prefixed with `__.`).
//!   `UUID("..")` → `UUID.fromString("..")`; a bare `UUID()` (random UUID) is left as is and fails
//!   in the engine.
//! - **Strategies.** `new` is dropped; named arguments become one map argument:
//!   `new SubgraphStrategy(vertices: __.has(..))` → `SubgraphStrategy(#{"vertices": __.has(..)})`.
//!   Bare class references (`withoutStrategies(CountStrategy)`) stay identifiers.
//! - **Range literals** (`1..3`, `'a'..'c'` of `genericRangeLiteral`) do not occur in the suite;
//!   `..` is passed through unchanged (Rhai has an exclusive integer range with the same spelling,
//!   which differs semantically, so the construct would need a decision if it ever appears).
//! - **Groovy closures / lambdas** do not occur in the 3.8.2 suite (the language grammar has no
//!   lambda syntax); `{` always means a set literal.
//! - **Rhai-reserved step names.** `match` is a reserved word in Rhai and cannot be written as a
//!   method name (`.match(`) — Rhai rejects it at parse time, and the DSL's token remapper does
//!   not cover it. The construct is genuinely inexpressible in Rhai today, so it yields
//!   `Unsupported("step name is a reserved Rhai word: match")` rather than a compile error. `call` is
//!   a Rhai built-in function (it invokes a function pointer), so `.call(` runs that instead of a
//!   step: it is `Unsupported("step name is a Rhai built-in function: call")` as well.
//!
//! # Parameters and side effects
//!
//! Parameter identifiers stay identifiers; one that clashes with a global of the DSL scope or a
//! Rhai keyword is renamed to `p_<name>` consistently (outside string literals, map keys and
//! method names), and every parameter's Gremlin → Rhai name is returned in
//! [`Translation::Ok::param_names`]. For each side effect the leading `g.` of the script becomes
//! `g.withSideEffect("<name>", se_<name>).`, in declaration order; the executor binds `se_<name>`.

use std::collections::HashMap;

/// Result of translating one Gremlin text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Translation {
    /// A Rhai script, plus the Gremlin → Rhai name of every parameter (in `params` order).
    Ok {
        /// The Rhai script.
        script: String,
        /// `(gremlin_name, rhai_name)` for every parameter passed to [`translate`].
        param_names: Vec<(String, String)>,
    },
    /// The text uses a construct that cannot be expressed for Graphersal (e.g. a literal type it
    /// does not have).
    Unsupported {
        /// Human-readable reason, stable enough to group by (e.g. `unsupported literal type: byte`).
        reason: String,
    },
}

/// Globals the DSL's `graph_scope` defines (plus `NaN`/`Infinity`, bound by the executor).
const DSL_GLOBALS: &[&str] = &[
    "g",
    "__",
    "_g",
    "P",
    "TextP",
    "V",
    "UUID",
    "Column",
    "keys",
    "values",
    "params",
    "GType",
    "By",
    "CastPolicy",
    "ProfileType",
    "Order",
    "Pick",
    "T",
    "Scope",
    "Cardinality",
    "Direction",
    "Merge",
    "Operator",
    "Barrier",
    "Pop",
    "first",
    "last",
    "all",
    "mixed",
    "NaN",
    "Infinity",
];

/// Rhai keywords and reserved words that cannot be used as variable names.
const RHAI_RESERVED: &[&str] = &[
    "true",
    "false",
    "let",
    "const",
    "if",
    "else",
    "switch",
    "do",
    "while",
    "until",
    "loop",
    "for",
    "in",
    "continue",
    "break",
    "return",
    "throw",
    "try",
    "catch",
    "import",
    "export",
    "as",
    "global",
    "private",
    "fn",
    "Fn",
    "call",
    "curry",
    "this",
    "type_of",
    "print",
    "debug",
    "eval",
    "is_def_var",
    "is_def_fn",
    "is_shared",
    "var",
    "static",
    "shared",
    "goto",
    "exit",
    "match",
    "case",
    "public",
    "protected",
    "new",
    "use",
    "with",
    "module",
    "package",
    "super",
    "spawn",
    "thread",
    "go",
    "sync",
    "async",
    "await",
    "yield",
    "default",
    "void",
    "null",
    "nil",
    "is",
    "where",
];

/// Step names that are reserved in Rhai and that the DSL's token remapper does not turn into
/// identifiers after a `.`; such a method call cannot be parsed at all.
const RHAI_RESERVED_STEPS: &[&str] = &["match"];

/// Step names that are Rhai built-in functions (`call` invokes a function pointer): such a call
/// parses, but runs Rhai's built-in instead of a step, so it cannot be expressed either. The
/// step is unsupported (Graphersal has no `call()` step; a future one needs a DSL name of its own).
const RHAI_BUILTIN_STEPS: &[&str] = &["call"];

/// The untranslatable reason of a step named like a Rhai reserved word or built-in, if it is one.
fn rhai_step_clash(name: &str) -> Option<String> {
    if RHAI_RESERVED_STEPS.contains(&name) {
        Some(format!("step name is a reserved Rhai word: {name}"))
    } else if RHAI_BUILTIN_STEPS.contains(&name) {
        Some(format!("step name is a Rhai built-in function: {name}"))
    } else {
        None
    }
}

/// Bare predicate names of `P` (grammar rules `traversalPredicate__*`).
const P_PREDICATES: &[&str] = &[
    "eq", "neq", "lt", "lte", "gt", "gte", "inside", "outside", "between", "within", "without",
    "typeOf",
];

/// Bare predicate names of `TextP`.
const TEXTP_PREDICATES: &[&str] = &[
    "containing",
    "notContaining",
    "startingWith",
    "notStartingWith",
    "endingWith",
    "notEndingWith",
    "regex",
    "notRegex",
];

/// The static-import class of a bare token (identifier not followed by `(`), per the 3.8.2
/// grammar. `set`/`list` are resolved by the caller (`Cardinality` or `GType`).
fn static_import_class(name: &str) -> Option<&'static str> {
    Some(match name {
        "asc" | "desc" | "shuffle" => "Order",
        "local" | "global" => "Scope",
        "id" | "label" | "key" | "value" => "T",
        "keys" | "values" => "Column",
        "first" | "last" | "all" | "mixed" => "Pop",
        "sum" | "minus" | "mult" | "div" | "min" | "max" | "assign" | "and" | "or" | "addAll"
        | "sumLong" => "Operator",
        "OUT" | "IN" | "BOTH" | "from" | "to" => "Direction",
        "single" | "set" | "list" => "Cardinality",
        "onCreate" | "onMatch" | "outV" | "inV" => "Merge",
        "normSack" => "Barrier",
        "any" | "none" | "unproductive" => "Pick",
        "second" | "minute" | "hour" | "day" => "DT",
        "bigDecimal" | "BIGDECIMAL" | "bigInt" | "BIGINT" | "binary" | "BINARY" | "boolean"
        | "BOOLEAN" | "byte" | "BYTE" | "char" | "CHAR" | "datetime" | "DATETIME" | "double"
        | "DOUBLE" | "duration" | "DURATION" | "edge" | "EDGE" | "float" | "FLOAT" | "graph"
        | "GRAPH" | "int" | "INT" | "LIST" | "long" | "LONG" | "map" | "MAP" | "NULL"
        | "number" | "NUMBER" | "path" | "PATH" | "property" | "PROPERTY" | "SET" | "short"
        | "SHORT" | "string" | "STRING" | "tree" | "TREE" | "uuid" | "UUID" | "vertex"
        | "VERTEX" | "vproperty" | "VPROPERTY" => "GType",
        _ => return None,
    })
}

/// Translates one Gremlin text (traversal, initializer or `GraphCount` traversal).
///
/// `params` are the scenario's parameter names, `side_effects` its side-effect names (in
/// declaration order; pass an empty slice for texts other than the main traversal).
pub fn translate(gremlin: &str, params: &[String], side_effects: &[String]) -> Translation {
    match translate_inner(gremlin, params, side_effects) {
        Ok((script, param_names)) => Translation::Ok {
            script,
            param_names,
        },
        Err(reason) => Translation::Unsupported { reason },
    }
}

fn translate_inner(
    gremlin: &str,
    params: &[String],
    side_effects: &[String],
) -> Result<(String, Vec<(String, String)>), String> {
    let toks = tokenize(gremlin)?;
    let matching = match_brackets(&toks)?;

    let mut renames = HashMap::new();
    let mut param_names = Vec::with_capacity(params.len());
    for p in params {
        let rhai = if DSL_GLOBALS.contains(&p.as_str()) || RHAI_RESERVED.contains(&p.as_str()) {
            format!("p_{p}")
        } else {
            p.clone()
        };
        renames.insert(p.clone(), rhai.clone());
        param_names.push((p.clone(), rhai));
    }

    let tr = Translator {
        toks: &toks,
        matching: &matching,
        renames: &renames,
    };
    let mut out = String::with_capacity(gremlin.len() + 32);
    tr.emit_range(0, toks.len(), None, &mut out)?;

    if !side_effects.is_empty() {
        out = prefix_side_effects(&out, side_effects)?;
    }
    Ok((out, param_names))
}

/// Rewrites the leading `g.` of `script` into `g.withSideEffect("<name>", se_<name>).` per side
/// effect.
fn prefix_side_effects(script: &str, side_effects: &[String]) -> Result<String, String> {
    let start = script.len() - script.trim_start().len();
    let rest = &script[start..];
    let Some(after) = rest.strip_prefix("g.") else {
        return Err("side effects on a text that does not start with `g.`".to_string());
    };
    let mut out = String::with_capacity(script.len() + 48 * side_effects.len());
    out.push_str(&script[..start]);
    out.push_str("g.");
    for name in side_effects {
        if !is_identifier(name) {
            return Err(format!("side-effect name is not an identifier: {name}"));
        }
        out.push_str(&format!("withSideEffect(\"{name}\", se_{name})."));
    }
    out.push_str(after);
    Ok(out)
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------------------------

/// One lexical token of the Gremlin text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// Whitespace, kept verbatim.
    Ws(String),
    /// A `//` line comment, kept verbatim (without the line break).
    Comment(String),
    /// A string literal, already unescaped.
    Str(String),
    /// A numeric literal, raw text including an optional type suffix (no sign).
    Num(String),
    /// An identifier or keyword.
    Ident(String),
    /// Punctuation: `( ) [ ] { } , . .. : ;` and operators such as `-`.
    Punct(String),
}

impl Tok {
    fn is_trivia(&self) -> bool {
        matches!(self, Tok::Ws(_) | Tok::Comment(_))
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self, Tok::Punct(q) if q == p)
    }
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            toks.push(Tok::Ws(chars[start..i].iter().collect()));
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            let start = i;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            toks.push(Tok::Comment(chars[start..i].iter().collect()));
        } else if c == '"' || c == '\'' {
            let (s, next) = lex_string(&chars, i)?;
            toks.push(Tok::Str(s));
            i = next;
        } else if c.is_ascii_digit()
            || (c == '.'
                && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())
                && !toks
                    .iter()
                    .rev()
                    .find(|t: &&Tok| !t.is_trivia())
                    .is_some_and(|t| {
                        matches!(t, Tok::Ident(_) | Tok::Num(_) | Tok::Str(_))
                            || t.is_punct(")")
                            || t.is_punct("]")
                    }))
        {
            // A number, including the grammar's leading-dot float `.5` (a `.` at operand
            // position, i.e. not after an operand).
            let (n, next) = lex_number(&chars, i);
            toks.push(Tok::Num(n));
            i = next;
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            toks.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c == '.' && chars.get(i + 1) == Some(&'.') {
            toks.push(Tok::Punct("..".to_string()));
            i += 2;
        } else {
            toks.push(Tok::Punct(c.to_string()));
            i += 1;
        }
    }
    Ok(toks)
}

/// Lexes a quoted string starting at `start`; returns the unescaped content and the index after
/// the closing quote. Escapes follow the grammar: `\b \t \n \f \r \" \' \\`, octal, `\uXXXX`
/// (with repeated `u`), and a backslash-newline line join.
fn lex_string(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let quote = chars[start];
    let mut out = String::new();
    let mut i = start + 1;
    loop {
        let Some(&c) = chars.get(i) else {
            return Err("unterminated string literal".to_string());
        };
        if c == quote {
            return Ok((out, i + 1));
        }
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        let Some(&e) = chars.get(i + 1) else {
            return Err("unterminated escape in string literal".to_string());
        };
        i += 2;
        match e {
            'b' => out.push('\u{8}'),
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'f' => out.push('\u{c}'),
            'r' => {
                if chars.get(i) == Some(&'\n') {
                    // `\` + CRLF line join.
                    i += 1;
                } else {
                    out.push('\r');
                }
            }
            '\n' => {} // line join
            '"' | '\'' | '\\' => out.push(e),
            'u' => {
                while chars.get(i) == Some(&'u') {
                    i += 1;
                }
                let hex: String = chars.get(i..i + 4).unwrap_or(&[]).iter().collect();
                let code = u32::from_str_radix(&hex, 16)
                    .ok()
                    .filter(|_| hex.len() == 4)
                    .ok_or_else(|| format!("bad unicode escape: \\u{hex}"))?;
                out.push(char::from_u32(code).ok_or_else(|| format!("bad code point {code}"))?);
                i += 4;
            }
            '0'..='7' => {
                let max = if e <= '3' { 3 } else { 2 };
                let mut digits = e.to_string();
                while digits.len() < max && chars.get(i).is_some_and(|d| ('0'..='7').contains(d)) {
                    digits.push(chars[i]);
                    i += 1;
                }
                let code = u32::from_str_radix(&digits, 8).map_err(|e| e.to_string())?;
                out.push(char::from_u32(code).ok_or_else(|| format!("bad code point {code}"))?);
            }
            other => return Err(format!("unknown escape in string literal: \\{other}")),
        }
    }
}

/// Lexes a numeric literal (no sign) starting at a digit; returns the raw text and the next index.
fn lex_number(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start;
    let is_digit_or_us = |c: &char| c.is_ascii_digit() || *c == '_';
    if chars[i] == '0' && matches!(chars.get(i + 1), Some('x' | 'X')) {
        i += 2;
        while chars
            .get(i)
            .is_some_and(|c| c.is_ascii_hexdigit() || *c == '_')
        {
            i += 1;
        }
    } else {
        // A leading-dot float (`.5`) starts directly at the fraction below.
        while chars.get(i).is_some_and(is_digit_or_us) {
            i += 1;
        }
        // Fraction: `.` followed by a digit (`1..3` is a range, `x.1` does not occur).
        if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
            while chars.get(i).is_some_and(is_digit_or_us) {
                i += 1;
            }
        }
        // Exponent.
        if matches!(chars.get(i), Some('e' | 'E')) {
            let mut j = i + 1;
            if matches!(chars.get(j), Some('+' | '-')) {
                j += 1;
            }
            if chars.get(j).is_some_and(|c| c.is_ascii_digit()) {
                i = j;
                while chars.get(i).is_some_and(is_digit_or_us) {
                    i += 1;
                }
            }
        }
    }
    // Type suffix: a single letter not followed by another identifier character.
    if chars.get(i).is_some_and(|c| c.is_ascii_alphabetic())
        && !chars
            .get(i + 1)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
    {
        i += 1;
    }
    (chars[start..i].iter().collect(), i)
}

/// For every opening bracket index, the index of its closing bracket.
fn match_brackets(toks: &[Tok]) -> Result<Vec<usize>, String> {
    let mut matching = vec![usize::MAX; toks.len()];
    let mut stack: Vec<(usize, &str)> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let Tok::Punct(p) = t else { continue };
        match p.as_str() {
            "(" | "[" | "{" => stack.push((i, p.as_str())),
            ")" | "]" | "}" => {
                let want = match p.as_str() {
                    ")" => "(",
                    "]" => "[",
                    _ => "{",
                };
                match stack.pop() {
                    Some((open, kind)) if kind == want => matching[open] = i,
                    _ => return Err(format!("unbalanced `{p}`")),
                }
            }
            _ => {}
        }
    }
    if let Some((_, kind)) = stack.last() {
        return Err(format!("unclosed `{kind}`"));
    }
    Ok(matching)
}

// ---------------------------------------------------------------------------------------------
// Translation
// ---------------------------------------------------------------------------------------------

struct Translator<'a> {
    toks: &'a [Tok],
    matching: &'a [usize],
    renames: &'a HashMap<String, String>,
}

impl Translator<'_> {
    /// Index of the previous non-trivia token before `i`.
    fn prev_sig(&self, i: usize) -> Option<usize> {
        (0..i).rev().find(|&j| !self.toks[j].is_trivia())
    }

    /// Index of the next non-trivia token at or after `i`, below `hi`.
    fn next_sig(&self, i: usize, hi: usize) -> Option<usize> {
        (i..hi).find(|&j| !self.toks[j].is_trivia())
    }

    /// Non-trivia token indices of `lo..hi`.
    fn sig_indices(&self, lo: usize, hi: usize) -> Vec<usize> {
        (lo..hi).filter(|&j| !self.toks[j].is_trivia()).collect()
    }

    /// Splits `lo..hi` at top-level occurrences of `sep` (nested brackets are skipped).
    fn split_top(&self, lo: usize, hi: usize, sep: &str) -> Vec<(usize, usize)> {
        let mut parts = Vec::new();
        let mut start = lo;
        let mut i = lo;
        while i < hi {
            let t = &self.toks[i];
            if t.is_punct("(") || t.is_punct("[") || t.is_punct("{") {
                i = self.matching[i] + 1;
                continue;
            }
            if t.is_punct(sep) {
                parts.push((start, i));
                start = i + 1;
            }
            i += 1;
        }
        parts.push((start, hi));
        parts
    }

    /// First top-level `:` in `lo..hi`.
    fn top_colon(&self, lo: usize, hi: usize) -> Option<usize> {
        let mut i = lo;
        while i < hi {
            let t = &self.toks[i];
            if t.is_punct("(") || t.is_punct("[") || t.is_punct("{") {
                i = self.matching[i] + 1;
                continue;
            }
            if t.is_punct(":") {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// Emits the translation of `lo..hi`. `callee` is the name of the call whose argument list
    /// this range is, if any.
    fn emit_range(
        &self,
        lo: usize,
        hi: usize,
        callee: Option<&str>,
        out: &mut String,
    ) -> Result<(), String> {
        let mut i = lo;
        while i < hi {
            match &self.toks[i] {
                Tok::Ws(s) | Tok::Comment(s) => out.push_str(s),
                Tok::Str(s) => push_rhai_string(s, out),
                Tok::Num(raw) => out.push_str(&convert_number(raw)?),
                Tok::Punct(p) => match p.as_str() {
                    "(" => {
                        let close = self.matching[i];
                        let name = match self.prev_sig(i).map(|j| &self.toks[j]) {
                            Some(Tok::Ident(n)) => Some(n.as_str()),
                            _ => None,
                        };
                        out.push('(');
                        self.emit_range(i + 1, close, name, out)?;
                        out.push(')');
                        i = close;
                    }
                    "[" => {
                        let close = self.matching[i];
                        self.emit_collection(i + 1, close, out)?;
                        i = close;
                    }
                    // A Groovy set literal is only a side-effect seed (`withSideEffect("a", {..})`);
                    // anywhere else a set is a *value*, which Graphersal does not have.
                    "{" if callee == Some("withSideEffect") => {
                        let close = self.matching[i];
                        out.push_str("Set.of(");
                        self.emit_range(i + 1, close, None, out)?;
                        out.push(')');
                        i = close;
                    }
                    "{" => return Err("unsupported literal type: set".to_string()),
                    _ => out.push_str(p),
                },
                Tok::Ident(name) => {
                    i = self.emit_ident(i, hi, name, callee, out)?;
                }
            }
            i += 1;
        }
        Ok(())
    }

    /// Emits identifier `name` at index `i`; returns the index of the last token consumed.
    fn emit_ident(
        &self,
        i: usize,
        hi: usize,
        name: &str,
        callee: Option<&str>,
        out: &mut String,
    ) -> Result<usize, String> {
        let after_dot = self.prev_sig(i).is_some_and(|j| self.toks[j].is_punct("."));
        let next = self.next_sig(i + 1, hi);
        let paren = next.filter(|&j| self.toks[j].is_punct("("));

        if after_dot {
            if paren.is_some()
                && let Some(reason) = rhai_step_clash(name)
            {
                return Err(reason);
            }
            out.push_str(name);
            return Ok(i);
        }

        // Parameters first: a parameter is neither a step nor a token.
        if let Some(rhai) = self.renames.get(name) {
            out.push_str(rhai);
            return Ok(i);
        }

        if name == "new" && next.is_some_and(|j| matches!(self.toks[j], Tok::Ident(_))) {
            // Drop `new` and the whitespace after it.
            return Ok(next.map_or(i, |j| j - 1));
        }

        match name {
            "null" => {
                out.push_str("()");
                return Ok(i);
            }
            "datetime" | "DateTime" if paren.is_some() => {
                // The DSL's datetime constructors (`datetime("..")`, `DateTime("..")`), not steps.
                out.push_str(name);
                return Ok(i);
            }
            "UUID" => {
                if let Some(open) = paren
                    && !self.sig_indices(open + 1, self.matching[open]).is_empty()
                {
                    out.push_str("UUID.fromString");
                    return Ok(i);
                }
                out.push_str(name);
                return Ok(i);
            }
            _ => {}
        }

        if let Some(open) = paren {
            let close = self.matching[open];
            if matches!(name, "V" | "E") {
                // A bare `V(..)`/`E(..)` is the anonymous step, like every other bare step
                // (`from(V(vid1))`), not a class constructor.
                out.push_str("__.");
                out.push_str(name);
                return Ok(i);
            }
            if name.starts_with(|c: char| c.is_uppercase()) {
                // A class (strategy) constructor. Named arguments → one map argument.
                let sig = self.sig_indices(open + 1, close);
                let named = sig.len() >= 2
                    && matches!(self.toks[sig[0]], Tok::Ident(_))
                    && self.toks[sig[1]].is_punct(":");
                if named {
                    out.push_str(name);
                    out.push_str("(#{");
                    self.emit_entries(open + 1, close, out)?;
                    out.push_str("})");
                    return Ok(close);
                }
                out.push_str(name);
                return Ok(i);
            }
            if let Some(reason) = rhai_step_clash(name) {
                return Err(reason);
            }
            let prefix = if P_PREDICATES.contains(&name) {
                "P."
            } else if TEXTP_PREDICATES.contains(&name) {
                "TextP."
            } else if matches!(name, "single" | "set" | "list") {
                "Cardinality."
            } else if name == "not" && self.first_arg_is_predicate(open + 1, close) {
                "P."
            } else {
                "__."
            };
            out.push_str(prefix);
            out.push_str(name);
            return Ok(i);
        }

        let class = match name {
            "set" | "list" if callee == Some("typeOf") => Some("GType"),
            // `UUID` is handled above; `null` too.
            _ => static_import_class(name),
        };
        if let Some(class) = class {
            out.push_str(class);
            out.push('.');
        }
        out.push_str(name);
        Ok(i)
    }

    /// Whether the first argument in `lo..hi` is a predicate (`P.`, `TextP.` or a bare predicate
    /// call).
    fn first_arg_is_predicate(&self, lo: usize, hi: usize) -> bool {
        let Some(j) = self.next_sig(lo, hi) else {
            return false;
        };
        match &self.toks[j] {
            Tok::Ident(n) if n == "P" || n == "TextP" => true,
            Tok::Ident(n) => {
                (P_PREDICATES.contains(&n.as_str())
                    || TEXTP_PREDICATES.contains(&n.as_str())
                    || n == "not")
                    && self
                        .next_sig(j + 1, hi)
                        .is_some_and(|k| self.toks[k].is_punct("("))
            }
            _ => false,
        }
    }

    /// Emits `[..]` whose content is `lo..hi`: a list or a map.
    fn emit_collection(&self, lo: usize, hi: usize, out: &mut String) -> Result<(), String> {
        let sig = self.sig_indices(lo, hi);
        if sig.len() == 1 && self.toks[sig[0]].is_punct(":") {
            out.push_str("#{}");
            return Ok(());
        }
        if self.top_colon(lo, hi).is_some() {
            out.push_str("#{");
            self.emit_entries(lo, hi, out)?;
            out.push('}');
        } else {
            out.push('[');
            self.emit_range(lo, hi, None, out)?;
            out.push(']');
        }
        Ok(())
    }

    /// Emits `key: value, ...` entries of a map literal or strategy configuration as Rhai
    /// object-map entries with string keys.
    fn emit_entries(&self, lo: usize, hi: usize, out: &mut String) -> Result<(), String> {
        let parts = self.split_top(lo, hi, ",");
        for (n, &(start, end)) in parts.iter().enumerate() {
            if n > 0 {
                out.push(',');
            }
            let colon = self
                .top_colon(start, end)
                .ok_or_else(|| "map literal entry without `:`".to_string())?;
            // Keep the whitespace in front of the key.
            let mut k = start;
            while k < colon && self.toks[k].is_trivia() {
                if let Tok::Ws(s) | Tok::Comment(s) = &self.toks[k] {
                    out.push_str(s);
                }
                k += 1;
            }
            let key = self.map_key(start, colon)?;
            push_rhai_string(&key, out);
            out.push(':');
            if self.sig_indices(colon + 1, end).is_empty() {
                return Err("map literal entry without a value".to_string());
            }
            self.emit_range(colon + 1, end, None, out)?;
        }
        Ok(())
    }

    /// The string key of a map entry whose key tokens are `lo..hi`.
    ///
    /// Grammar `mapKey`: `(traversalT)` | `traversalTLong` | `(traversalDirection)` |
    /// `traversalDirectionLong` | `(genericLiteral)` | `keyword | nakedKey`. The token forms
    /// become the DSL's reserved string keys (`T.label`, `(T.id)`, `(label)` → `"T.label"`/
    /// `"T.id"`; `Direction.OUT`, `(OUT)`, `(from)`, `Direction.to` → `"Direction.OUT"`/
    /// `"Direction.IN"`); a bare `id:`/`label:`/`OUT:` is a naked key, i.e. the string itself.
    fn map_key(&self, lo: usize, hi: usize) -> Result<String, String> {
        let sig = self.sig_indices(lo, hi);
        let toks: Vec<&Tok> = sig.iter().map(|&j| &self.toks[j]).collect();
        let parenthesized =
            toks.len() >= 3 && toks[0].is_punct("(") && toks[toks.len() - 1].is_punct(")");
        let inner = if parenthesized {
            &toks[1..toks.len() - 1]
        } else {
            &toks[..]
        };
        // A qualified token (`T.id`, `Direction.OUT`), plain or in parentheses.
        if let [Tok::Ident(class), dot, Tok::Ident(name)] = inner
            && dot.is_punct(".")
            && let Some(key) = token_map_key(class, name)
        {
            return Ok(key);
        }
        match toks.as_slice() {
            [Tok::Str(s)] | [Tok::Ident(s)] => Ok(s.clone()),
            [open, Tok::Str(s), close] if open.is_punct("(") && close.is_punct(")") => {
                Ok(s.clone())
            }
            // A short token in parentheses: `(id)`, `(label)`, `(OUT)`, `(from)`, ...
            [open, Tok::Ident(name), close] if open.is_punct("(") && close.is_punct(")") => {
                let class = match name.as_str() {
                    "id" | "label" | "key" | "value" => "T",
                    _ => "Direction",
                };
                token_map_key(class, name)
                    .ok_or_else(|| "map literal with non-string key".to_string())
            }
            _ => Err("map literal with non-string key".to_string()),
        }
    }
}

/// The DSL's reserved string key of a `T` / `Direction` token used as a map key: `T.id`,
/// `T.label` (and `T.key`/`T.value`, which the engine rejects); `Direction.OUT`/`IN`/`BOTH`
/// with the aliases `from` (= `OUT`) and `to` (= `IN`).
fn token_map_key(class: &str, name: &str) -> Option<String> {
    match (class, name) {
        ("T", "id" | "label" | "key" | "value") => Some(format!("T.{name}")),
        ("Direction", "OUT" | "from") => Some("Direction.OUT".to_string()),
        ("Direction", "IN" | "to") => Some("Direction.IN".to_string()),
        ("Direction", "BOTH") => Some("Direction.BOTH".to_string()),
        _ => None,
    }
}

/// Appends `s` as a double-quoted Rhai string literal.
fn push_rhai_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Converts a raw Gremlin numeric literal to Rhai.
fn convert_number(raw: &str) -> Result<String, String> {
    let hex = raw.starts_with("0x") || raw.starts_with("0X");
    let last = raw.chars().last().unwrap_or('0');
    let suffix = if last.is_ascii_alphabetic() && !(hex && last.is_ascii_hexdigit()) {
        Some(last)
    } else {
        None
    };
    let body = match suffix {
        Some(_) => &raw[..raw.len() - 1],
        None => raw,
    };
    // Test-surface deviation (tests/tinkerpop/README.md): Graphersal has int64 and float64 only,
    // so `1b`/`1s`/`1n` map to an integer and `1m` to a float; the width cannot be told apart.
    let is_decimal = matches!(suffix, Some('m' | 'M'));
    let digits: String = body.chars().filter(|&c| c != '_').collect();
    let is_float = !hex
        && (digits.contains('.')
            || digits.contains(['e', 'E'])
            || matches!(suffix, Some('d' | 'D' | 'f' | 'F' | 'm' | 'M')));
    if is_float {
        let lower = digits.to_ascii_lowercase();
        let (mantissa, exponent) = match lower.find('e') {
            Some(p) => lower.split_at(p),
            None => (lower.as_str(), ""),
        };
        let mantissa = if mantissa.starts_with('.') {
            format!("0{mantissa}")
        } else if mantissa.contains('.') {
            mantissa.to_string()
        } else {
            format!("{mantissa}.0")
        };
        let text = format!("{mantissa}{exponent}");
        return match text.parse::<f64>() {
            Ok(v) if v.is_finite() => Ok(text),
            _ if is_decimal => Err("unsupported literal type: bigdecimal".to_string()),
            _ => Err(format!("float literal out of float64 range: {raw}")),
        };
    }
    if !matches!(
        suffix,
        None | Some('i' | 'I' | 'l' | 'L' | 'b' | 'B' | 's' | 'S' | 'n' | 'N')
    ) {
        return Err(format!("unknown numeric suffix: {raw}"));
    }
    let value = if hex {
        i64::from_str_radix(&digits[2..], 16)
    } else if digits.len() > 1 && digits.starts_with('0') {
        i64::from_str_radix(&digits[1..], 8)
    } else {
        digits.parse::<i64>()
    };
    match value {
        Ok(v) => Ok(v.to_string()),
        Err(_) if matches!(suffix, None | Some('n' | 'N')) => {
            Err("unsupported literal type: biginteger".to_string())
        }
        Err(_) => Err(format!("integer literal out of int64 range: {raw}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(gremlin: &str) -> String {
        ok_with(gremlin, &[], &[]).0
    }

    fn ok_with(
        gremlin: &str,
        params: &[&str],
        side_effects: &[&str],
    ) -> (String, Vec<(String, String)>) {
        let params: Vec<String> = params.iter().map(|s| s.to_string()).collect();
        let side_effects: Vec<String> = side_effects.iter().map(|s| s.to_string()).collect();
        match translate(gremlin, &params, &side_effects) {
            Translation::Ok {
                script,
                param_names,
            } => (script, param_names),
            Translation::Unsupported { reason } => panic!("`{gremlin}` unsupported: {reason}"),
        }
    }

    fn unsupported(gremlin: &str) -> String {
        match translate(gremlin, &[], &[]) {
            Translation::Unsupported { reason } => reason,
            Translation::Ok { script, .. } => panic!("`{gremlin}` translated to `{script}`"),
        }
    }

    #[test]
    fn qualified_anonymous_traversals_pass_through() {
        assert_eq!(
            ok(r#"g.V().where(__.out("created")).values("name")"#),
            r#"g.V().where(__.out("created")).values("name")"#
        );
    }

    #[test]
    fn bare_steps_get_anonymous_prefix() {
        assert_eq!(
            ok("g.V().repeat(out()).times(2)"),
            "g.V().repeat(__.out()).times(2)"
        );
        assert_eq!(
            ok("g.V().where(inject('y'))"),
            r#"g.V().where(__.inject("y"))"#
        );
        // `in`/`as` are Rhai keywords, but after `__.` the DSL accepts them.
        assert_eq!(ok("g.V().local(in())"), "g.V().local(__.in())");
        // A bare `V(..)`/`E(..)` is the anonymous start step, not a class constructor.
        assert_eq!(
            ok("g.addE('knows').from(V(vid1)).to(V().has('name', 'vadas'))"),
            r#"g.addE("knows").from(__.V(vid1)).to(__.V().has("name", "vadas"))"#
        );
        assert_eq!(
            ok("g.inject(1).coalesce(E().hasLabel('x'))"),
            r#"g.inject(1).coalesce(__.E().hasLabel("x"))"#
        );
    }

    #[test]
    fn nesting_bare_step_in_bare_step_in_map_literal() {
        assert_eq!(
            ok("g.inject([a: union(out(), both())])"),
            r#"g.inject(#{"a": __.union(__.out(), __.both())})"#
        );
    }

    #[test]
    fn bare_predicates_and_not() {
        assert_eq!(
            ok("g.V().values('age').is(gt(30))"),
            r#"g.V().values("age").is(P.gt(30))"#
        );
        assert_eq!(
            ok("g.V().has('name', containing('ark'))"),
            r#"g.V().has("name", TextP.containing("ark"))"#
        );
        assert_eq!(
            ok("g.V().has('age', not(within(27, 29)))"),
            r#"g.V().has("age", P.not(P.within(27, 29)))"#
        );
        assert_eq!(
            ok("g.V().not(out('created'))"),
            r#"g.V().not(__.out("created"))"#
        );
        assert_eq!(ok("g.V().is(typeOf(set))"), "g.V().is(P.typeOf(GType.set))");
    }

    #[test]
    fn qualified_tokens_pass_through() {
        let line = r#"g.V().hasLabel("person").local(__.values("age").sum(Scope.local)).order().by(Order.desc).select(Pop.all, "a")"#;
        assert_eq!(ok(line), line);
        let merge = "g.mergeV(xx1).option(Merge.onCreate, xx2).option(Merge.onMatch, xx3)";
        assert_eq!(ok(merge), merge);
    }

    #[test]
    fn bare_tokens_get_their_class() {
        assert_eq!(
            ok("g.V().order().by('age', desc)"),
            r#"g.V().order().by("age", Order.desc)"#
        );
        assert_eq!(
            ok("g.V().fold().count(local)"),
            "g.V().fold().count(Scope.local)"
        );
        assert_eq!(
            ok("g.V().as('a').select(first, 'a')"),
            r#"g.V().as("a").select(Pop.first, "a")"#
        );
        for (bare, class) in [
            ("last", "Pop.last"),
            ("all", "Pop.all"),
            ("mixed", "Pop.mixed"),
        ] {
            assert_eq!(
                ok(&format!("g.V().as('a').select({bare}, 'a')")),
                format!(r#"g.V().as("a").select({class}, "a")"#)
            );
        }
        // the mixed spelling of `Paths.feature`: bare and qualified side by side
        assert_eq!(
            ok(
                r#"g.V().as("v").both().as("v").project("a","b","c").by(__.select(first, "v")).by(__.select(last, "v")).by(__.select(Pop.all, "v"))"#
            ),
            r#"g.V().as("v").both().as("v").project("a","b","c").by(__.select(Pop.first, "v")).by(__.select(Pop.last, "v")).by(__.select(Pop.all, "v"))"#
        );
        // a name followed by `(` is a step call, never a token
        assert_eq!(ok("g.V().all(gt(1)).last()"), "g.V().all(P.gt(1)).last()");
        assert!(DSL_GLOBALS.contains(&"Pop"));
        assert_eq!(
            ok("g.withSack(1).V().sack(sum).by(constant(1))"),
            "g.withSack(1).V().sack(Operator.sum).by(__.constant(1))"
        );
        assert_eq!(
            ok("g.V().group().by(label).by(values)"),
            "g.V().group().by(T.label).by(Column.values)"
        );
        assert_eq!(
            ok("g.V().toE(OUT, 'x')"),
            r#"g.V().toE(Direction.OUT, "x")"#
        );
        assert_eq!(
            ok("g.V().property(list, 'x', 1)"),
            r#"g.V().property(Cardinality.list, "x", 1)"#
        );
        assert_eq!(
            ok("g.V().property(single('x'), 1)"),
            r#"g.V().property(Cardinality.single("x"), 1)"#
        );
        assert_eq!(
            ok("g.withSack(1.0).V().barrier(normSack)"),
            "g.withSack(1.0).V().barrier(Barrier.normSack)"
        );
    }

    #[test]
    fn numeric_suffixes() {
        assert_eq!(ok("g.V().has('age', 29L)"), r#"g.V().has("age", 29)"#);
        assert_eq!(
            ok("g.inject(1l, 1i, 2.0d, 1d, 1f, 1D)"),
            "g.inject(1, 1, 2.0, 1.0, 1.0, 1.0)"
        );
        assert_eq!(
            ok("g.inject(1.7976931348623157E+308d)"),
            "g.inject(1.7976931348623157e+308)"
        );
        assert_eq!(ok("g.inject(-1, 1_000)"), "g.inject(-1, 1000)");
        assert_eq!(ok("g.inject(0x1F, 017)"), "g.inject(31, 15)");
        assert_eq!(ok("g.inject(.5, .5f, -.5d)"), "g.inject(0.5, 0.5, -0.5)");
        assert_eq!(ok("g.V().values('a').is(1)"), r#"g.V().values("a").is(1)"#);
        // Width literals collapse to int64/float64 (test-surface deviation).
        assert_eq!(ok("g.inject(1b, 1s, 1n, -3b)"), "g.inject(1, 1, 1, -3)");
        assert_eq!(ok("g.inject(1m, 1.50m)"), "g.inject(1.0, 1.50)");
        assert_eq!(
            unsupported("g.inject(99999999999999999999n)"),
            "unsupported literal type: biginteger"
        );
        assert_eq!(
            unsupported("g.inject(1e999m)"),
            "unsupported literal type: bigdecimal"
        );
        assert_eq!(
            unsupported("g.inject(99999999999999999999)"),
            "unsupported literal type: biginteger"
        );
    }

    #[test]
    fn strings() {
        assert_eq!(
            ok(r#"g.V().has('name', 'marko')"#),
            r#"g.V().has("name", "marko")"#
        );
        assert_eq!(ok(r#"g.inject('it\'s "x"')"#), r#"g.inject("it's \"x\"")"#);
        assert_eq!(ok(r#"g.inject("a\\b\n")"#), r#"g.inject("a\\b\n")"#);
    }

    #[test]
    fn string_contents_are_never_rewritten() {
        let line = r#"g.inject("[a:1] desc 1L out() null {x} new")"#;
        assert_eq!(ok(line), line);
        assert_eq!(ok("g.inject('desc')"), r#"g.inject("desc")"#);
    }

    #[test]
    fn collections() {
        assert_eq!(ok("g.inject([1, 2])"), "g.inject([1, 2])");
        assert_eq!(ok("g.inject([a: 1])"), r#"g.inject(#{"a": 1})"#);
        assert_eq!(
            ok(r#"g.mergeV(["name": "marko", age: 29])"#),
            r#"g.mergeV(#{"name": "marko", "age": 29})"#
        );
        assert_eq!(ok("g.inject([:])"), "g.inject(#{})");
        assert_eq!(
            ok("g.inject([[1], [x: [:]]])"),
            r#"g.inject([[1], #{"x": #{}}])"#
        );
        assert_eq!(
            ok(r#"g.mergeV([(T.label): "person"])"#),
            r#"g.mergeV(#{"T.label": "person"})"#
        );
        assert_eq!(
            ok(r#"g.mergeV([T.label: "person", T.id: 1])"#),
            r#"g.mergeV(#{"T.label": "person", "T.id": 1})"#
        );
        assert_eq!(
            ok(r#"g.mergeV([(label): "person", (id): 1, (T.id): 2])"#),
            r#"g.mergeV(#{"T.label": "person", "T.id": 1, "T.id": 2})"#
        );
        assert_eq!(
            ok(r#"g.inject([T.key: 1, (T.value): 2])"#),
            r#"g.inject(#{"T.key": 1, "T.value": 2})"#
        );
        assert_eq!(
            ok(r#"g.mergeE([Direction.OUT: 1, Direction.IN: 2, Direction.BOTH: 3])"#),
            r#"g.mergeE(#{"Direction.OUT": 1, "Direction.IN": 2, "Direction.BOTH": 3})"#
        );
        assert_eq!(
            ok(r#"g.mergeE([(OUT): 1, (IN): 2, (BOTH): 3, (from): 4, (to): 5])"#),
            r#"g.mergeE(#{"Direction.OUT": 1, "Direction.IN": 2, "Direction.BOTH": 3, "Direction.OUT": 4, "Direction.IN": 5})"#
        );
        assert_eq!(
            ok(r#"g.mergeE([Direction.from: 1, (Direction.to): 2])"#),
            r#"g.mergeE(#{"Direction.OUT": 1, "Direction.IN": 2})"#
        );
        // Bare `id:` / `label:` / `OUT:` are naked string keys (grammar `keyword | nakedKey`).
        assert_eq!(
            ok(r#"g.inject([id: 1, label: 2, OUT: 3])"#),
            r#"g.inject(#{"id": 1, "label": 2, "OUT": 3})"#
        );
        assert_eq!(
            unsupported(r#"g.inject([(Merge.outV): 1])"#),
            "map literal with non-string key"
        );
        assert_eq!(
            unsupported("g.inject([1: 2])"),
            "map literal with non-string key"
        );
        assert_eq!(
            unsupported(r#"g.inject({"x","y"})"#),
            "unsupported literal type: set"
        );
    }

    #[test]
    fn set_literal_only_as_a_side_effect_seed() {
        assert_eq!(
            ok(r#"g.withSideEffect("a", {"alice"}).V().cap("a")"#),
            r#"g.withSideEffect("a", Set.of("alice")).V().cap("a")"#
        );
        assert_eq!(
            ok(r#"g.withSideEffect("x",{}).V()"#),
            r#"g.withSideEffect("x",Set.of()).V()"#
        );
        assert_eq!(
            ok(r#"g.withSideEffect("x", {1, 2}).V()"#),
            r#"g.withSideEffect("x", Set.of(1, 2)).V()"#
        );
    }

    #[test]
    fn literals() {
        assert_eq!(
            ok("g.inject(null, NaN, -Infinity)"),
            "g.inject((), NaN, -Infinity)"
        );
        assert_eq!(
            ok(r#"g.inject(datetime("2023-08-08T00:00:00Z"))"#),
            r#"g.inject(datetime("2023-08-08T00:00:00Z"))"#
        );
        assert_eq!(
            ok("g.inject(DateTime('2023-08-08T00:00:00Z'))"),
            r#"g.inject(DateTime("2023-08-08T00:00:00Z"))"#
        );
        assert_eq!(
            ok("g.V().where(__.is(P.gt(datetime('2020-01-01T00:00:00Z')))).dateAdd(hour, 2)"),
            r#"g.V().where(__.is(P.gt(datetime("2020-01-01T00:00:00Z")))).dateAdd(DT.hour, 2)"#
        );
        assert_eq!(
            ok(r#"g.inject(UUID("f47af10b-58cc-4372-a567-0f02b2f3d479"))"#),
            r#"g.inject(UUID.fromString("f47af10b-58cc-4372-a567-0f02b2f3d479"))"#
        );
    }

    #[test]
    fn strategies() {
        assert_eq!(
            ok(
                r#"g.withStrategies(new SubgraphStrategy(vertices: __.has("name", P.within("josh", "lop")))).V()"#
            ),
            r#"g.withStrategies(SubgraphStrategy(#{"vertices": __.has("name", P.within("josh", "lop"))})).V()"#
        );
        assert_eq!(
            ok(
                r#"g.withStrategies(new PartitionStrategy(partitionKey: "_partition", writePartition: "a", readPartitions: ["a"])).V()"#
            ),
            r#"g.withStrategies(PartitionStrategy(#{"partitionKey": "_partition", "writePartition": "a", "readPartitions": ["a"]})).V()"#
        );
        assert_eq!(
            ok("g.withoutStrategies(CountStrategy).V().count()"),
            "g.withoutStrategies(CountStrategy).V().count()"
        );
        assert_eq!(
            unsupported(
                r#"g.withStrategies(ReservedKeysVerificationStrategy(throwException: true, keys: {"age"})).V()"#
            ),
            "unsupported literal type: set"
        );
    }

    #[test]
    fn multi_line_is_kept() {
        let text = "g.V().\n  out('knows').\n  values('name')";
        assert_eq!(ok(text), "g.V().\n  out(\"knows\").\n  values(\"name\")");
    }

    #[test]
    fn match_step_is_unsupported() {
        assert_eq!(
            unsupported("g.V().match(__.as('a').out().as('b'))"),
            "step name is a reserved Rhai word: match"
        );
    }

    #[test]
    fn call_step_is_unsupported() {
        assert_eq!(
            unsupported("g.V().call('tinkerpop.dc')"),
            "step name is a Rhai built-in function: call"
        );
        assert_eq!(
            unsupported("g.V().where(call('tinkerpop.dc'))"),
            "step name is a Rhai built-in function: call"
        );
    }

    #[test]
    fn parameter_rename() {
        let (script, names) = ok_with(
            "g.V(vid1).has('x', V).values(xx1).is(V)",
            &["vid1", "V", "xx1"],
            &[],
        );
        assert_eq!(script, r#"g.V(vid1).has("x", p_V).values(xx1).is(p_V)"#);
        assert_eq!(
            names,
            vec![
                ("vid1".to_string(), "vid1".to_string()),
                ("V".to_string(), "p_V".to_string()),
                ("xx1".to_string(), "xx1".to_string()),
            ]
        );
        // Map keys, method names and string contents are not renamed.
        let (script, _) = ok_with("g.inject([in: in]).in('in')", &["in"], &[]);
        assert_eq!(script, r#"g.inject(#{"in": p_in}).in("in")"#);
        // A parameter named like a token is neither a token nor a step.
        let (script, _) = ok_with("g.V().order().by(desc)", &["desc"], &[]);
        assert_eq!(script, "g.V().order().by(desc)");
    }

    #[test]
    fn side_effect_prefix() {
        let (script, _) = ok_with("g.V().out().aggregate('a')", &[], &["a", "b"]);
        assert_eq!(
            script,
            r#"g.withSideEffect("a", se_a).withSideEffect("b", se_b).V().out().aggregate("a")"#
        );
        // Real suite line (side effect `a`, sideEffect/Aggregate.feature).
        let (script, _) = ok_with(
            r#"g.V().both().values("name").local(aggregate("a")).cap("a")"#,
            &[],
            &["a"],
        );
        assert_eq!(
            script,
            r#"g.withSideEffect("a", se_a).V().both().values("name").local(__.aggregate("a")).cap("a")"#
        );
    }

    /// One real line of the vendored suite per rule.
    #[test]
    fn real_suite_lines() {
        let cases: &[(&str, &str)] = &[
            // Bare step at argument position, single-quoted string.
            (
                "g.V().repeat(inject('y')).times(2)",
                r#"g.V().repeat(__.inject("y")).times(2)"#,
            ),
            (
                "g.V().union(__.outE().count(), __.local(inE().count()))",
                "g.V().union(__.outE().count(), __.local(__.inE().count()))",
            ),
            // Bare predicates inside bare steps.
            (
                r#"g.withStrategies(RepeatUnrollStrategy).V().repeat(both().has("age", lt(30))).times(2)"#,
                r#"g.withStrategies(RepeatUnrollStrategy).V().repeat(__.both().has("age", P.lt(30))).times(2)"#,
            ),
            (
                r#"g.withStrategies(RepeatUnrollStrategy).V().repeat(out().has("name", notStartingWith("z"))).times(2)"#,
                r#"g.withStrategies(RepeatUnrollStrategy).V().repeat(__.out().has("name", TextP.notStartingWith("z"))).times(2)"#,
            ),
            (
                r#"g.V().outE("knows").subgraph("sg").cap("sg").is(not(P.typeOf(GType.GRAPH))).count()"#,
                r#"g.V().outE("knows").subgraph("sg").cap("sg").is(P.not(P.typeOf(GType.GRAPH))).count()"#,
            ),
            // Bare tokens.
            (
                r#"g.V().values("list").is(P.typeOf(GType.LIST)).count(local)"#,
                r#"g.V().values("list").is(P.typeOf(GType.LIST)).count(Scope.local)"#,
            ),
            (
                r#"g.withSack(0L).V().in().barrier().local(__.sack(sum).by("age")).sack()"#,
                r#"g.withSack(0).V().in().barrier().local(__.sack(Operator.sum).by("age")).sack()"#,
            ),
            (
                "g.V().groupCount().by(label).is(P.typeOf(GType.MAP))",
                "g.V().groupCount().by(T.label).is(P.typeOf(GType.MAP))",
            ),
            (
                "g.mergeE(xx1).option(outV, xx2).option(inV, xx3)",
                "g.mergeE(xx1).option(Merge.outV, xx2).option(Merge.inV, xx3)",
            ),
            (
                r#"g.addV("person").property(list, "name", "stephen")"#,
                r#"g.addV("person").property(Cardinality.list, "name", "stephen")"#,
            ),
            (
                r#"g.V().as("v").valueMap().with("direction", OUT)"#,
                r#"g.V().as("v").valueMap().with("direction", Direction.OUT)"#,
            ),
            // Numbers.
            (
                "g.withSack(2147483647i).inject(0.5f).sack(div).sack()",
                "g.withSack(2147483647).inject(0.5).sack(Operator.div).sack()",
            ),
            ("g.inject(.5)", "g.inject(0.5)"),
            // Collections, null, UUID, NaN/Infinity.
            (
                r#"g.inject(["a":1],["b":2]).fold([:], Operator.addAll)"#,
                r#"g.inject(#{"a":1},#{"b":2}).fold(#{}, Operator.addAll)"#,
            ),
            (
                r#"g.V().fold().merge([k:"v"])"#,
                r#"g.V().fold().merge(#{"k":"v"})"#,
            ),
            (
                r#"g.addV("foo").property(set, null)"#,
                r#"g.addV("foo").property(Cardinality.set, ())"#,
            ),
            (
                r#"g.addV("data").property("uuid", UUID("f47af10b-58cc-4372-a567-0f02b2f3d479"))"#,
                r#"g.addV("data").property("uuid", UUID.fromString("f47af10b-58cc-4372-a567-0f02b2f3d479"))"#,
            ),
            (
                "g.inject(Infinity).is(P.eq(+Infinity))",
                "g.inject(Infinity).is(P.eq(+Infinity))",
            ),
            // Strategy.
            (
                r#"g.withStrategies(new SubgraphStrategy(vertices: __.has("name", P.within("josh", "lop", "ripple")))).V()"#,
                r#"g.withStrategies(SubgraphStrategy(#{"vertices": __.has("name", P.within("josh", "lop", "ripple"))})).V()"#,
            ),
            (
                r#"g.withStrategies(ReservedKeysVerificationStrategy(throwException: true)).addV("person")"#,
                r#"g.withStrategies(ReservedKeysVerificationStrategy(#{"throwException": true})).addV("person")"#,
            ),
        ];
        for (gremlin, rhai) in cases {
            assert_eq!(&ok(gremlin), rhai, "translating `{gremlin}`");
        }

        let unsupported_cases: &[(&str, &str)] = &[
            (
                r#"g.withStrategies(ReservedKeysVerificationStrategy(throwException: true, keys: {"age"})).addV("person")"#,
                "unsupported literal type: set",
            ),
            (
                r#"g.V().match(__.as("a").out().as("b"))"#,
                "step name is a reserved Rhai word: match",
            ),
        ];
        for (gremlin, reason) in unsupported_cases {
            assert_eq!(&unsupported(gremlin), reason, "translating `{gremlin}`");
        }
    }
}
