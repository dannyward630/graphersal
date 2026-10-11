//! Glob patterns for the `glob_path()` step: the grammar, the compiler, the per-segment matcher and
//! the automaton tables the step runs.
//!
//! A pattern such as `**/author/*/some/*.rs` is a `/`-separated list of segments. Each segment
//! matches one level of the walk below the start vertex, against the match key of a visited vertex.
//! The compiled [`GlobPattern`] is a nondeterministic automaton whose live states fit a `u64` mask:
//! states `0..n` are "about to match segment `i`", state `n` is "accept".
//!
//! This module is graph-independent: it only compiles patterns and matches `&str` values. Matching
//! never allocates.
//!
//! Grammar:
//!
//! ```text
//! pattern      := segment ( "/" segment )*
//! segment      := "**" | part+
//! part         := literal-char | "\" any-char | "*" | "?" | class | alternatives
//! class        := "[" "!"? class-item+ "]"          ; "]" first in the set is literal
//! class-item   := char | char "-" char
//! alternatives := "{" alt ( "," alt )* "}"          ; alt may be empty; no nesting; no "/"
//! ```
//!
//! A stray `]`, `}` or `,` outside a class or alternatives is an ordinary character.

use crate::CompactString;
use crate::error::ValueError;

/// The maximum number of segments of a pattern after collapsing `**/**`: the `u64` state mask
/// needs one bit per segment plus one for the accept state.
pub(crate) const MAX_SEGMENTS: usize = 63;

/// The maximum number of alternatives one segment may expand to (the product of all its `{..}`
/// groups).
pub(crate) const MAX_ALTERNATIVES: usize = 256;

/// The `reason` texts of [`ValueError::InvalidGlobPattern`]; `help()` keys its advice on them.
pub(crate) mod reason {
    /// The pattern is the empty string.
    pub(crate) const EMPTY_PATTERN: &str = "the pattern is empty";
    /// The pattern starts with `/`.
    pub(crate) const LEADING_SLASH: &str = "a pattern cannot start with '/'";
    /// The pattern ends with `/`.
    pub(crate) const TRAILING_SLASH: &str = "a pattern cannot end with '/'";
    /// Two consecutive `/`.
    pub(crate) const EMPTY_SEGMENT: &str = "empty segment between two '/'";
    /// A whole segment is `.` or `..`.
    pub(crate) const RESERVED_SEGMENT: &str = "'.' and '..' are reserved as whole segments";
    /// `**` next to other characters in one segment.
    pub(crate) const DOUBLE_STAR_IN_SEGMENT: &str = "'**' must be a whole segment";
    /// A lone `\` at the end of the pattern.
    pub(crate) const TRAILING_ESCAPE: &str = "a trailing '\\' escapes nothing";
    /// A `[` without its `]` in the same segment.
    pub(crate) const UNCLOSED_CLASS: &str = "unclosed character class '['";
    /// `[]` or `[!]`.
    pub(crate) const EMPTY_CLASS: &str = "empty character class";
    /// A range whose start is greater than its end, e.g. `[z-a]`.
    pub(crate) const INVERTED_RANGE: &str = "inverted character range";
    /// A `{` without its `}`.
    pub(crate) const UNCLOSED_ALTERNATIVES: &str = "unclosed alternatives '{'";
    /// A `{` inside `{..}`.
    pub(crate) const NESTED_ALTERNATIVES: &str = "alternatives '{..}' cannot be nested";
    /// A `/` inside `{..}`.
    pub(crate) const SLASH_IN_ALTERNATIVES: &str = "alternatives '{..}' cannot contain '/'";
    /// A segment expands to more than [`MAX_ALTERNATIVES`](super::MAX_ALTERNATIVES) alternatives.
    pub(crate) const TOO_MANY_ALTERNATIVES: &str =
        "a segment expands to more than 256 alternatives";
    /// More than [`MAX_SEGMENTS`](super::MAX_SEGMENTS) segments.
    pub(crate) const TOO_MANY_SEGMENTS: &str = "the pattern has more than 63 segments";
}

/// One item of a character class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClassItem {
    /// One character.
    Single(char),
    /// An inclusive range of characters.
    Range(char, char),
}

/// One token of a [`Segment::Glob`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// Exactly this character.
    Char(char),
    /// `?`: exactly one character (a Unicode scalar value, not a byte).
    AnyChar,
    /// `*`: any sequence of characters, including the empty one.
    Star,
    /// `[..]` / `[!..]`: one character in (or, negated, not in) the item list.
    Class {
        /// `[!..]`.
        negated: bool,
        /// The listed characters and ranges.
        items: Vec<ClassItem>,
    },
}

impl Token {
    fn matches_char(&self, c: char) -> bool {
        match self {
            Token::Char(expected) => *expected == c,
            Token::AnyChar | Token::Star => true,
            Token::Class { negated, items } => {
                let hit = items.iter().any(|item| match item {
                    ClassItem::Single(single) => *single == c,
                    ClassItem::Range(lo, hi) => *lo <= c && c <= *hi,
                });
                hit != *negated
            }
        }
    }
}

/// One compiled alternative of a [`Segment::Alternatives`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AltMatcher {
    /// A plain string, compared with `==`.
    Literal(CompactString),
    /// A token sequence.
    Glob(Vec<Token>),
}

