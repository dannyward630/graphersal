//! The `math()` expression language: an own recursive-descent engine with the semantics of
//! exp4j 0.4.8 as TinkerPop 3.8.2's `MathStep` configures it, plus Graphersal's nested-access
//! extension. Pure: no graph, no Rhai. Parse once with [`MathExpr::parse`], evaluate per
//! traverser with [`MathExpr::eval`] (no allocation per evaluation).
//!
//! Extend the language only through the tables in [`registry`] (`FUNCTIONS`, `BINARY_OPS`,
//! `UNARY_OPS`, `CONSTANTS`) plus one functional test row; the parser and evaluator are generic.
//!
//! # Reference semantics (verified against the sources)
//!
//! Sources: exp4j 0.4.8 (`net.objecthunter.exp4j`: `tokenizer/Tokenizer.java`,
//! `shuntingyard/ShuntingYard.java`, `operator/Operator.java`, `operator/Operators.java`,
//! `function/Functions.java`, `ExpressionBuilder.java`, `Expression.java`) and TinkerPop 3.8.2
//! `gremlin-core/.../traversal/step/map/MathStep.java`.
//!
//! - **Builder options.** `MathStep.TinkerExpression` builds with
//!   `ExpressionBuilder(equation).variables(vars).implicitMultiplication(false)`, so `2 x`,
//!   `2(3)`, `2 pi`, `1 2` are errors. Variables are collected by `MathStep.VARIABLE_PATTERN`:
//!   every `\b[a-zA-Z_][a-zA-Z0-9_]*\b` that does not start with a digit and is not one of the 19
//!   function names in `MathStep.FUNCTIONS`, in first-appearance order (`LinkedHashSet`).
//! - **Precedence** (`Operator.PRECEDENCE_*`, higher binds tighter): binary `+ -` 500 <
//!   `* / %` 1000 < unary `- +` 5000 < `^` 10000. Binary operators are left-associative except
//!   `^` (right). Unary operators are right-associative and never pop a pending binary
//!   operator (`ShuntingYard`), so `-2^2 = -4`, `2^-2^2 = 2^(-(2^2))`, `2^3^2 = 512`,
//!   `-7%3 = (-7)%3 = -1`, `--2 = 2`, `+-2 = -2`, `2--2 = 4`.
//! - **Unary vs binary** (`Tokenizer.getOperator`): `+`/`-` is unary at the start, after an
//!   opening bracket, after `,` and after another operator; everywhere else binary.
//! - **Brackets**: exp4j treats `(`, `[`, `{` all as parentheses. Graphersal accepts all three
//!   for grouping and calls, but requires the closer to match the opener (exp4j accepts `(1]`).
//! - **Functions** (`Functions.getBuiltinFunction`), 23, all arity 1 except `pow` (2):
//!   `abs acos asin atan cbrt ceil cos cosh cot exp expm1 floor log log10 log1p log2 pow signum
//!   sin sinh sqrt tan tanh`. `log` is the natural log; `log2(x)` is `log(x)/log(2)`;
//!   `signum` returns 0 for NaN and -0.0; `cot(x)` throws "Division by zero in cotangent!" where
//!   `tan(x) == 0`. Domain errors are not errors: `sqrt(-1)` and `log(-1)` are NaN,
//!   `log(0)` is -inf (`java.lang.Math`). `pow`/`^` follow `Math.pow` (NaN exponent → NaN,
//!   `(±1)^±inf` → NaN), see [`registry::java_pow`].
//! - **Function call syntax.** A function token is pushed on the operator stack and is only
//!   emitted by a closing parenthesis or at the end, so a function name *without* parentheses
//!   applies to everything to its right up to the end of the enclosing group:
//!   `sin _` = `sin(_)`, `sin _ + 1` = `sin(_ + 1)`, `2 * sin _ + 1` = `2 * sin(_ + 1)`,
//!   `(sin _ + 1) * 2` = `sin(_ + 1) * 2`. A `+`/`-` right after a function name is a binary
//!   operator in exp4j, so `sin -1` fails; Graphersal rejects it with a hint to write `sin(-1)`.
//!   exp4j does not check the argument count syntactically (it pops `numArguments` values at
//!   evaluation and fails on a stack mismatch); Graphersal checks the arity at parse time.
//! - **Constants** (`Expression.createDefaultVariables`): `pi` = `π` = `Math.PI`, `e` =
//!   `Math.E`, `φ` = 1.61803398874 (exp4j's truncated literal). exp4j implements them as
//!   pre-set variables, so a declared variable of the same name overrides the value. `pi`/`e`
//!   match `MathStep.VARIABLE_PATTERN`, so TinkerPop declares them as step variables and
//!   resolves them through `Scoping` (a scope key `e` wins; without one TinkerPop throws
//!   "must resolve to a Number"). `π`/`φ` never match the ASCII pattern and are plain constants.
//! - **Division by zero**: `/` and `%` throw `ArithmeticException("Division by zero!")` when the
//!   divisor `== 0d` (also `-0.0`); a NaN divisor yields NaN; `x/inf` is 0.
//! - **Number literals** (`Tokenizer.parseNumberToken` + `Double.parseDouble`): start with a
//!   digit or `.`, greedily continue over digits, `.`, `e`, `E`, and `+`/`-` right after
//!   `e`/`E`; a trailing `e`/`E` is given back. `12`, `1.5`, `.5`, `5.`, `1e3`, `1E-3`,
//!   `2.5e+2`, `2.e3` are valid; `1.2.3`, `1e+`, `1ee3` are errors. No sign, hex or `inf`.
//! - **Identifiers** (`Tokenizer.parseFunctionOrVariable`): exp4j takes the longest prefix that
//!   is a known variable or function. With MathStep's declared variables that equals maximal
//!   munch over `[A-Za-z_][A-Za-z0-9_]*`: `sinx`, `sinh2`, `log2x` are variables, `sin(x)` a
//!   call. Whitespace is skipped between tokens (`Character.isWhitespace`); an empty or blank
//!   expression is an error ("Expression can not be empty").
//!
//! # Graphersal deviations
//!
//! - `cot`, `expm1`, `log1p`, `pow` are usable. In TinkerPop they are missing from
//!   `MathStep.FUNCTIONS`, get declared as variables, and `ExpressionBuilder.build()` throws "A
//!   variable can not have the same name as a function" — so no working TinkerPop query uses them.
//! - `pi`/`e` are variables (they take part in [`MathExpr::variables`] and the `by()` ring as in
//!   TinkerPop) that carry their constant as a fallback ([`VarRef::constant`]): a resolver uses
//!   a scope value when one exists and the constant otherwise, where TinkerPop would throw.
//! - Arity and a signed paren-less argument are parse errors (exp4j: evaluation errors).
//! - Brackets must match in kind.
//! - **Nested access** extension, after any variable, with no whitespace before `.`/`[`:
//!   `_.a.b`, `_[0]`, `_.items[2].price`, `_["unit price"]`, `_['k']`, `a.b + c.d[1]`.
//!   Keys after `.` follow the identifier grammar; quoted keys use `"` or `'` with the escapes
//!   `\\`, `\"`, `\'`; an index is an optionally negative decimal integer (its meaning is the
//!   resolver's). exp4j rejects every such expression (a `.` after a name starts an invalid
//!   number; `[` after a name needs implicit multiplication), so no TinkerPop expression changes
//!   meaning.
//! - Nesting deeper than [`MAX_DEPTH`] (brackets, operators, calls) is a parse error instead of
//!   a stack overflow.

