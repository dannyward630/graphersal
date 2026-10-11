//! Registry tables of the `math()` expression language: the single place to add a function,
//! operator or constant. The parser and evaluator look entries up by name/symbol and never
//! special-case a particular entry.

use super::MathEvalErrorKind;

/// How many arguments a [`FunctionDef`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arity {
    /// Exactly `n` arguments.
    Exact(usize),
    /// `n` or more arguments (none of the exp4j built-ins is variadic; kept for extension).
    #[allow(dead_code)] // no variadic built-in yet; the parser and tests handle it already
    AtLeast(usize),
}

impl Arity {
    /// Whether `n` arguments satisfy this arity.
    pub(crate) fn accepts(self, n: usize) -> bool {
        match self {
            Arity::Exact(k) => n == k,
            Arity::AtLeast(k) => n >= k,
        }
    }
}

impl std::fmt::Display for Arity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arity::Exact(1) => f.write_str("1 argument"),
            Arity::Exact(n) => write!(f, "{n} arguments"),
            Arity::AtLeast(n) => write!(f, "at least {n} arguments"),
        }
    }
}

/// A built-in function, e.g. `sqrt(x)`.
pub(crate) struct FunctionDef {
    /// The name as written in an expression.
    pub(crate) name: &'static str,
    /// Accepted argument count, checked at parse time.
    pub(crate) arity: Arity,
    /// The implementation; receives exactly the parsed arguments.
    pub(crate) eval: fn(&[f64]) -> Result<f64, MathEvalErrorKind>,
    /// One-line description, for diagnostics and docs.
    pub(crate) doc: &'static str,
}

/// Associativity of a binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Assoc {
    /// `a - b - c` = `(a - b) - c`.
    Left,
    /// `a ^ b ^ c` = `a ^ (b ^ c)`.
    Right,
}

/// A binary infix operator.
pub(crate) struct BinaryOpDef {
    /// The operator character.
    pub(crate) symbol: char,
    /// Binding strength (exp4j's numbers: higher binds tighter).
    pub(crate) precedence: u16,
    /// Grouping of a chain of equal-precedence operators.
    pub(crate) assoc: Assoc,
    /// The implementation.
    pub(crate) eval: fn(f64, f64) -> Result<f64, MathEvalErrorKind>,
    /// One-line description, for the reference in the book (`reference_rows`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) doc: &'static str,
}

/// A unary prefix operator (always right-associative, as in exp4j).
pub(crate) struct UnaryOpDef {
    /// The operator character.
    pub(crate) symbol: char,
    /// Binding strength of the operand: the operand absorbs binary operators of at least this
    /// precedence, so `-2^2` is `-(2^2)` and `-2*3` is `(-2)*3`.
    pub(crate) precedence: u16,
    /// The implementation.
    pub(crate) eval: fn(f64) -> f64,
    /// One-line description, for the reference in the book (`reference_rows`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) doc: &'static str,
}

/// A named constant.
pub(crate) struct ConstantDef {
    /// The name as written in an expression.
    pub(crate) name: &'static str,
    /// The value (exactly exp4j's).
    pub(crate) value: f64,
    /// One-line description, for the reference in the book (`reference_rows`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) doc: &'static str,
}

/// exp4j `Operator.PRECEDENCE_ADDITION`.
const PREC_ADD: u16 = 500;
/// exp4j `Operator.PRECEDENCE_MULTIPLICATION` (also division and modulo).
const PREC_MUL: u16 = 1000;
/// exp4j `Operator.PRECEDENCE_UNARY_MINUS` (also unary plus).
const PREC_UNARY: u16 = 5000;
/// exp4j `Operator.PRECEDENCE_POWER`.
const PREC_POW: u16 = 10000;

/// `java.lang.Math.pow` semantics on top of `f64::powf` (C `pow`). They differ in two cases:
/// Java returns NaN for a NaN exponent (C: `pow(1, NaN) = 1`) and for `|x| == 1` with an
/// infinite exponent (C: 1). Used by both `^` and `pow(x, y)`.
pub(crate) fn java_pow(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        1.0
    } else if y.is_nan() || (x.abs() == 1.0 && y.is_infinite()) {
        f64::NAN
    } else {
        x.powf(y)
    }
}

