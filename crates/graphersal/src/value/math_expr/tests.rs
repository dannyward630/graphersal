//! Functional tests of the `math()` expression engine: table-driven, no graph, no Rhai.

use super::registry::{BINARY_OPS, CONSTANTS, FUNCTIONS, UNARY_OPS};
use super::*;
use std::f64::consts::{E, FRAC_PI_2, FRAC_PI_4, PI};

/// Error type of the test resolver.
#[derive(Debug, PartialEq)]
enum TestErr {
    /// An evaluation error from the engine.
    Eval(MathEvalError),
    /// A variable with no binding and no constant fallback.
    Unbound(String),
}

impl From<MathEvalError> for TestErr {
    fn from(e: MathEvalError) -> Self {
        TestErr::Eval(e)
    }
}

/// Bindings used by every value case.
const BINDINGS: &[(&str, f64)] = &[
    ("_", 10.0),
    ("x", 2.0),
    ("y", 3.0),
    ("a", 4.0),
    ("sinx", 7.0),
    ("sinh2", 8.0),
    ("log2x", 9.0),
    ("_1", 11.0),
];

/// Evaluates `expr` against `bindings`; a variable named like a constant falls back to it.
fn eval_with(expr: &MathExpr, bindings: &[(&str, f64)]) -> Result<f64, TestErr> {
    let names = expr.variables();
    expr.eval(|var: &VarRef| {
        let name = names
            .get(usize::from(var.slot))
            .map(|n| n.as_str())
            .unwrap_or("?");
        bindings
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
            .or(var.constant)
            .ok_or_else(|| TestErr::Unbound(name.to_string()))
    })
}

/// Parses and evaluates against [`BINDINGS`].
fn eval(src: &str) -> Result<f64, TestErr> {
    let expr = MathExpr::parse(src).unwrap_or_else(|e| panic!("{src:?} failed to parse: {e}"));
    eval_with(&expr, BINDINGS)
}

/// Equality for f64 results: both NaN, equal infinities/zeros, or within 1e-12 relative.
fn same(actual: f64, expected: f64) -> bool {
    if expected.is_nan() {
        return actual.is_nan();
    }
    if expected.is_infinite() || expected == 0.0 {
        return actual == expected;
    }
    ((actual - expected) / expected).abs() < 1e-12
}