mod eval;
mod lexer;
mod parser;
pub(crate) mod registry;
#[cfg(test)]
mod tests;

use compact_str::CompactString;
use registry::{BinaryOpDef, FunctionDef, UnaryOpDef};

pub(crate) use registry::Arity;

/// Maximum nesting depth of an expression (brackets, operator operands, call arguments).
pub(crate) const MAX_DEPTH: usize = 256;

/// Maximum number of arguments of one function call (the evaluator uses a fixed stack buffer).
pub(crate) const MAX_CALL_ARGS: usize = 8;

/// One step of a nested-access path (`_.a`, `_[0]`, `_["unit price"]`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PathSeg {
    /// A map key: `.name`, `["name"]` or `['name']`.
    Key(CompactString),
    /// A list index: `[n]` (may be negative; the resolver defines its meaning).
    Index(i64),
}

/// A variable occurrence: which variable, the nested-access path applied to its value, and
/// where it is in the source.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VarRef {
    /// Index into [`MathExpr::variables`]; equal names share a slot.
    pub(crate) slot: u16,
    /// Nested-access path, empty for a plain variable.
    pub(crate) path: Vec<PathSeg>,
    /// Byte offset of the variable name in the source.
    pub(crate) offset: usize,
    /// For a variable named like a constant (`pi`, `e`): the constant's value, to use when the
    /// name does not resolve to anything else.
    pub(crate) constant: Option<f64>,
}