/// exp4j's division and modulo throw `ArithmeticException("Division by zero!")` when the divisor
/// is `0.0` (or `-0.0`); a NaN divisor yields NaN.
fn checked_divisor(b: f64) -> Result<(), MathEvalErrorKind> {
    if b == 0.0 {
        Err(MathEvalErrorKind::DivisionByZero)
    } else {
        Ok(())
    }
}

/// Binary operators (exp4j `Operators`).
pub(crate) static BINARY_OPS: &[BinaryOpDef] = &[
    BinaryOpDef {
        symbol: '+',
        precedence: PREC_ADD,
        assoc: Assoc::Left,
        eval: |a, b| Ok(a + b),
        doc: "addition",
    },
    BinaryOpDef {
        symbol: '-',
        precedence: PREC_ADD,
        assoc: Assoc::Left,
        eval: |a, b| Ok(a - b),
        doc: "subtraction",
    },
    BinaryOpDef {
        symbol: '*',
        precedence: PREC_MUL,
        assoc: Assoc::Left,
        eval: |a, b| Ok(a * b),
        doc: "multiplication",
    },
    BinaryOpDef {
        symbol: '/',
        precedence: PREC_MUL,
        assoc: Assoc::Left,
        eval: |a, b| checked_divisor(b).map(|()| a / b),
        doc: "division; a zero divisor is an error",
    },
    BinaryOpDef {
        symbol: '%',
        precedence: PREC_MUL,
        assoc: Assoc::Left,
        eval: |a, b| checked_divisor(b).map(|()| a % b),
        doc: "remainder with the sign of the dividend (Java `%`); a zero divisor is an error",
    },
    BinaryOpDef {
        symbol: '^',
        precedence: PREC_POW,
        assoc: Assoc::Right,
        eval: |a, b| Ok(java_pow(a, b)),
        doc: "power (right-associative)",
    },
];

/// Unary prefix operators (exp4j `Operators`).
pub(crate) static UNARY_OPS: &[UnaryOpDef] = &[
    UnaryOpDef {
        symbol: '-',
        precedence: PREC_UNARY,
        eval: |a| -a,
        doc: "negation",
    },
    UnaryOpDef {
        symbol: '+',
        precedence: PREC_UNARY,
        eval: |a| a,
        doc: "identity",
    },
];

/// Constants (exp4j `Expression.createDefaultVariables`). `φ` is exp4j's truncated literal.
#[allow(clippy::excessive_precision)]
pub(crate) static CONSTANTS: &[ConstantDef] = &[
    ConstantDef {
        name: "pi",
        value: std::f64::consts::PI,
        doc: "π",
    },
    ConstantDef {
        name: "π",
        value: std::f64::consts::PI,
        doc: "π",
    },
    ConstantDef {
        name: "e",
        value: std::f64::consts::E,
        doc: "Euler's number",
    },
    ConstantDef {
        name: "φ",
        value: 1.61803398874,
        doc: "golden ratio (exp4j's 12-digit value)",
    },
];

