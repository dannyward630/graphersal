//! Recursive-descent (precedence-climbing) parser that compiles straight to the postfix program
//! of [`MathExpr`]. Operators and functions come from the [`registry`](super::registry) tables.

use compact_str::CompactString;

use super::lexer::{Token, TokenKind, tokenize};
use super::registry::{self, Arity, Assoc, FunctionDef};
use super::{
    Instr, MAX_CALL_ARGS, MAX_DEPTH, MathExpr, MathParseErrorKind as K, PathSeg, RawParseError,
    VarRef,
};

/// What an operand position expects, for error messages.
const EXPECTED_OPERAND: &str = "a number, a variable, a function or '('";

/// Parses and compiles `src`.
pub(crate) fn parse(src: &str) -> Result<MathExpr, RawParseError> {
    if src.trim().is_empty() {
        return Err(RawParseError::new(0, K::Empty));
    }
    let mut p = Parser {
        toks: tokenize(src)?,
        pos: 0,
        depth: 0,
        program: Vec::new(),
        refs: Vec::new(),
        names: Vec::new(),
        stack: 0,
        max_stack: 0,
    };
    p.parse_expr(0)?;
    let tok = p.peek();
    if tok.kind != TokenKind::End {
        return Err(p.after_operand_error(tok, "an operator or the end of the expression"));
    }
    Ok(MathExpr {
        program: p.program.into_boxed_slice(),
        refs: p.refs.into_boxed_slice(),
        names: p.names.into_boxed_slice(),
        max_stack: p.max_stack,
    })
}

/// Parser state.
struct Parser<'a> {
    /// All tokens, ending with `End`.
    toks: Vec<Token<'a>>,
    /// Index of the next token.
    pos: usize,
    /// Current nesting depth of `parse_expr`.
    depth: usize,
    /// The program emitted so far.
    program: Vec<Instr>,
    /// Variable occurrences emitted so far.
    refs: Vec<VarRef>,
    /// Distinct variable names (slot order).
    names: Vec<CompactString>,
    /// Value-stack height after the emitted program.
    stack: usize,
    /// Highest value-stack height seen.
    max_stack: usize,
}

/// Human description of a token for error messages.
fn describe(tok: &Token<'_>) -> String {
    match &tok.kind {
        TokenKind::Number(_) => format!("number '{}'", tok.text),
        TokenKind::Ident => format!("name '{}'", tok.text),
        TokenKind::Str(_) => format!("string {}", tok.text),
        TokenKind::Op(_)
        | TokenKind::Open(_)
        | TokenKind::Close(_)
        | TokenKind::Comma
        | TokenKind::Dot => {
            format!("'{}'", tok.text)
        }
        TokenKind::End => "end of expression".to_string(),
    }
}

/// The closer that matches an opener.
fn closer(open: char) -> char {
    match open {
        '[' => ']',
        '{' => '}',
        _ => ')',
    }
}