impl AltMatcher {
    fn matches(&self, value: &str) -> bool {
        match self {
            AltMatcher::Literal(literal) => literal.as_str() == value,
            AltMatcher::Glob(tokens) => glob_match(tokens, value),
        }
    }
}

/// One compiled segment of a [`GlobPattern`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// No metacharacters left after unescaping; matched by string comparison.
    Literal(CompactString),
    /// A whole-segment `*`: matches every child without reading its value.
    Any,
    /// A segment with wildcards or classes.
    Glob(Vec<Token>),
    /// A segment with `{..}` groups, expanded to its alternatives.
    Alternatives(Vec<AltMatcher>),
    /// A whole-segment `**`: any number of levels.
    DoubleStar {
        /// True only for the last segment: it then consumes at least one level and accepts.
        trailing: bool,
    },
}

impl Segment {
    /// Tests one value against this segment. `Any` and `DoubleStar` match every value.
    pub(crate) fn matches(&self, value: &str) -> bool {
        match self {
            Segment::Literal(literal) => literal.as_str() == value,
            Segment::Any | Segment::DoubleStar { .. } => true,
            Segment::Glob(tokens) => glob_match(tokens, value),
            Segment::Alternatives(alternatives) => {
                alternatives.iter().any(|alt| alt.matches(value))
            }
        }
    }

    /// True when the segment reads the match key of a visited vertex.
    pub(crate) fn needs_key(&self) -> bool {
        match self {
            Segment::Literal(_) | Segment::Glob(_) | Segment::Alternatives(_) => true,
            Segment::Any | Segment::DoubleStar { .. } => false,
        }
    }
}

/// Matches `value` against `tokens` with the classic iterative wildcard algorithm and a single
/// backtrack point for the last `*`: O(|value| · |tokens|) worst case, no allocation.
fn glob_match(tokens: &[Token], value: &str) -> bool {
    let mut t = 0usize;
    let mut s = 0usize; // byte offset into `value`, always on a char boundary
    let mut star: Option<(usize, usize)> = None; // (token after the star, value offset)
    loop {
        if let Some(token) = tokens.get(t) {
            if matches!(token, Token::Star) {
                star = Some((t + 1, s));
                t += 1;
                continue;
            }
            if let Some(c) = value.get(s..).and_then(|rest| rest.chars().next())
                && token.matches_char(c)
            {
                t += 1;
                s += c.len_utf8();
                continue;
            }
        } else if s == value.len() {
            return true;
        }
        // Mismatch: let the last star absorb one more character, or fail.
        match star {
            Some((after_star, star_s)) => {
                match value.get(star_s..).and_then(|rest| rest.chars().next()) {
                    Some(c) => {
                        let next = star_s + c.len_utf8();
                        star = Some((after_star, next));
                        t = after_star;
                        s = next;
                    }
                    None => return false,
                }
            }
            None => return false,
        }
    }
}

/// A compiled glob pattern with its automaton tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GlobPattern {
    source: String,
    segments: Vec<Segment>,
    initial: u64,
    needs_key: u64,
}

impl GlobPattern {
    /// Compiles `pattern`, or reports the first violation as
    /// [`ValueError::InvalidGlobPattern`] with its 0-based char position.
    pub(crate) fn compile(pattern: &str) -> Result<Self, ValueError> {
        let segments = Parser::new(pattern).parse()?;
        let needs_key = segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| segment.needs_key())
            .fold(0u64, |mask, (i, _)| mask | (1u64 << i));
        let mut compiled = GlobPattern {
            source: pattern.to_string(),
            segments,
            initial: 0,
            needs_key,
        };
        compiled.initial = compiled.closure(1);
        Ok(compiled)
    }

    /// The pattern text as written.
    // Used by the unit tests; the step itself needs only the automaton tables.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn as_str(&self) -> &str {
        &self.source
    }

    /// The compiled segments.
    // Used by the unit tests; the step itself needs only the automaton tables.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The segment count `n`; state `n` is the accept state.
    // Used by the unit tests; the step itself needs only the automaton tables.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn n(&self) -> usize {
        self.segments.len()
    }

    /// The ε-closure of state 0: the live states at the start vertex.
    pub(crate) fn initial(&self) -> u64 {
        self.initial
    }

    /// The states whose segment reads the match key (all except `Any` and `DoubleStar`).
    pub(crate) fn needs_key(&self) -> u64 {
        self.needs_key
    }

    /// The accept-state bit.
    pub(crate) fn accept_bit(&self) -> u64 {
        1u64 << self.segments.len()
    }

    /// True when `mask` contains the accept state.
    pub(crate) fn accepts(&self, mask: u64) -> bool {
        mask & self.accept_bit() != 0
    }

    /// The bits of the non-accept states `0..n`.
    fn state_bits(&self) -> u64 {
        self.accept_bit() - 1
    }

    /// Adds `i + 1` for every live state `i` whose segment is a non-trailing `DoubleStar`
    /// (zero levels), transitively.
    pub(crate) fn closure(&self, mask: u64) -> u64 {
        // Edges only lead from `i` to `i + 1`, so one ascending pass is transitive.
        let mut mask = mask;
        for (i, segment) in self.segments.iter().enumerate() {
            if mask & (1u64 << i) != 0 && matches!(segment, Segment::DoubleStar { trailing: false })
            {
                mask |= 1u64 << (i + 1);
            }
        }
        mask
    }

    /// The transition for one child vertex, followed by [`closure`](Self::closure). `value` is the
    /// child's match key, `None` when it is missing or not a string: named segments then do not
    /// match, wildcards still do.
    pub(crate) fn step(&self, mask: u64, value: Option<&str>) -> u64 {
        let mut next = 0u64;
        let mut live = mask & self.state_bits();
        while live != 0 {
            let i = live.trailing_zeros() as usize;
            live &= live - 1;
            let Some(segment) = self.segments.get(i) else {
                break;
            };
            match segment {
                Segment::Literal(_) | Segment::Glob(_) | Segment::Alternatives(_) => {
                    if value.is_some_and(|value| segment.matches(value)) {
                        next |= 1u64 << (i + 1);
                    }
                }
                Segment::Any => next |= 1u64 << (i + 1),
                Segment::DoubleStar { trailing: false } => next |= 1u64 << i,
                Segment::DoubleStar { trailing: true } => next |= (1u64 << i) | (1u64 << (i + 1)),
            }
        }
        self.closure(next)
    }
}