/// exp4j `signum`: 1, -1, or 0 for anything else (including NaN and -0.0).
fn signum(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// exp4j `cot`: `1 / tan(x)`, an error when `tan(x) == 0`.
fn cot(x: f64) -> Result<f64, MathEvalErrorKind> {
    let t = x.tan();
    if t == 0.0 {
        Err(MathEvalErrorKind::CotangentDivisionByZero)
    } else {
        Ok(1.0 / t)
    }
}

/// Defines a one-argument function row whose body is an `f64 -> f64` expression.
macro_rules! unary_fn {
    ($name:literal, $f:expr_2021, $doc:literal) => {
        FunctionDef {
            name: $name,
            arity: Arity::Exact(1),
            eval: |a| Ok(($f)(arg(a, 0))),
            doc: $doc,
        }
    };
}

/// The `i`-th argument; the parser guarantees the arity, NaN is a defensive fallback.
fn arg(args: &[f64], i: usize) -> f64 {
    args.get(i).copied().unwrap_or(f64::NAN)
}

/// Functions: exactly the exp4j 0.4.8 built-in set (`Functions.getBuiltinFunction`).
pub(crate) static FUNCTIONS: &[FunctionDef] = &[
    unary_fn!("abs", f64::abs, "absolute value"),
    unary_fn!("acos", f64::acos, "arc cosine (radians)"),
    unary_fn!("asin", f64::asin, "arc sine (radians)"),
    unary_fn!("atan", f64::atan, "arc tangent (radians)"),
    unary_fn!("cbrt", f64::cbrt, "cube root"),
    unary_fn!("ceil", f64::ceil, "round up to an integer"),
    unary_fn!("cos", f64::cos, "cosine (radians)"),
    unary_fn!("cosh", f64::cosh, "hyperbolic cosine"),
    FunctionDef {
        name: "cot",
        arity: Arity::Exact(1),
        eval: |a| cot(arg(a, 0)),
        doc: "cotangent; an error where tan(x) = 0",
    },
    unary_fn!("exp", f64::exp, "e raised to x"),
    unary_fn!("expm1", f64::exp_m1, "exp(x) - 1"),
    unary_fn!("floor", f64::floor, "round down to an integer"),
    unary_fn!("log", f64::ln, "natural logarithm"),
    unary_fn!("log10", f64::log10, "base-10 logarithm"),
    unary_fn!("log1p", f64::ln_1p, "log(1 + x)"),
    // exp4j computes `Math.log(x) / Math.log(2)`, not a dedicated log2.
    unary_fn!("log2", |x: f64| x.ln() / 2f64.ln(), "base-2 logarithm"),
    FunctionDef {
        name: "pow",
        arity: Arity::Exact(2),
        eval: |a| Ok(java_pow(arg(a, 0), arg(a, 1))),
        doc: "x raised to y",
    },
    unary_fn!("signum", signum, "sign: -1, 0 or 1"),
    unary_fn!("sin", f64::sin, "sine (radians)"),
    unary_fn!("sinh", f64::sinh, "hyperbolic sine"),
    unary_fn!("sqrt", f64::sqrt, "square root"),
    unary_fn!("tan", f64::tan, "tangent (radians)"),
    unary_fn!("tanh", f64::tanh, "hyperbolic tangent"),
];

/// Looks a function up by name.
pub(crate) fn function(name: &str) -> Option<&'static FunctionDef> {
    FUNCTIONS.iter().find(|f| f.name == name)
}

/// Looks a binary operator up by symbol.
pub(crate) fn binary_op(symbol: char) -> Option<&'static BinaryOpDef> {
    BINARY_OPS.iter().find(|o| o.symbol == symbol)
}

/// Looks a unary operator up by symbol.
pub(crate) fn unary_op(symbol: char) -> Option<&'static UnaryOpDef> {
    UNARY_OPS.iter().find(|o| o.symbol == symbol)
}

/// Looks a constant up by name.
pub(crate) fn constant(name: &str) -> Option<&'static ConstantDef> {
    CONSTANTS.iter().find(|c| c.name == name)
}

/// The language reference as Markdown table rows, generated from the tables above. The book page
/// `book/src/users_guide/math.md` must contain every row (checked by a unit test), so extending
/// a table forces a doc update.
#[cfg(test)]
pub(crate) fn reference_rows() -> Vec<String> {
    let mut rows = Vec::new();
    for op in BINARY_OPS {
        let assoc = match op.assoc {
            Assoc::Left => "left",
            Assoc::Right => "right",
        };
        rows.push(format!(
            "| `a {} b` | {} | {assoc} | {} |",
            op.symbol, op.precedence, op.doc
        ));
    }
    for op in UNARY_OPS {
        rows.push(format!(
            "| `{}a` | {} | right | {} |",
            op.symbol, op.precedence, op.doc
        ));
    }
    for f in FUNCTIONS {
        let args = match f.arity {
            Arity::Exact(1) => "x".to_string(),
            Arity::Exact(2) => "x, y".to_string(),
            Arity::Exact(n) => format!("{n} arguments"),
            Arity::AtLeast(n) => format!("x1, ..., x{n}, ..."),
        };
        rows.push(format!("| `{}({args})` | {} |", f.name, f.doc));
    }
    for c in CONSTANTS {
        rows.push(format!("| `{}` | {} | {} |", c.name, c.value, c.doc));
    }
    rows
}
