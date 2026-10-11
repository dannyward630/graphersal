//! Tokenizer of the `math()` expression language (see the module docs for the grammar).

use super::{MathParseErrorKind, RawParseError};

/// What a [`Token`] is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    /// A number literal; `text` keeps the raw spelling (needed to validate index literals).
    Number(f64),
    /// An identifier: `[A-Za-z_][A-Za-z0-9_]*`, or one of the single-character constant names
    /// (`π`, `φ`).
    Ident,
    /// A quoted string (only valid as a bracket key): the unescaped content.
    Str(String),
    /// An operator character: `+ - * / % ^`.
    Op(char),
    /// An opening bracket: `(`, `[` or `{` (exp4j treats all three as parentheses).
    Open(char),
    /// A closing bracket: `)`, `]` or `}`.
    Close(char),
    /// `,`, the argument separator.
    Comma,
    /// `.`, not followed by a digit (a field access of the nested-access extension).
    Dot,
    /// End of input.
    End,
}

/// A token with its byte span in the source.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token<'a> {
    /// The kind (and payload).
    pub(crate) kind: TokenKind,
    /// Byte offset of the first character.
    pub(crate) start: usize,
    /// The raw source text of the token.
    pub(crate) text: &'a str,
    /// Whether whitespace separates this token from the previous one.
    pub(crate) space_before: bool,
}

/// Characters exp4j treats as operator characters (`Operator.ALLOWED_OPERATOR_CHARS`); only the
/// first six are built-in operators.
const OPERATOR_CHARS: &[char] = &['+', '-', '*', '/', '%', '^'];

/// Splits `src` into tokens, ending with one [`TokenKind::End`].
pub(crate) fn tokenize(src: &str) -> Result<Vec<Token<'_>>, RawParseError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        let ws_start = pos;
        while let Some(c) = src[pos..].chars().next().filter(|c| c.is_whitespace()) {
            pos += c.len_utf8();
        }
        let space_before = pos > ws_start;
        let start = pos;
        let Some(c) = src[pos..].chars().next() else {
            out.push(Token {
                kind: TokenKind::End,
                start,
                text: "",
                space_before,
            });
            return Ok(out);
        };
        let next_is_digit = bytes.get(pos + 1).is_some_and(u8::is_ascii_digit);
        let kind = if c.is_ascii_digit() || (c == '.' && next_is_digit) {
            pos = scan_number(bytes, pos);
            let text = &src[start..pos];
            match text.parse::<f64>() {
                Ok(v) => TokenKind::Number(v),
                Err(_) => {
                    return Err(RawParseError::new(
                        start,
                        MathParseErrorKind::InvalidNumber(text.into()),
                    ));
                }
            }
        } else if c.is_ascii_alphabetic() || c == '_' {
            pos += 1;
            while bytes
                .get(pos)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                pos += 1;
            }
            TokenKind::Ident
        } else if c == 'π' || c == 'φ' {
            pos += c.len_utf8();
            TokenKind::Ident
        } else if c == '"' || c == '\'' {
            let (content, end) = scan_string(src, pos, c)?;
            pos = end;
            TokenKind::Str(content)
        } else {
            pos += c.len_utf8();
            match c {
                '(' | '[' | '{' => TokenKind::Open(c),
                ')' | ']' | '}' => TokenKind::Close(c),
                ',' => TokenKind::Comma,
                '.' => TokenKind::Dot,
                c if OPERATOR_CHARS.contains(&c) => TokenKind::Op(c),
                c => {
                    return Err(RawParseError::new(
                        start,
                        MathParseErrorKind::UnexpectedChar(c),
                    ));
                }
            }
        };
        out.push(Token {
            kind,
            start,
            text: &src[start..pos],
            space_before,
        });
    }
}

/// exp4j `Tokenizer.parseNumberToken`: greedily takes digits, `.`, `e`/`E`, and a sign right
/// after `e`/`E`; a trailing `e`/`E` is given back (so `2e` is the number `2` followed by the
/// name `e`). The text is then parsed as a whole, so `1.2.3` or `1e+` is an invalid number.
fn scan_number(bytes: &[u8], start: usize) -> usize {
    let mut pos = start + 1;
    while let Some(&b) = bytes.get(pos) {
        let after_e = matches!(bytes.get(pos - 1), Some(b'e' | b'E'));
        if b.is_ascii_digit()
            || matches!(b, b'.' | b'e' | b'E')
            || (after_e && matches!(b, b'+' | b'-'))
        {
            pos += 1;
        } else {
            break;
        }
    }
    if matches!(bytes.get(pos - 1), Some(b'e' | b'E')) {
        pos -= 1;
    }
    pos
}

/// Scans a string quoted with `quote` starting at `start`; returns the unescaped content and
/// the byte offset after the closing quote. Escapes: `\\`, `\"`, `\'`.
fn scan_string(src: &str, start: usize, quote: char) -> Result<(String, usize), RawParseError> {
    let mut content = String::new();
    let mut chars = src[start + 1..].char_indices();
    while let Some((i, c)) = chars.next() {
        let at = start + 1 + i;
        if c == quote {
            return Ok((content, at + 1));
        }
        if c == '\\' {
            match chars.next() {
                Some((_, e @ ('\\' | '"' | '\''))) => content.push(e),
                Some((_, e)) => {
                    return Err(RawParseError::new(at, MathParseErrorKind::InvalidEscape(e)));
                }
                None => break,
            }
        } else {
            content.push(c);
        }
    }
    Err(RawParseError::new(
        start,
        MathParseErrorKind::UnterminatedString,
    ))
}