/// Returns `pattern` with every segment that uses `**` next to other characters rewritten into
/// whole-segment form (`a**b` → `a*/**/*b`, `a**` → `a*/**`, `**b` → `**/*b`, `***` → `**`), or
/// `None` when no segment needs it. Used by `help()`.
pub(crate) fn double_star_suggestion(pattern: &str) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut segments: Vec<String> = Vec::new();
    let mut changed = false;
    let mut start = 0usize;
    let mut i = 0usize;
    loop {
        let at_end = i >= chars.len();
        if at_end || chars.get(i) == Some(&'/') {
            let raw = chars.get(start..i).unwrap_or_default();
            match rewrite_double_star(raw) {
                Some(fixed) => {
                    changed = true;
                    segments.push(fixed);
                }
                None => segments.push(raw.iter().collect()),
            }
            if at_end {
                break;
            }
            i += 1;
            start = i;
        } else if chars.get(i) == Some(&'\\') {
            i += 2;
        } else {
            i += 1;
        }
    }
    changed.then(|| segments.join("/"))
}

/// Rewrites one raw segment that contains an unescaped `**` run but is not exactly `**`.
fn rewrite_double_star(raw: &[char]) -> Option<String> {
    if raw == ['*', '*'] {
        return None;
    }
    let mut first: Option<usize> = None;
    let mut last_end = 0usize;
    let mut i = 0usize;
    while i < raw.len() {
        match raw.get(i) {
            Some('\\') => i += 2,
            Some('*') => {
                let run_start = i;
                while raw.get(i) == Some(&'*') {
                    i += 1;
                }
                if i - run_start >= 2 {
                    first.get_or_insert(run_start);
                    last_end = i;
                }
            }
            _ => i += 1,
        }
    }
    let first = first?;
    let prefix: String = raw.get(..first).unwrap_or_default().iter().collect();
    let suffix: String = raw.get(last_end..).unwrap_or_default().iter().collect();
    let mut parts = Vec::new();
    if !prefix.is_empty() {
        parts.push(format!("{prefix}*"));
    }
    parts.push("**".to_string());
    if !suffix.is_empty() {
        parts.push(format!("*{suffix}"));
    }
    Some(parts.join("/"))
}

/// A parsed segment element before `{..}` expansion.
#[derive(Debug, Clone)]
enum Part {
    Char(char),
    AnyChar,
    Star,
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
    /// A `{..}` group; `position` is the char index of its `{`.
    Alternatives {
        position: usize,
        alternatives: Vec<Vec<Part>>,
    },
}