/// Runs a table of `(expression, expected value)` cases, reporting all mismatches at once.
fn check_values(cases: &[(&str, f64)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(src, expected)| match eval(src) {
            Ok(v) if same(v, *expected) => None,
            other => Some(format!("{src:?}: expected {expected}, got {other:?}")),
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} failing cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Literals and whitespace.
const LITERALS: &[(&str, f64)] = &[
    ("12", 12.0),
    ("0", 0.0),
    ("007", 7.0),
    ("1.5", 1.5),
    (".5", 0.5),
    ("5.", 5.0),
    ("1e3", 1000.0),
    ("1E-3", 0.001),
    ("2.5e+2", 250.0),
    ("2.e3", 2000.0),
    ("1.2E3", 1200.0),
    ("  42  ", 42.0),
    ("\t1\n+\r2 ", 3.0),
    ("1+2", 3.0),
    ("1e3-1", 999.0),
    ("1e-3-1", -0.999),
];

/// Operators, precedence and associativity (exp4j: + - 500 < * / % 1000 < unary 5000 < ^ 10000).
const OPERATORS: &[(&str, f64)] = &[
    ("2+3", 5.0),
    ("2-3", -1.0),
    ("2*3", 6.0),
    ("7/2", 3.5),
    ("29/2", 14.5),
    ("7%3", 1.0),
    ("-7%3", -1.0),
    ("7%-3", 1.0),
    ("7.5%2", 1.5),
    ("2^3", 8.0),
    ("2^0.5", std::f64::consts::SQRT_2),
    ("-2", -2.0),
    ("+2", 2.0),
    ("-2^2", -4.0),
    ("(-2)^2", 4.0),
    ("2^3^2", 512.0),
    ("(2^3)^2", 64.0),
    ("2^-1", 0.5),
    ("2^-2^2", 0.0625),
    ("2^-2*3", 0.75),
    ("2*3+4", 10.0),
    ("2+3*4", 14.0),
    ("(2+3)*4", 20.0),
    ("10-4-3", 3.0),
    ("100/10/5", 2.0),
    ("2*-3", -6.0),
    ("2*-3+1", -5.0),
    ("-2*3", -6.0),
    ("--2", 2.0),
    ("+-2", -2.0),
    ("-+2", -2.0),
    ("2--2", 4.0),
    ("2+-2", 0.0),
    ("2 - - - 2", 0.0),
    ("-(1+2)", -3.0),
    ("8/2*4", 16.0),
    ("8%5*2", 6.0),
    ("1/(1/0.0001)", 0.0001),
    ("[1+2]*3", 9.0),
    ("{1+2}*3", 9.0),
    ("({[1]})", 1.0),
    ("1/sqrt(0)^-1", 0.0),
];

/// Every function in `FUNCTIONS`, with domain edges as `java.lang.Math` returns them.
/// `sqrt(-1)` is NaN, `exp(1000)` is +inf.
const FUNCTION_CASES: &[(&str, f64)] = &[
    ("abs(-2.5)", 2.5),
    ("abs(3)", 3.0),
    ("abs(-exp(1000))", f64::INFINITY),
    ("acos(1)", 0.0),
    ("acos(2)", f64::NAN),
    ("asin(1)", FRAC_PI_2),
    ("asin(-2)", f64::NAN),
    ("atan(1)", FRAC_PI_4),
    ("atan(exp(1000))", FRAC_PI_2),
    ("cbrt(27)", 3.0),
    ("cbrt(-8)", -2.0),
    ("ceil(1.2)", 2.0),
    ("ceil(-1.2)", -1.0),
    ("cos(0)", 1.0),
    ("cos(pi)", -1.0),
    ("cosh(0)", 1.0),
    ("cot(1)", 0.642_092_615_934_330_7),
    ("cot(pi/4)", 1.0),
    ("exp(0)", 1.0),
    ("exp(1)", E),
    ("exp(1000)", f64::INFINITY),
    ("expm1(0)", 0.0),
    ("expm1(1)", E - 1.0),
    ("floor(-1.5)", -2.0),
    ("floor(1.5)", 1.0),
    ("log(e)", 1.0),
    ("log(1)", 0.0),
    ("log(0)", f64::NEG_INFINITY),
    ("log(-1)", f64::NAN),
    ("log10(1000)", 3.0),
    ("log10(0)", f64::NEG_INFINITY),
    ("log1p(0)", 0.0),
    ("log1p(e-1)", 1.0),
    ("log2(8)", 3.0),
    ("log2(1024)", 10.0),
    ("log2(-1)", f64::NAN),
    ("pow(2, 10)", 1024.0),
    ("pow(2,0.5)", std::f64::consts::SQRT_2),
    ("pow(-8, 1/3)", f64::NAN),
    ("pow(5, 0)", 1.0),
    ("pow(sqrt(-1), 0)", 1.0),
    ("pow(1, sqrt(-1))", f64::NAN),
    ("pow(-1, exp(1000))", f64::NAN),
    ("1^sqrt(-1)", f64::NAN),
    ("signum(-3)", -1.0),
    ("signum(0.1)", 1.0),
    ("signum(0)", 0.0),
    ("signum(sqrt(-1))", 0.0),
    ("sin(0)", 0.0),
    ("sin(pi/2)", 1.0),
    ("sinh(0)", 0.0),
    ("sinh(1)", 1.175_201_193_643_801_4),
    ("sqrt(16)", 4.0),
    ("sqrt(2)", std::f64::consts::SQRT_2),
    ("sqrt(-1)", f64::NAN),
    ("tan(0)", 0.0),
    ("tan(pi/4)", 1.0),
    ("tanh(0)", 0.0),
    ("tanh(exp(1000))", 1.0),
    ("sqrt(16) + 1", 5.0),
    ("pow(2, 3) ^ 2", 64.0),
    ("pow(pow(2, 2), pow(1+1, 1))", 16.0),
    ("sqrt[16]", 4.0),
    ("sqrt{16}", 4.0),
    ("sqrt ( 16 )", 4.0),
    ("-sqrt(4)^2", -4.0),
];

/// exp4j's paren-less call: the function applies to the rest of the enclosing group.
const PARENLESS_CASES: &[(&str, f64)] = &[
    ("sin _", -0.544_021_110_889_369_8),
    ("sin _ + 1", -0.999_990_206_550_703_5),
    ("2 * sin _ + 1", -1.999_980_413_101_407),
    ("(sin _ + 1) * 2", -1.999_980_413_101_407),
    ("sqrt 16", 4.0),
    ("sqrt _ * 10 - 64", 6.0),
    ("abs _ - 20", 10.0),
    ("sin cos 0", 0.841_470_984_807_896_5),
    ("1 + sqrt 4 ^ 2 / 2", 3.828_427_124_746_190_3),
];

/// Constants; `pi`/`e` are unbound here, so they fall back to the constant value.
const CONSTANT_CASES: &[(&str, f64)] = &[
    ("pi", PI),
    ("π", PI),
    ("e", E),
    ("φ", 1.61803398874),
    ("2*pi", 2.0 * PI),
    ("π*2", 2.0 * PI),
    ("e^2", E * E),
    ("φ-1", 0.61803398874),
    ("2e3*e", 2000.0 * E),
];

/// Variables (see [`BINDINGS`]): names next to function names, repeated names.
const VARIABLE_CASES: &[(&str, f64)] = &[
    ("_", 10.0),
    ("_ * 2", 20.0),
    ("_ / 4", 2.5),
    ("x + y", 5.0),
    ("x ^ y", 8.0),
    ("_ + _ + x", 22.0),
    ("sinx", 7.0),
    ("sin(x)", 0.909_297_426_825_681_7),
    ("sinh2", 8.0),
    ("log2x", 9.0),
    ("_1", 11.0),
    ("-_", -10.0),
    ("pow(x, y)", 8.0),
    ("sqrt(a) * x", 4.0),
];

#[test]
fn literals() {
    check_values(LITERALS);
}

#[test]
fn operators_precedence_associativity() {
    check_values(OPERATORS);
}

#[test]
fn functions() {
    check_values(FUNCTION_CASES);
}

#[test]
fn parenless_function_calls() {
    check_values(PARENLESS_CASES);
}

#[test]
fn constants() {
    check_values(CONSTANT_CASES);
}

#[test]
fn variables() {
    check_values(VARIABLE_CASES);
}

#[test]
fn result_is_always_f64() {
    // TinkerPop returns a Double even for integer-looking operands: 29 / 2 = 14.5, not 14.
    assert_eq!(eval("29 / 2"), Ok(14.5));
    assert_eq!(eval("_ + 51"), Ok(61.0));
}

#[test]
fn log2_matches_exp4j_formula() {
    for x in [3.0_f64, 8.0, 10.0, 1e-300, 12345.678] {
        let got = eval(&format!("log2({x})")).unwrap_or(f64::NAN);
        assert_eq!(got.to_bits(), (x.ln() / 2f64.ln()).to_bits(), "log2({x})");
    }
}

#[test]
fn signum_of_negative_zero_is_positive_zero() {
    assert_eq!(eval("signum(-0)").map(f64::to_bits), Ok(0.0_f64.to_bits()));
}

/// Parses or fails the test with the parse error's message.
fn parse(src: &str) -> MathExpr {
    MathExpr::parse(src).unwrap_or_else(|e| panic!("{src:?} failed to parse: {e}"))
}

#[test]
fn variables_are_distinct_in_first_appearance_order() {
    let expr = parse("x + _ * x + y - _");
    assert_eq!(expr.variables(), ["x", "_", "y"]);
    let slots: Vec<u16> = expr.var_refs().iter().map(|r| r.slot).collect();
    assert_eq!(slots, [0, 1, 0, 2, 1]);
    let offsets: Vec<usize> = expr.var_refs().iter().map(|r| r.offset).collect();
    assert_eq!(offsets, [0, 4, 8, 12, 16]);
}

#[test]
fn functions_and_non_ascii_constants_are_not_variables() {
    assert!(parse("sin(1) + π + φ + 2").variables().is_empty());
    assert_eq!(parse("sinx + sin(x)").variables(), ["sinx", "x"]);
}

#[test]
fn ascii_constants_are_variables_with_a_constant_fallback() {
    // TinkerPop's VARIABLE_PATTERN declares `pi`/`e` as step variables (scope keys win).
    let expr = parse("e * pi + _");
    assert_eq!(expr.variables(), ["e", "pi", "_"]);
    let fallbacks: Vec<Option<f64>> = expr.var_refs().iter().map(|r| r.constant).collect();
    assert_eq!(fallbacks, [Some(E), Some(PI), None]);
    // Bound: the binding shadows the constant.
    assert_eq!(
        eval_with(&expr, &[("e", 2.0), ("pi", 3.0), ("_", 1.0)]),
        Ok(7.0)
    );
    // Unbound: the constant is used.
    assert_eq!(eval_with(&expr, &[("_", 1.0)]), Ok(E * PI + 1.0));
}

#[test]
fn unbound_variable_is_reported_by_the_resolver() {
    assert_eq!(
        eval_with(&parse("1 + zz"), &[]),
        Err(TestErr::Unbound("zz".into()))
    );
}

#[test]
fn resolver_is_called_once_per_occurrence_in_source_order() {
    let expr = parse("y + x * y");
    let mut seen = Vec::new();
    let r: Result<f64, TestErr> = expr.eval(|v: &VarRef| {
        seen.push(v.slot);
        Ok(f64::from(v.slot) + 2.0)
    });
    assert_eq!(r, Ok(2.0 + 3.0 * 2.0));
    assert_eq!(seen, [0, 1, 0]);
}

#[test]
fn expression_is_reusable() {
    let expr = parse("_ * 2 + 1");
    for v in [0.0, 1.5, -3.0] {
        assert_eq!(eval_with(&expr, &[("_", v)]), Ok(v * 2.0 + 1.0));
    }
}

/// Key segment shorthand.
fn k(s: &str) -> PathSeg {
    PathSeg::Key(s.into())
}

/// Index segment shorthand.
fn i(n: i64) -> PathSeg {
    PathSeg::Index(n)
}

/// A variable name with its access path.
type NamedPath<'a> = (&'a str, Vec<PathSeg>);

#[test]
fn nested_access_paths() {
    let cases: Vec<(&str, Vec<NamedPath>)> = vec![
        ("_.a", vec![("_", vec![k("a")])]),
        ("_.a.b.c", vec![("_", vec![k("a"), k("b"), k("c")])]),
        ("_[0]", vec![("_", vec![i(0)])]),
        ("_[ 12 ]", vec![("_", vec![i(12)])]),
        ("_[-1]", vec![("_", vec![i(-1)])]),
        (
            "_.items[2].price",
            vec![("_", vec![k("items"), i(2), k("price")])],
        ),
        ("_[\"unit price\"]", vec![("_", vec![k("unit price")])]),
        ("_['k']", vec![("_", vec![k("k")])]),
        (
            "_['it\\'s'][\"q\\\"\\\\\"]",
            vec![("_", vec![k("it's"), k("q\"\\")])],
        ),
        ("_.sin", vec![("_", vec![k("sin")])]),
        ("_._1", vec![("_", vec![k("_1")])]),
        (
            "a.b + c.d[1]",
            vec![("a", vec![k("b")]), ("c", vec![k("d"), i(1)])],
        ),
        ("a.b * a.c", vec![("a", vec![k("b")]), ("a", vec![k("c")])]),
        ("pi.x", vec![("pi", vec![k("x")])]),
        ("sqrt(_.v[0])", vec![("_", vec![k("v"), i(0)])]),
        ("_.a^2", vec![("_", vec![k("a")])]),
        ("(_.a)", vec![("_", vec![k("a")])]),
    ];
    for (src, expected) in cases {
        let expr = parse(src);
        let got: Vec<NamedPath> = expr
            .var_refs()
            .iter()
            .map(|r| {
                (
                    expr.variables()[usize::from(r.slot)].as_str(),
                    r.path.clone(),
                )
            })
            .collect();
        assert_eq!(got, expected, "{src:?}");
    }
    // Same base name with different paths shares one slot (one by() ring entry).
    assert_eq!(parse("a.b * a.c + a").variables(), ["a"]);
}

#[test]
fn nested_access_values_come_from_the_resolver() {
    let expr = parse("_.price.total * 2 + _.items[1]");
    let r: Result<f64, TestErr> = expr.eval(|v: &VarRef| match v.path.as_slice() {
        [PathSeg::Key(a), PathSeg::Key(b)] if a == "price" && b == "total" => Ok(5.0),
        [PathSeg::Key(a), PathSeg::Index(1)] if a == "items" => Ok(0.5),
        _ => Err(TestErr::Unbound(format!("{:?}", v.path))),
    });
    assert_eq!(r, Ok(10.5));
}

/// Short name of a parse-error kind, for the error table.
fn kind_name(k: &MathParseErrorKind) -> &'static str {
    use MathParseErrorKind as K;
    match k {
        K::Empty => "Empty",
        K::UnexpectedChar(_) => "UnexpectedChar",
        K::InvalidNumber(_) => "InvalidNumber",
        K::Unexpected { .. } => "Unexpected",
        K::MissingOperator { .. } => "MissingOperator",
        K::UnknownFunction { .. } => "UnknownFunction",
        K::Arity { .. } => "Arity",
        K::SignedArgumentWithoutParens { .. } => "SignedArgumentWithoutParens",
        K::CallNeedsParens { .. } => "CallNeedsParens",
        K::Unclosed(_) => "Unclosed",
        K::MismatchedBracket { .. } => "MismatchedBracket",
        K::Unmatched(_) => "Unmatched",
        K::BadPath(_) => "BadPath",
        K::BadIndex(_) => "BadIndex",
        K::UnterminatedString => "UnterminatedString",
        K::InvalidEscape(_) => "InvalidEscape",
        K::TooDeep(_) => "TooDeep",
        K::TooManyVariables(_) => "TooManyVariables",
        K::TooManyArguments(_) => "TooManyArguments",
    }
}