/// Whether `name` matches TinkerPop's variable pattern `[a-zA-Z_][a-zA-Z0-9_]*`.
fn is_ascii_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl<'a> Parser<'a> {
    /// The next token (the final `End` repeats forever).
    fn peek(&self) -> Token<'a> {
        let i = self.pos.min(self.toks.len().saturating_sub(1));
        self.toks.get(i).cloned().unwrap_or(Token {
            kind: TokenKind::End,
            start: 0,
            text: "",
            space_before: false,
        })
    }

    /// Consumes the next token.
    fn advance(&mut self) {
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
    }

    /// Appends an instruction and tracks the value-stack height.
    fn emit(&mut self, instr: Instr) {
        let (pop, push) = match &instr {
            Instr::Push(_) | Instr::Var(_) => (0, 1),
            Instr::Unary(_) => (1, 1),
            Instr::Binary { .. } => (2, 1),
            Instr::Call { argc, .. } => (*argc, 1),
        };
        self.stack = self.stack.saturating_sub(pop) + push;
        self.max_stack = self.max_stack.max(self.stack);
        self.program.push(instr);
    }

    /// The error for a token found where an operator, closer or end was expected.
    fn after_operand_error(&self, tok: Token<'a>, expected: &'static str) -> RawParseError {
        let kind = match &tok.kind {
            TokenKind::Number(_) | TokenKind::Ident | TokenKind::Open(_) | TokenKind::Str(_) => {
                K::MissingOperator {
                    found: describe(&tok),
                }
            }
            TokenKind::Close(c) => K::Unmatched(*c),
            _ => K::Unexpected {
                found: describe(&tok),
                expected,
            },
        };
        RawParseError::new(tok.start, kind)
    }

    /// `expr(min) := prefix (binop[prec >= min] expr(prec or prec+1))*` (precedence climbing).
    fn parse_expr(&mut self, min_prec: u16) -> Result<(), RawParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(RawParseError::new(self.peek().start, K::TooDeep(MAX_DEPTH)));
        }
        self.parse_prefix()?;
        loop {
            let tok = self.peek();
            let TokenKind::Op(c) = tok.kind else { break };
            let Some(op) = registry::binary_op(c).filter(|op| op.precedence >= min_prec) else {
                break;
            };
            self.advance();
            let next_min = match op.assoc {
                Assoc::Left => op.precedence.saturating_add(1),
                Assoc::Right => op.precedence,
            };
            self.parse_expr(next_min)?;
            self.emit(Instr::Binary {
                op,
                offset: tok.start,
            });
        }
        self.depth -= 1;
        Ok(())
    }

    /// An operand: unary operator, number, bracketed group, call, constant or variable.
    fn parse_prefix(&mut self) -> Result<(), RawParseError> {
        let tok = self.peek();
        match &tok.kind {
            TokenKind::Op(c) if registry::unary_op(*c).is_some() => {
                self.advance();
                if let Some(op) = registry::unary_op(*c) {
                    self.parse_expr(op.precedence)?;
                    self.emit(Instr::Unary(op));
                }
                Ok(())
            }
            TokenKind::Number(v) => {
                self.advance();
                self.emit(Instr::Push(*v));
                Ok(())
            }
            TokenKind::Open(open) => {
                self.advance();
                self.parse_expr(0)?;
                self.expect_close(*open, tok.start)
            }
            TokenKind::Ident => {
                self.advance();
                if let Some(func) = registry::function(tok.text) {
                    return self.parse_call(func, &tok);
                }
                match registry::constant(tok.text) {
                    Some(c) if !is_ascii_identifier(c.name) => {
                        self.emit(Instr::Push(c.value));
                        Ok(())
                    }
                    c => self.parse_variable(&tok, c.map(|c| c.value)),
                }
            }
            _ => Err(RawParseError::new(
                tok.start,
                K::Unexpected {
                    found: describe(&tok),
                    expected: EXPECTED_OPERAND,
                },
            )),
        }
    }

    /// Consumes the closer matching `open` (opened at `open_offset`).
    fn expect_close(&mut self, open: char, open_offset: usize) -> Result<(), RawParseError> {
        let tok = self.peek();
        match tok.kind {
            TokenKind::Close(c) if c == closer(open) => {
                self.advance();
                Ok(())
            }
            TokenKind::Close(close) => Err(RawParseError::new(
                tok.start,
                K::MismatchedBracket { open, close },
            )),
            TokenKind::End => Err(RawParseError::new(open_offset, K::Unclosed(open))),
            _ => Err(self.after_operand_error(tok, "an operator or a closing bracket")),
        }
    }
}

impl<'a> Parser<'a> {
    /// A call after the function name `name`: `f(a, b)` with any bracket kind, or exp4j's
    /// paren-less form `f x`, whose single argument is the rest of the enclosing group.
    fn parse_call(
        &mut self,
        func: &'static FunctionDef,
        name: &Token<'a>,
    ) -> Result<(), RawParseError> {
        let tok = self.peek();
        let argc = match tok.kind {
            TokenKind::Open(open) => {
                self.advance();
                let mut argc = 0;
                if !matches!(self.peek().kind, TokenKind::Close(_)) {
                    loop {
                        self.parse_expr(0)?;
                        argc += 1;
                        if self.peek().kind != TokenKind::Comma {
                            break;
                        }
                        self.advance();
                    }
                }
                self.expect_close(open, tok.start)?;
                argc
            }
            _ => {
                if func.arity != Arity::Exact(1) {
                    return Err(RawParseError::new(
                        name.start,
                        K::CallNeedsParens {
                            name: func.name,
                            expected: func.arity,
                        },
                    ));
                }
                if matches!(tok.kind, TokenKind::Op('+' | '-')) {
                    return Err(RawParseError::new(
                        tok.start,
                        K::SignedArgumentWithoutParens { name: func.name },
                    ));
                }
                self.parse_expr(0)?;
                1
            }
        };
        if argc > MAX_CALL_ARGS {
            return Err(RawParseError::new(
                name.start,
                K::TooManyArguments(MAX_CALL_ARGS),
            ));
        }
        if !func.arity.accepts(argc) {
            return Err(RawParseError::new(
                name.start,
                K::Arity {
                    name: func.name,
                    expected: func.arity,
                    found: argc,
                },
            ));
        }
        self.emit(Instr::Call {
            func,
            argc,
            offset: name.start,
        });
        Ok(())
    }