struct Parser<'a> {
    pattern: &'a str,
    chars: Vec<char>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(pattern: &'a str) -> Self {
        Parser {
            pattern,
            chars: pattern.chars().collect(),
            pos: 0,
        }
    }

    fn error(&self, position: usize, reason: &str) -> ValueError {
        ValueError::InvalidGlobPattern {
            pattern: self.pattern.to_string(),
            position,
            reason: reason.to_string(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn parse(mut self) -> Result<Vec<Segment>, ValueError> {
        if self.chars.is_empty() {
            return Err(self.error(0, reason::EMPTY_PATTERN));
        }
        if self.peek() == Some('/') {
            return Err(self.error(0, reason::LEADING_SLASH));
        }
        let mut segments: Vec<Segment> = Vec::new();
        loop {
            let start = self.pos;
            let segment = self.parse_segment()?;
            match segment {
                None => return Err(self.error(start, reason::EMPTY_SEGMENT)),
                Some(Segment::DoubleStar { .. })
                    if matches!(segments.last(), Some(Segment::DoubleStar { .. })) =>
                {
                    // `**/**` collapses into one `**`.
                }
                Some(segment) => {
                    if segments.len() == MAX_SEGMENTS {
                        return Err(self.error(start, reason::TOO_MANY_SEGMENTS));
                    }
                    segments.push(segment);
                }
            }
            if self.pos >= self.chars.len() {
                break;
            }
            // At an unescaped '/'.
            self.pos += 1;
            if self.pos >= self.chars.len() {
                return Err(self.error(self.pos - 1, reason::TRAILING_SLASH));
            }
        }
        if let Some(Segment::DoubleStar { trailing }) = segments.last_mut() {
            *trailing = true;
        }
        Ok(segments)
    }

    /// Parses one segment up to an unescaped `/` or the end; `None` for an empty segment.
    fn parse_segment(&mut self) -> Result<Option<Segment>, ValueError> {
        let start = self.pos;
        let mut parts: Vec<Part> = Vec::new();
        let mut double_star: Option<usize> = None;
        let mut last_star: Option<usize> = None;
        while let Some(c) = self.peek() {
            if c == '/' {
                break;
            }
            let part_pos = self.pos;
            let part = self.parse_part(false)?;
            if matches!(part, Part::Star) {
                if let Some(previous) = last_star
                    && matches!(parts.last(), Some(Part::Star))
                {
                    double_star.get_or_insert(previous);
                }
                last_star = Some(part_pos);
            }
            parts.push(part);
        }
        if parts.is_empty() {
            return Ok(None);
        }
        let raw = self.chars.get(start..self.pos).unwrap_or_default();
        if raw == ['*', '*'] {
            return Ok(Some(Segment::DoubleStar { trailing: false }));
        }
        if let Some(position) = double_star {
            return Err(self.error(position, reason::DOUBLE_STAR_IN_SEGMENT));
        }
        if raw == ['.'] || raw == ['.', '.'] {
            return Err(self.error(start, reason::RESERVED_SEGMENT));
        }
        self.compile_segment(parts).map(Some)
    }

    /// Parses one part at the cursor; the caller guarantees a char is present.
    fn parse_part(&mut self, in_alternatives: bool) -> Result<Part, ValueError> {
        let position = self.pos;
        let Some(c) = self.peek() else {
            return Err(self.error(position, reason::EMPTY_SEGMENT));
        };
        match c {
            '\\' => {
                let escaped = self.parse_escape()?;
                Ok(Part::Char(escaped))
            }
            '*' => {
                self.pos += 1;
                Ok(Part::Star)
            }
            '?' => {
                self.pos += 1;
                Ok(Part::AnyChar)
            }
            '[' => self.parse_class(),
            '{' if in_alternatives => Err(self.error(position, reason::NESTED_ALTERNATIVES)),
            '{' => self.parse_alternatives(),
            other => {
                self.pos += 1;
                Ok(Part::Char(other))
            }
        }
    }

    /// Consumes `\` and the char it escapes.
    fn parse_escape(&mut self) -> Result<char, ValueError> {
        let position = self.pos;
        match self.chars.get(position + 1).copied() {
            Some(escaped) => {
                self.pos += 2;
                Ok(escaped)
            }
            None => Err(self.error(position, reason::TRAILING_ESCAPE)),
        }
    }

    fn parse_class(&mut self) -> Result<Part, ValueError> {
        let open = self.pos;
        self.pos += 1;
        let mut negated = false;
        if self.peek() == Some('!') {
            negated = true;
            self.pos += 1;
        }
        let body_start = self.pos;
        let mut items: Vec<ClassItem> = Vec::new();
        loop {
            let Some(c) = self.peek().filter(|c| *c != '/') else {
                // Only a leading, literal ']' was read: this was `[]` or `[!]`.
                let only_bracket =
                    self.pos == body_start + 1 && self.chars.get(body_start) == Some(&']');
                let reason = if only_bracket {
                    reason::EMPTY_CLASS
                } else {
                    reason::UNCLOSED_CLASS
                };
                return Err(self.error(open, reason));
            };
            if c == ']' && self.pos > body_start {
                self.pos += 1;
                break;
            }
            let item_pos = self.pos;
            let lo = self.class_char()?;
            let is_range = self.peek() == Some('-')
                && self
                    .chars
                    .get(self.pos + 1)
                    .is_some_and(|next| *next != ']' && *next != '/');
            if is_range {
                self.pos += 1;
                let hi = self.class_char()?;
                if lo > hi {
                    return Err(self.error(item_pos, reason::INVERTED_RANGE));
                }
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Single(lo));
            }
        }
        Ok(Part::Class { negated, items })
    }

    /// Reads one (possibly escaped) class character.
    fn class_char(&mut self) -> Result<char, ValueError> {
        match self.peek() {
            Some('\\') => self.parse_escape(),
            Some(c) => {
                self.pos += 1;
                Ok(c)
            }
            None => Err(self.error(self.pos, reason::UNCLOSED_CLASS)),
        }
    }

    fn parse_alternatives(&mut self) -> Result<Part, ValueError> {
        let open = self.pos;
        self.pos += 1;
        let mut alternatives: Vec<Vec<Part>> = Vec::new();
        let mut current: Vec<Part> = Vec::new();
        let mut last_star: Option<usize> = None;
        loop {
            match self.peek() {
                None => return Err(self.error(open, reason::UNCLOSED_ALTERNATIVES)),
                Some('}') => {
                    self.pos += 1;
                    alternatives.push(current);
                    break;
                }
                Some(',') => {
                    self.pos += 1;
                    alternatives.push(std::mem::take(&mut current));
                    last_star = None;
                }
                Some('/') => return Err(self.error(self.pos, reason::SLASH_IN_ALTERNATIVES)),
                Some(_) => {
                    let part_pos = self.pos;
                    let part = self.parse_part(true)?;
                    if matches!(part, Part::Star) {
                        if let (Some(previous), Some(Part::Star)) = (last_star, current.last()) {
                            return Err(self.error(previous, reason::DOUBLE_STAR_IN_SEGMENT));
                        }
                        last_star = Some(part_pos);
                    }
                    current.push(part);
                }
            }
        }
        Ok(Part::Alternatives {
            position: open,
            alternatives,
        })
    }

    /// Expands the `{..}` groups of a segment and compiles every combination.
    fn compile_segment(&self, parts: Vec<Part>) -> Result<Segment, ValueError> {
        let has_alternatives = parts
            .iter()
            .any(|part| matches!(part, Part::Alternatives { .. }));
        if !has_alternatives {
            return Ok(match compile_alternative(parts) {
                AltMatcher::Literal(literal) => Segment::Literal(literal),
                AltMatcher::Glob(tokens) if tokens == [Token::Star] => Segment::Any,
                AltMatcher::Glob(tokens) => Segment::Glob(tokens),
            });
        }
        let mut combinations: Vec<Vec<Part>> = vec![Vec::new()];
        for part in parts {
            match part {
                Part::Alternatives {
                    position,
                    alternatives,
                } => {
                    if combinations.len().saturating_mul(alternatives.len()) > MAX_ALTERNATIVES {
                        return Err(self.error(position, reason::TOO_MANY_ALTERNATIVES));
                    }
                    let mut expanded = Vec::with_capacity(combinations.len() * alternatives.len());
                    for combination in &combinations {
                        for alternative in &alternatives {
                            let mut next = combination.clone();
                            next.extend(alternative.iter().cloned());
                            expanded.push(next);
                        }
                    }
                    combinations = expanded;
                }
                other => {
                    for combination in &mut combinations {
                        combination.push(other.clone());
                    }
                }
            }
        }
        let mut matchers: Vec<AltMatcher> =
            combinations.into_iter().map(compile_alternative).collect();
        if matchers.len() == 1
            && let Some(only) = matchers.pop()
        {
            return Ok(match only {
                AltMatcher::Literal(literal) => Segment::Literal(literal),
                AltMatcher::Glob(tokens) if tokens == [Token::Star] => Segment::Any,
                AltMatcher::Glob(tokens) => Segment::Glob(tokens),
            });
        }
        Ok(Segment::Alternatives(matchers))
    }
}

/// Compiles an expanded part list (no `{..}` groups left) into a literal or a token sequence;
/// consecutive `*` collapse into one.
fn compile_alternative(parts: Vec<Part>) -> AltMatcher {
    if parts.iter().all(|part| matches!(part, Part::Char(_))) {
        let literal: CompactString = parts
            .iter()
            .filter_map(|part| match part {
                Part::Char(c) => Some(*c),
                _ => None,
            })
            .collect();
        return AltMatcher::Literal(literal);
    }
    let mut tokens: Vec<Token> = Vec::with_capacity(parts.len());
    for part in parts {
        let token = match part {
            Part::Char(c) => Token::Char(c),
            Part::AnyChar => Token::AnyChar,
            Part::Star => {
                if matches!(tokens.last(), Some(Token::Star)) {
                    continue;
                }
                Token::Star
            }
            Part::Class { negated, items } => Token::Class { negated, items },
            // Expansion removed every group before this point.
            Part::Alternatives { .. } => continue,
        };
        tokens.push(token);
    }
    AltMatcher::Glob(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(pattern: &str) -> GlobPattern {
        match GlobPattern::compile(pattern) {
            Ok(compiled) => compiled,
            Err(err) => panic!("'{pattern}' should compile: {err}"),
        }
    }

    fn segments(pattern: &str) -> Vec<Segment> {
        compile(pattern).segments().to_vec()
    }

    /// Asserts that `pattern` fails with `reason` at char `position`.
    fn assert_error(pattern: &str, position: usize, expected_reason: &str) {
        match GlobPattern::compile(pattern) {
            Err(ValueError::InvalidGlobPattern {
                pattern: reported,
                position: at,
                reason,
            }) => {
                assert_eq!(reported, pattern);
                assert_eq!(reason, expected_reason, "reason for '{pattern}'");
                assert_eq!(at, position, "position for '{pattern}'");
            }
            other => panic!("'{pattern}' should fail with '{expected_reason}', got {other:?}"),
        }
    }

    fn lit(s: &str) -> Segment {
        Segment::Literal(CompactString::new(s))
    }

    fn matches(pattern: &str, value: &str) -> bool {
        let compiled = compile(pattern);
        assert_eq!(
            compiled.n(),
            1,
            "single-segment pattern expected: '{pattern}'"
        );
        compiled.segments()[0].matches(value)
    }

    fn bits(states: &[usize]) -> u64 {
        states.iter().fold(0, |mask, i| mask | (1u64 << i))
    }

    // ---- compiler -------------------------------------------------------------------------

    #[test]
    fn literals_and_escapes_compile_to_literal_segments() {
        assert_eq!(segments("src/main.rs"), vec![lit("src"), lit("main.rs")]);
        assert_eq!(compile(r"src/\*").as_str(), r"src/\*");
        assert_eq!(segments(r"a\*\?\[\]\{\}\,\\"), vec![lit(r"a*?[]{},\")]);
        assert_eq!(segments(r"a\/b/c"), vec![lit("a/b"), lit("c")]);
        // `.` and `..` inside a segment are ordinary characters.
        assert_eq!(
            segments(".gitignore/a..b"),
            vec![lit(".gitignore"), lit("a..b")]
        );
        // An escaped dot is a literal value, not the reserved segment.
        assert_eq!(segments(r"\."), vec![lit(".")]);
        // Stray closers and commas outside a group are literal.
        assert_eq!(segments("a]b}c,d"), vec![lit("a]b}c,d")]);
        // Non-ASCII.
        assert_eq!(segments("résumé"), vec![lit("résumé")]);
    }

    #[test]
    fn wildcards_compile_to_any_glob_and_double_star() {
        assert_eq!(segments("*"), vec![Segment::Any]);
        assert_eq!(
            segments("*.rs"),
            vec![Segment::Glob(vec![
                Token::Star,
                Token::Char('.'),
                Token::Char('r'),
                Token::Char('s')
            ])]
        );
        assert_eq!(
            segments("a?"),
            vec![Segment::Glob(vec![Token::Char('a'), Token::AnyChar])]
        );
        assert_eq!(segments("**"), vec![Segment::DoubleStar { trailing: true }]);
        assert_eq!(
            segments("**/a"),
            vec![Segment::DoubleStar { trailing: false }, lit("a")]
        );
        assert_eq!(
            segments("a/**/b/**"),
            vec![
                lit("a"),
                Segment::DoubleStar { trailing: false },
                lit("b"),
                Segment::DoubleStar { trailing: true }
            ]
        );
    }

    #[test]
    fn consecutive_double_stars_collapse() {
        assert_eq!(
            segments("**/**"),
            vec![Segment::DoubleStar { trailing: true }]
        );
        assert_eq!(
            segments("a/**/**/**/b"),
            vec![lit("a"), Segment::DoubleStar { trailing: false }, lit("b")]
        );
        assert_eq!(
            segments("a/**/**"),
            vec![lit("a"), Segment::DoubleStar { trailing: true }]
        );
    }

    #[test]
    fn classes_compile_with_ranges_negation_and_literal_bracket() {
        assert_eq!(
            segments("[a-c_]"),
            vec![Segment::Glob(vec![Token::Class {
                negated: false,
                items: vec![ClassItem::Range('a', 'c'), ClassItem::Single('_')]
            }])]
        );
        assert_eq!(
            segments("[!]x]"),
            vec![Segment::Glob(vec![Token::Class {
                negated: true,
                items: vec![ClassItem::Single(']'), ClassItem::Single('x')]
            }])]
        );
        // A trailing '-' is literal; escapes work inside a class.
        assert_eq!(
            segments(r"[a-][\]]"),
            vec![Segment::Glob(vec![
                Token::Class {
                    negated: false,
                    items: vec![ClassItem::Single('a'), ClassItem::Single('-')]
                },
                Token::Class {
                    negated: false,
                    items: vec![ClassItem::Single(']')]
                }
            ])]
        );
    }

    #[test]
    fn alternatives_expand_per_segment() {
        assert_eq!(
            segments("file{,.bak}"),
            vec![Segment::Alternatives(vec![
                AltMatcher::Literal(CompactString::new("file")),
                AltMatcher::Literal(CompactString::new("file.bak")),
            ])]
        );
        assert_eq!(
            segments("{a,b}{1,2}"),
            vec![Segment::Alternatives(vec![
                AltMatcher::Literal(CompactString::new("a1")),
                AltMatcher::Literal(CompactString::new("a2")),
                AltMatcher::Literal(CompactString::new("b1")),
                AltMatcher::Literal(CompactString::new("b2")),
            ])]
        );
        assert_eq!(
            segments("*.{rs,to?l}"),
            vec![Segment::Alternatives(vec![
                AltMatcher::Glob(vec![
                    Token::Star,
                    Token::Char('.'),
                    Token::Char('r'),
                    Token::Char('s')
                ]),
                AltMatcher::Glob(vec![
                    Token::Star,
                    Token::Char('.'),
                    Token::Char('t'),
                    Token::Char('o'),
                    Token::AnyChar,
                    Token::Char('l')
                ]),
            ])]
        );
        // A single alternative simplifies to its own segment kind.
        assert_eq!(segments("{a}"), vec![lit("a")]);
        assert_eq!(segments("{*}"), vec![Segment::Any]);
    }

    #[test]
    fn empty_pattern_and_misplaced_slashes_are_errors() {
        assert_error("", 0, reason::EMPTY_PATTERN);
        assert_error("/a", 0, reason::LEADING_SLASH);
        assert_error("a/", 1, reason::TRAILING_SLASH);
        assert_error("a/b/", 3, reason::TRAILING_SLASH);
        assert_error("a//b", 2, reason::EMPTY_SEGMENT);
    }

    #[test]
    fn dot_segments_are_reserved() {
        assert_error(".", 0, reason::RESERVED_SEGMENT);
        assert_error("a/..", 2, reason::RESERVED_SEGMENT);
        assert_error("a/./b", 2, reason::RESERVED_SEGMENT);
    }

    #[test]
    fn double_star_must_be_a_whole_segment() {
        assert_error("a**", 1, reason::DOUBLE_STAR_IN_SEGMENT);
        assert_error("**b", 0, reason::DOUBLE_STAR_IN_SEGMENT);
        assert_error("x/a**b", 3, reason::DOUBLE_STAR_IN_SEGMENT);
        assert_error("***", 0, reason::DOUBLE_STAR_IN_SEGMENT);
        assert_error("{a**,b}", 2, reason::DOUBLE_STAR_IN_SEGMENT);
        // Escaped stars are literal and never form `**`.
        assert_eq!(
            segments(r"\**"),
            vec![Segment::Glob(vec![Token::Char('*'), Token::Star])]
        );
        // Two stars from different groups are not `**` in the source text.
        assert!(GlobPattern::compile("a{*,}*").is_ok());
    }

    #[test]
    fn double_star_suggestions_rewrite_the_offending_segment() {
        assert_eq!(double_star_suggestion("a**b").as_deref(), Some("a*/**/*b"));
        assert_eq!(double_star_suggestion("a**").as_deref(), Some("a*/**"));
        assert_eq!(double_star_suggestion("**b").as_deref(), Some("**/*b"));
        assert_eq!(double_star_suggestion("***").as_deref(), Some("**"));
        assert_eq!(
            double_star_suggestion("src/a**b/c").as_deref(),
            Some("src/a*/**/*b/c")
        );
        assert_eq!(double_star_suggestion("**/a"), None);
        assert_eq!(double_star_suggestion(r"\**"), None);
    }

    #[test]
    fn escape_errors() {
        assert_error(r"a\", 1, reason::TRAILING_ESCAPE);
        assert_error(r"[a\", 2, reason::TRAILING_ESCAPE);
    }

    #[test]
    fn class_errors() {
        assert_error("a[bc", 1, reason::UNCLOSED_CLASS);
        assert_error("[a/b]", 0, reason::UNCLOSED_CLASS);
        assert_error("[]", 0, reason::EMPTY_CLASS);
        assert_error("x[!]", 1, reason::EMPTY_CLASS);
        assert_error("[!", 0, reason::UNCLOSED_CLASS);
        assert_error("a[z-a]", 2, reason::INVERTED_RANGE);
    }

    #[test]
    fn alternatives_errors() {
        assert_error("a{b,c", 1, reason::UNCLOSED_ALTERNATIVES);
        assert_error("{a,{b}}", 3, reason::NESTED_ALTERNATIVES);
        assert_error("{src/a,b}", 4, reason::SLASH_IN_ALTERNATIVES);
    }

    #[test]
    fn alternatives_are_capped_at_256_per_segment() {
        // 16 × 16 = 256 is allowed.
        let sixteen = "{a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p}";
        let ok = format!("{sixteen}{sixteen}");
        match compile(&ok).segments() {
            [Segment::Alternatives(alternatives)] => assert_eq!(alternatives.len(), 256),
            other => panic!("expected 256 alternatives, got {other:?}"),
        }
        // A third group of two exceeds the cap; the error points at that group's '{'.
        let too_many = format!("{ok}{{x,y}}");
        assert_error(
            &too_many,
            2 * sixteen.chars().count(),
            reason::TOO_MANY_ALTERNATIVES,
        );
        // The cap is per segment: two segments of 256 each are fine.
        assert!(GlobPattern::compile(&format!("{ok}/{ok}")).is_ok());
    }

    #[test]
    fn segments_are_capped_at_63() {
        let max = vec!["a"; MAX_SEGMENTS].join("/");
        assert_eq!(compile(&max).n(), 63);
        let over = vec!["a"; MAX_SEGMENTS + 1].join("/");
        // The 64th segment starts at char 2 * 63.
        assert_error(&over, 2 * MAX_SEGMENTS, reason::TOO_MANY_SEGMENTS);
        // Collapsed `**` runs count once.
        let collapsed = format!("{max}/**/**");
        assert!(GlobPattern::compile(&collapsed).is_err());
        let collapsed_ok = format!("{}/**/**", vec!["a"; MAX_SEGMENTS - 1].join("/"));
        assert_eq!(compile(&collapsed_ok).n(), 63);
    }

    // ---- matcher --------------------------------------------------------------------------

    #[test]
    fn literal_fast_path_is_exact_and_case_sensitive() {
        assert!(matches("main.rs", "main.rs"));
        assert!(!matches("main.rs", "Main.rs"));
        assert!(!matches("main.rs", "main.rs "));
        assert!(!matches("main.rs", ""));
    }

    #[test]
    fn star_backtracks() {
        assert!(matches("a*b*c", "abc"));
        assert!(matches("a*b*c", "aXbYc"));
        assert!(matches("a*b*c", "abbbc"));
        assert!(matches("a*b*c", "abcbc"));
        assert!(matches("a*b*c", "aXbXbXc"));
        assert!(!matches("a*b*c", "aXbXcX"));
        assert!(!matches("a*b*c", "acb"));
        assert!(matches("*a*a*a", "aaaa"));
        assert!(!matches("*a*a*a", "aab"));
        assert!(matches("*.rs", ".rs"));
        // No hidden-file rule: `*` matches a leading dot.
        assert!(matches("*", ".hidden"));
        assert!(matches("*rc", ".bashrc"));
        // `*` matches a '/' inside a value.
        assert!(matches("a*b", "a/b"));
        assert!(matches("x*", "x"));
    }

    #[test]
    fn question_mark_matches_one_unicode_scalar() {
        assert!(matches("r?sum?.md", "résumé.md"));
        assert!(!matches("r?sum?.md", "rsum.md"));
        assert!(matches("??", "日本"));
        assert!(!matches("?", "日本"));
        assert!(!matches("?", ""));
    }

    #[test]
    fn classes_match_items_ranges_and_negation() {
        assert!(matches("file[0-9].txt", "file7.txt"));
        assert!(!matches("file[0-9].txt", "fileA.txt"));
        assert!(matches("[!0-9]*", "a1"));
        assert!(!matches("[!0-9]*", "1a"));
        assert!(matches("[]]", "]"));
        assert!(!matches("[]]", "a"));
        assert!(matches("[!]]", "a"));
        assert!(!matches("[!]]", "]"));
        assert!(matches("[é-ë]", "ê"));
        assert!(matches("[a-]", "-"));
    }

    #[test]
    fn escapes_match_literally() {
        assert!(matches(r"\*", "*"));
        assert!(!matches(r"\*", "a"));
        assert!(matches(r"a\?", "a?"));
        assert!(!matches(r"a\?", "ab"));
        assert!(matches(r"a\/b", "a/b"));
        assert!(matches(r"\{a,b\}", "{a,b}"));
        assert!(matches(r"x\\", r"x\"));
    }

    #[test]
    fn alternatives_match_any_including_empty() {
        assert!(matches("file{,.bak}", "file"));
        assert!(matches("file{,.bak}", "file.bak"));
        assert!(!matches("file{,.bak}", "file.old"));
        assert!(matches("*.{rs,toml}", "Cargo.toml"));
        assert!(!matches("*.{rs,toml}", "README.md"));
        assert!(matches("{a,b}{1,2}", "b1"));
    }

    // ---- automaton tables -----------------------------------------------------------------

    #[test]
    fn initial_and_closure_follow_non_trailing_double_stars() {
        assert_eq!(compile("a/b").initial(), bits(&[0]));
        assert_eq!(compile("**/a").initial(), bits(&[0, 1]));
        assert_eq!(compile("**/a/**/b").initial(), bits(&[0, 1]));
        // A trailing `**` has no ε-edge: `**` alone does not accept the start vertex.
        assert_eq!(compile("**").initial(), bits(&[0]));
        assert_eq!(compile("a/**").initial(), bits(&[0]));
        let pattern = compile("a/**/b/**/c");
        assert_eq!(pattern.closure(bits(&[1])), bits(&[1, 2]));
        assert_eq!(pattern.closure(bits(&[3])), bits(&[3, 4]));
        assert_eq!(pattern.closure(bits(&[0])), bits(&[0]));
    }

    #[test]
    fn needs_key_excludes_wildcard_segments() {
        assert_eq!(compile("a/*/**/b?/{x,y}/*").needs_key(), bits(&[0, 3, 4]));
        assert_eq!(compile("**").needs_key(), 0);
    }

    #[test]
    fn double_star_prefix_finds_a_match_at_depth_one() {
        let pattern = compile("**/a");
        let accept = pattern.accept_bit();
        let after = pattern.step(pattern.initial(), Some("a"));
        assert!(pattern.accepts(after));
        assert_eq!(after, bits(&[0, 1]) | accept);
        let other = pattern.step(pattern.initial(), Some("b"));
        assert!(!pattern.accepts(other));
        assert_eq!(other, bits(&[0, 1]));
        // Deeper: b/a.
        assert!(pattern.accepts(pattern.step(other, Some("a"))));
    }

    #[test]
    fn trailing_double_star_accepts_descendants_but_not_the_segment_itself() {
        let pattern = compile("a/**");
        let at_a = pattern.step(pattern.initial(), Some("a"));
        assert!(!pattern.accepts(at_a), "`a/**` must not accept `a` itself");
        assert_eq!(at_a, bits(&[1]));
        let child = pattern.step(at_a, Some("x"));
        assert!(pattern.accepts(child));
        assert_eq!(child, bits(&[1, 2]));
        let grandchild = pattern.step(child, None);
        assert!(pattern.accepts(grandchild));
    }

    #[test]
    fn lone_double_star_accepts_every_depth_from_one() {
        let pattern = compile("**");
        assert!(!pattern.accepts(pattern.initial()));
        let mut mask = pattern.initial();
        for _ in 0..5 {
            mask = pattern.step(mask, None);
            assert!(pattern.accepts(mask));
            assert_eq!(mask, bits(&[0, 1]));
        }
    }

    #[test]
    fn missing_value_blocks_named_segments_but_not_wildcards() {
        let named = compile("a/b");
        assert_eq!(named.step(named.initial(), None), 0);
        let glob = compile("a*");
        assert_eq!(glob.step(glob.initial(), None), 0);
        let alternatives = compile("{a,b}");
        assert_eq!(alternatives.step(alternatives.initial(), None), 0);
        let any = compile("*/a");
        assert_eq!(any.step(any.initial(), None), bits(&[1]));
        let double = compile("**/a");
        assert_eq!(double.step(double.initial(), None), bits(&[0, 1]));
    }

    #[test]
    fn accept_state_has_no_outgoing_transitions() {
        let pattern = compile("a");
        let accepted = pattern.step(pattern.initial(), Some("a"));
        assert!(pattern.accepts(accepted));
        assert_eq!(pattern.step(accepted, Some("a")), 0);
    }

    #[test]
    fn step_walks_a_full_pattern() {
        let pattern = compile("**/author/*/some/*.rs");
        let mut mask = pattern.initial();
        for level in ["src", "author", "x", "some"] {
            mask = pattern.step(mask, Some(level));
            assert!(!pattern.accepts(mask), "no accept at '{level}'");
        }
        assert!(pattern.accepts(pattern.step(mask, Some("lib.rs"))));
        assert!(!pattern.accepts(pattern.step(mask, Some("lib.toml"))));
    }
}