/// One instruction of the compiled postfix program.
#[derive(Clone)]
pub(crate) enum Instr {
    /// Push a literal or a non-shadowable constant.
    Push(f64),
    /// Push the value of `MathExpr::refs[i]` as computed by the resolver.
    Var(usize),
    /// Pop one value, push the operator result.
    Unary(&'static UnaryOpDef),
    /// Pop two values, push the operator result; `offset` locates the operator.
    Binary {
        op: &'static BinaryOpDef,
        offset: usize,
    },
    /// Pop `argc` values, push the function result; `offset` locates the function name.
    Call {
        func: &'static FunctionDef,
        argc: usize,
        offset: usize,
    },
}

/// A parsed `math()` expression, compiled to a postfix program. Parse once, evaluate many times.
#[derive(Clone)]
pub(crate) struct MathExpr {
    /// The postfix program.
    program: Box<[Instr]>,
    /// Every variable occurrence, in source order.
    refs: Box<[VarRef]>,
    /// Distinct variable names in first-appearance order (slot order).
    names: Box<[CompactString]>,
    /// The deepest value stack the program needs.
    max_stack: usize,
}

impl MathExpr {
    /// Parses `src`. Errors carry the byte offset and the 1-based character column.
    pub(crate) fn parse(src: &str) -> Result<MathExpr, MathParseError> {
        parser::parse(src).map_err(|e| e.locate(src))
    }

    /// Distinct variable names (including `_`) in order of first appearance; drives the
    /// `by()` ring like TinkerPop's `MathStep.getVariables`.
    pub(crate) fn variables(&self) -> &[CompactString] {
        &self.names
    }

    /// Every variable occurrence in source order (one entry per occurrence, with its path).
    pub(crate) fn var_refs(&self) -> &[VarRef] {
        &self.refs
    }
}

/// A parse error before the column is computed (internal).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawParseError {
    /// Byte offset into the source.
    offset: usize,
    /// What went wrong.
    kind: MathParseErrorKind,
}

impl RawParseError {
    /// Creates an error at `offset`.
    pub(crate) fn new(offset: usize, kind: MathParseErrorKind) -> Self {
        RawParseError { offset, kind }
    }

    /// Adds the 1-based character column for `src`.
    fn locate(self, src: &str) -> MathParseError {
        let column = src
            .get(..self.offset)
            .map_or(self.offset, |s| s.chars().count())
            + 1;
        MathParseError {
            offset: self.offset,
            column,
            kind: self.kind,
        }
    }
}

/// Why a `math()` expression could not be parsed, and where.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{kind} at column {column}")]
pub(crate) struct MathParseError {
    /// Byte offset of the offending token.
    pub(crate) offset: usize,
    /// 1-based character column of the offending token.
    pub(crate) column: usize,
    /// What went wrong.
    pub(crate) kind: MathParseErrorKind,
}