/// `(expression, byte offset, error kind)`.
const PARSE_ERRORS: &[(&str, usize, &str)] = &[
    ("", 0, "Empty"),
    ("   ", 0, "Empty"),
    ("1 +", 3, "Unexpected"),
    ("1 + * 2", 4, "Unexpected"),
    ("* 2", 0, "Unexpected"),
    ("(", 1, "Unexpected"),
    ("()", 1, "Unexpected"),
    (",", 0, "Unexpected"),
    ("(1, 2)", 2, "Unexpected"),
    ("1 , 2", 2, "Unexpected"),
    (".x", 0, "Unexpected"),
    ("π.x", 2, "Unexpected"),
    ("(1 + 2", 0, "Unclosed"),
    ("sin(1", 3, "Unclosed"),
    ("1 + 2)", 5, "Unmatched"),
    ("]", 0, "Unexpected"),
    ("(1]", 2, "MismatchedBracket"),
    ("{1)", 2, "MismatchedBracket"),
    ("foo(1)", 0, "UnknownFunction"),
    ("x (1)", 0, "UnknownFunction"),
    ("1 # 2", 2, "UnexpectedChar"),
    ("$", 0, "UnexpectedChar"),
    ("1 & 2", 2, "UnexpectedChar"),
    ("ä", 0, "UnexpectedChar"),
    ("1 2", 2, "MissingOperator"),
    ("x y", 2, "MissingOperator"),
    ("2x", 1, "MissingOperator"),
    ("2 pi", 2, "MissingOperator"),
    ("2(3)", 1, "MissingOperator"),
    ("(2)(3)", 3, "MissingOperator"),
    ("2e", 1, "MissingOperator"),
    ("2 sin(1)", 2, "MissingOperator"),
    ("(1 2)", 3, "MissingOperator"),
    ("1.2.3", 0, "InvalidNumber"),
    ("1e+", 0, "InvalidNumber"),
    ("1ee3", 0, "InvalidNumber"),
    ("2 + 3e-", 4, "InvalidNumber"),
    ("sin(1, 2)", 0, "Arity"),
    ("pow(2)", 0, "Arity"),
    ("sin()", 0, "Arity"),
    ("pow 2", 0, "CallNeedsParens"),
    ("sin -1", 4, "SignedArgumentWithoutParens"),
    ("sin +1", 4, "SignedArgumentWithoutParens"),
    ("sin", 3, "Unexpected"),
    ("sin )", 4, "Unexpected"),
    ("1 + 2) * (3", 5, "Unmatched"),
    ("_[", 2, "BadPath"),
    ("_[x]", 2, "BadPath"),
    ("_[]", 2, "BadPath"),
    ("_.", 2, "BadPath"),
    ("_. a", 3, "BadPath"),
    ("_.5", 1, "BadPath"),
    ("_.[0]", 2, "BadPath"),
    ("_[0 + 1]", 4, "BadPath"),
    ("_ [0]", 2, "BadPath"),
    ("_[0", 1, "Unclosed"),
    ("_[1.5]", 2, "BadIndex"),
    ("_[1e3]", 2, "BadIndex"),
    ("_[99999999999999999999]", 2, "BadIndex"),
    ("_['a", 2, "UnterminatedString"),
    ("_[\"a]", 2, "UnterminatedString"),
    ("_['\\n']", 3, "InvalidEscape"),
    ("'a'", 0, "Unexpected"),
    ("1 + 'a'", 4, "Unexpected"),
];