    /// A variable `name` with its optional nested-access path.
    fn parse_variable(
        &mut self,
        name: &Token<'a>,
        constant: Option<f64>,
    ) -> Result<(), RawParseError> {
        let slot = match self.names.iter().position(|n| n == name.text) {
            Some(i) => i,
            None => {
                self.names.push(CompactString::from(name.text));
                self.names.len() - 1
            }
        };
        let slot = u16::try_from(slot).map_err(|_| {
            RawParseError::new(name.start, K::TooManyVariables(usize::from(u16::MAX) + 1))
        })?;
        let path = self.parse_path()?;
        let next = self.peek();
        if path.is_empty() && next.kind == TokenKind::Open('[') && next.space_before {
            return Err(RawParseError::new(
                next.start,
                K::BadPath("no whitespace is allowed before '[' in an access path"),
            ));
        }
        if path.is_empty() && matches!(next.kind, TokenKind::Open(_)) {
            let available = registry::FUNCTIONS
                .iter()
                .map(|f| f.name)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RawParseError::new(
                name.start,
                K::UnknownFunction {
                    name: name.text.to_string(),
                    available,
                },
            ));
        }
        self.refs.push(VarRef {
            slot,
            path,
            offset: name.start,
            constant,
        });
        self.emit(Instr::Var(self.refs.len() - 1));
        Ok(())
    }

    /// `( '.' ident | '[' integer ']' | '[' string ']' )*`, each segment directly adjacent.
    fn parse_path(&mut self) -> Result<Vec<PathSeg>, RawParseError> {
        let mut path = Vec::new();
        loop {
            let tok = self.peek();
            if tok.space_before {
                return Ok(path);
            }
            match tok.kind {
                TokenKind::Dot => {
                    self.advance();
                    let key = self.peek();
                    if key.kind == TokenKind::Ident
                        && !key.space_before
                        && is_ascii_identifier(key.text)
                    {
                        self.advance();
                        path.push(PathSeg::Key(CompactString::from(key.text)));
                    } else {
                        return Err(RawParseError::new(
                            key.start,
                            K::BadPath("expected a key name after '.'"),
                        ));
                    }
                }
                TokenKind::Number(_) if tok.text.starts_with('.') => {
                    return Err(RawParseError::new(
                        tok.start,
                        K::BadPath("a key after '.' must be a name; write [n] for a list index"),
                    ));
                }
                TokenKind::Open('[') => {
                    self.advance();
                    path.push(self.parse_bracket_segment()?);
                    let close = self.peek();
                    match close.kind {
                        TokenKind::Close(']') => self.advance(),
                        TokenKind::End => {
                            return Err(RawParseError::new(tok.start, K::Unclosed('[')));
                        }
                        _ => {
                            return Err(RawParseError::new(
                                close.start,
                                K::BadPath("expected ']'"),
                            ));
                        }
                    }
                }
                _ => return Ok(path),
            }
        }
    }

    /// The content of `[...]` in a path: a quoted key or an optionally negative integer.
    fn parse_bracket_segment(&mut self) -> Result<PathSeg, RawParseError> {
        let tok = self.peek();
        let (negative, num) = match tok.kind {
            TokenKind::Str(s) => {
                self.advance();
                return Ok(PathSeg::Key(CompactString::from(s)));
            }
            TokenKind::Op('-') => {
                self.advance();
                (true, self.peek())
            }
            _ => (false, tok.clone()),
        };
        if !matches!(num.kind, TokenKind::Number(_)) {
            return Err(RawParseError::new(
                num.start,
                K::BadPath("expected an integer index or a quoted key inside [...]"),
            ));
        }
        let parsed = if num.text.bytes().all(|b| b.is_ascii_digit()) {
            num.text.parse::<i64>().ok()
        } else {
            None
        };
        let Some(value) = parsed else {
            return Err(RawParseError::new(
                num.start,
                K::BadIndex(num.text.to_string()),
            ));
        };
        self.advance();
        Ok(PathSeg::Index(if negative { -value } else { value }))
    }
}