/// The kinds of [`MathParseError`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub(crate) enum MathParseErrorKind {
    /// The expression is empty or blank.
    #[error("the expression is empty")]
    Empty,
    /// A character that is not part of the language.
    #[error("unexpected character '{0}'")]
    UnexpectedChar(char),
    /// A number literal that does not parse (`1.2.3`, `1e+`).
    #[error("invalid number '{0}'")]
    InvalidNumber(String),
    /// A token where something else was expected.
    #[error("unexpected {found}; expected {expected}")]
    Unexpected {
        /// Description of the token found.
        found: String,
        /// What the parser expected.
        expected: &'static str,
    },
    /// Two operands without an operator (implicit multiplication is off, as in TinkerPop).
    #[error(
        "missing operator before {found}; implicit multiplication is not supported, write '*' explicitly"
    )]
    MissingOperator {
        /// Description of the token found.
        found: String,
    },
    /// A name followed by an opening bracket that is not a function.
    #[error("unknown function '{name}'; available functions: {available}")]
    UnknownFunction {
        /// The name.
        name: String,
        /// Comma-separated list of the built-in functions.
        available: String,
    },
    /// A function called with the wrong number of arguments.
    #[error("function '{name}' takes {expected}, got {found}")]
    Arity {
        /// The function name.
        name: &'static str,
        /// What the function accepts.
        expected: Arity,
        /// How many arguments were given.
        found: usize,
    },
    /// A function name used without parentheses before a signed argument (`sin -1`).
    #[error("'{name}' without parentheses cannot take a signed argument; write {name}(-...)")]
    SignedArgumentWithoutParens {
        /// The function name.
        name: &'static str,
    },
    /// A function name without parentheses that needs more than one argument.
    #[error("function '{name}' takes {expected} and needs parentheses: {name}(a, b)")]
    CallNeedsParens {
        /// The function name.
        name: &'static str,
        /// What the function accepts.
        expected: Arity,
    },
    /// An opening bracket without its closer.
    #[error("unclosed '{0}'")]
    Unclosed(char),
    /// A closer that does not match the opener.
    #[error("'{close}' does not match '{open}'")]
    MismatchedBracket {
        /// The opener.
        open: char,
        /// The closer found.
        close: char,
    },
    /// A closer without an opener.
    #[error("unmatched '{0}'")]
    Unmatched(char),
    /// An invalid nested-access segment.
    #[error("invalid access path: {0}")]
    BadPath(&'static str),
    /// An index literal that is not an integer or does not fit an i64.
    #[error("invalid index '{0}'; an index is an integer such as [0]")]
    BadIndex(String),
    /// A quoted string without its closing quote.
    #[error("unterminated string")]
    UnterminatedString,
    /// An unsupported escape in a quoted string.
    #[error("invalid escape '\\{0}'; supported: \\\\, \\\", \\'")]
    InvalidEscape(char),
    /// Nesting beyond [`MAX_DEPTH`].
    #[error("the expression is nested deeper than {0} levels")]
    TooDeep(usize),
    /// More distinct variables than slots.
    #[error("too many distinct variables (at most {0})")]
    TooManyVariables(usize),
    /// More call arguments than [`MAX_CALL_ARGS`].
    #[error("too many function arguments (at most {0})")]
    TooManyArguments(usize),
}

/// A failure while evaluating a parsed expression, and where.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{kind} at offset {offset}")]
pub(crate) struct MathEvalError {
    /// Byte offset of the operator or function that failed.
    pub(crate) offset: usize,
    /// What went wrong.
    pub(crate) kind: MathEvalErrorKind,
}

/// The kinds of [`MathEvalError`]; the messages are exp4j's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum MathEvalErrorKind {
    /// `/` or `%` by zero.
    #[error("Division by zero!")]
    DivisionByZero,
    /// `cot(x)` where `tan(x) == 0`.
    #[error("Division by zero in cotangent!")]
    CotangentDivisionByZero,
    /// The compiled program is inconsistent (an engine bug, never a user error).
    #[error("internal error: malformed math program")]
    MalformedProgram,
}