#[test]
fn parse_errors_have_kind_and_offset() {
    let failures: Vec<String> = PARSE_ERRORS
        .iter()
        .filter_map(|(src, offset, kind)| match MathExpr::parse(src) {
            Err(e) if e.offset == *offset && kind_name(&e.kind) == *kind => None,
            Err(e) => Some(format!(
                "{src:?}: expected {kind}@{offset}, got {}@{} ({e})",
                kind_name(&e.kind),
                e.offset
            )),
            Ok(_) => Some(format!("{src:?}: expected {kind}@{offset}, parsed fine")),
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} failing cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// `(expression, byte offset of the failing operator/function, kind)`.
const EVAL_ERRORS: &[(&str, usize, MathEvalErrorKind)] = &[
    ("1/0", 1, MathEvalErrorKind::DivisionByZero),
    ("1 / -0", 2, MathEvalErrorKind::DivisionByZero),
    ("_ / (x - 2)", 2, MathEvalErrorKind::DivisionByZero),
    ("0/0", 1, MathEvalErrorKind::DivisionByZero),
    ("1 % 0", 2, MathEvalErrorKind::DivisionByZero),
    ("2 + _ % (y - 3) * 4", 6, MathEvalErrorKind::DivisionByZero),
    ("cot(0)", 0, MathEvalErrorKind::CotangentDivisionByZero),
    (
        "1 + cot(_ - 10)",
        4,
        MathEvalErrorKind::CotangentDivisionByZero,
    ),
];

#[test]
fn eval_errors_have_kind_and_offset() {
    for (src, offset, kind) in EVAL_ERRORS {
        assert_eq!(
            eval(src),
            Err(TestErr::Eval(MathEvalError {
                offset: *offset,
                kind: *kind
            })),
            "{src:?}"
        );
    }
}

#[test]
fn division_by_nan_or_infinity_is_not_an_error() {
    check_values(&[
        ("1/sqrt(-1)", f64::NAN),
        ("1/exp(1000)", 0.0),
        ("5 % exp(1000)", 5.0),
        ("1 % sqrt(-1)", f64::NAN),
    ]);
}

#[test]
fn deep_nesting_is_a_clean_error() {
    for (open, close) in [
        ("(", ")"),
        ("-", ""),
        ("sin(", ")"),
        ("2^", ""),
        ("sqrt ", ""),
    ] {
        let src = format!("{}1{}", open.repeat(10_000), close.repeat(10_000));
        match MathExpr::parse(&src) {
            Err(e) => assert_eq!(e.kind, MathParseErrorKind::TooDeep(MAX_DEPTH), "{open}"),
            Ok(_) => panic!("{open}: 10 000 levels must not parse"),
        }
    }
    // Just below the cap is fine.
    let ok = format!(
        "{}1{}",
        "(".repeat(MAX_DEPTH - 1),
        ")".repeat(MAX_DEPTH - 1)
    );
    assert_eq!(eval(&ok), Ok(1.0));
}

#[test]
fn long_flat_chains_are_not_nesting() {
    let sum = vec!["1"; 10_000].join(" + ");
    assert_eq!(eval(&sum), Ok(10_000.0));
    let product = vec!["_"; 400].join("*");
    assert_eq!(eval(&product), Ok(f64::INFINITY));
}

#[test]
fn wide_programs_use_the_heap_stack() {
    // Each nested pow keeps one pending argument on the value stack: 40 levels > inline size.
    let src = format!("{}2{}", "pow(1, ".repeat(40), ")".repeat(40));
    assert_eq!(eval(&src), Ok(1.0));
    let src = format!("{}1{}", "1 + (".repeat(100), ")".repeat(100));
    assert_eq!(eval(&src), Ok(101.0));
}

#[test]
fn error_column_counts_characters_not_bytes() {
    let e = MathExpr::parse("π + φ + #").err();
    assert_eq!(e.as_ref().map(|e| (e.offset, e.column)), Some((10, 9)));
}

#[test]
fn error_messages_are_human_readable() {
    let msg = |src: &str| {
        MathExpr::parse(src)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    };
    assert_eq!(
        msg("1 +"),
        "unexpected end of expression; expected a number, a variable, a function or '(' at column 4"
    );
    assert_eq!(
        msg("1 2"),
        "missing operator before number '2'; implicit multiplication is not supported, write '*' explicitly at column 3"
    );
    assert_eq!(
        msg("pow(2)"),
        "function 'pow' takes 2 arguments, got 1 at column 1"
    );
    assert_eq!(
        msg("sin(1, 2)"),
        "function 'sin' takes 1 argument, got 2 at column 1"
    );
    assert!(msg("foo(1)").starts_with("unknown function 'foo'; available functions: abs, acos,"));
    assert_eq!(
        msg("_[x]"),
        "invalid access path: expected an integer index or a quoted key inside [...] at column 3"
    );
    assert_eq!(
        MathEvalErrorKind::DivisionByZero.to_string(),
        "Division by zero!"
    );
    assert_eq!(
        MathEvalErrorKind::CotangentDivisionByZero.to_string(),
        "Division by zero in cotangent!"
    );
}

/// Every value table, for the extensibility guard.
const ALL_VALUE_TABLES: &[&[(&str, f64)]] = &[
    LITERALS,
    OPERATORS,
    FUNCTION_CASES,
    PARENLESS_CASES,
    CONSTANT_CASES,
    VARIABLE_CASES,
];

#[test]
fn every_registry_entry_has_a_functional_case() {
    let mut funcs = Vec::new();
    let mut binary = Vec::new();
    let mut unary = Vec::new();
    let mut constant_names = Vec::new();
    for (src, _) in ALL_VALUE_TABLES.iter().flat_map(|t| t.iter()) {
        let expr = parse(src);
        for instr in expr.program.iter() {
            match instr {
                Instr::Call { func, .. } => funcs.push(func.name),
                Instr::Binary { op, .. } => binary.push(op.symbol),
                Instr::Unary(op) => unary.push(op.symbol),
                Instr::Push(_) | Instr::Var(_) => {}
            }
        }
        for r in expr.var_refs().iter().filter(|r| r.constant.is_some()) {
            constant_names.push(expr.variables()[usize::from(r.slot)].to_string());
        }
        constant_names.extend(
            CONSTANTS
                .iter()
                .filter(|c| !c.name.is_ascii() && src.contains(c.name))
                .map(|c| c.name.to_string()),
        );
    }
    let missing: Vec<String> = FUNCTIONS
        .iter()
        .filter(|f| !funcs.contains(&f.name))
        .map(|f| format!("function {}", f.name))
        .chain(
            BINARY_OPS
                .iter()
                .filter(|o| !binary.contains(&o.symbol))
                .map(|o| format!("binary {}", o.symbol)),
        )
        .chain(
            UNARY_OPS
                .iter()
                .filter(|o| !unary.contains(&o.symbol))
                .map(|o| format!("unary {}", o.symbol)),
        )
        .chain(
            CONSTANTS
                .iter()
                .filter(|c| !constant_names.iter().any(|n| n == c.name))
                .map(|c| format!("constant {}", c.name)),
        )
        .collect();
    assert!(
        missing.is_empty(),
        "registry entries without a functional test case: {missing:?}"
    );
}

#[test]
fn registry_names_are_unique_and_well_formed() {
    let mut names: Vec<&str> = FUNCTIONS
        .iter()
        .map(|f| f.name)
        .chain(CONSTANTS.iter().map(|c| c.name))
        .collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "a function and a constant (or two entries) share a name"
    );
    for f in FUNCTIONS {
        assert!(
            f.name.chars().all(|c| c.is_ascii_alphanumeric()) && !f.doc.is_empty(),
            "{}",
            f.name
        );
    }
    assert_eq!(
        FUNCTIONS.len(),
        23,
        "exp4j 0.4.8 has exactly 23 built-in functions"
    );
}

#[test]
fn arity_accepts() {
    assert!(Arity::Exact(2).accepts(2) && !Arity::Exact(2).accepts(1));
    assert!(
        Arity::AtLeast(1).accepts(1)
            && Arity::AtLeast(1).accepts(5)
            && !Arity::AtLeast(1).accepts(0)
    );
    assert_eq!(Arity::AtLeast(2).to_string(), "at least 2 arguments");
}

/// The book's `math()` reference (`book/src/users_guide/math.md`) must contain every row the
/// registry generates (`registry::reference_rows`): adding or changing a function, operator or
/// constant forces a doc update.
#[test]
fn book_reference_matches_the_registry() {
    let page = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../book/src/users_guide/math.md"
    );
    let text = std::fs::read_to_string(page).unwrap_or_default();
    assert!(!text.is_empty(), "missing book page {page}");
    let missing: Vec<String> = super::registry::reference_rows()
        .into_iter()
        .filter(|row| !text.contains(row.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "{page} lacks the registry rows:\n{}",
        missing.join("\n")
    );
    for f in FUNCTIONS {
        assert!(
            text.contains(&format!("`{}(", f.name)),
            "{page} does not document {}",
            f.name
        );
    }
}
