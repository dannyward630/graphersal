//! The `Operator` token: pure semantics of [`Operator::apply`] (design `sack-design.md` section 3)
//! and the Rhai constants.

use graphersal::error::{OperatorFailure, ValueError};
use graphersal::prelude::*;

fn int(v: i64) -> ElementProperty {
    ElementProperty::Number(Number::from(v))
}
fn float(v: f64) -> ElementProperty {
    ElementProperty::Number(Number::from(v))
}
fn text(v: &str) -> ElementProperty {
    ElementProperty::String(v.into())
}
fn list(items: Vec<ElementProperty>) -> ElementProperty {
    ElementProperty::Array(items)
}
fn map(entries: Vec<(&str, ElementProperty)>) -> ElementProperty {
    ElementProperty::from_entries(entries.into_iter().map(|(k, v)| (text(k), v)))
}
fn ok(op: Operator, a: ElementProperty, b: ElementProperty) -> ElementProperty {
    op.apply(&a, &b)
        .unwrap_or_else(|e| panic!("{op}({a:?}, {b:?}) failed: {e}"))
}
fn failure(op: Operator, a: ElementProperty, b: ElementProperty) -> OperatorFailure {
    match op.apply(&a, &b) {
        Err(ValueError::OperatorFailed { kind, operator, .. }) => {
            assert_eq!(operator, op.name());
            kind
        }
        other => panic!("{op}({a:?}, {b:?}) should fail with OperatorFailed, got {other:?}"),
    }
}
fn incompatible(op: Operator, a: ElementProperty, b: ElementProperty) {
    assert!(
        matches!(
            failure(op, a, b),
            OperatorFailure::IncompatibleOperands { .. }
        ),
        "expected IncompatibleOperands"
    );
}

#[test]
fn numeric_sack_scenarios_as_pure_calls() {
    use Operator::*;
    assert_eq!(ok(Sum, int(127), int(1)), int(128));
    assert_eq!(ok(Sum, int(32767), int(1)), int(32768));
    assert_eq!(ok(Sum, int(2147483647), int(1)), int(2147483648));
    assert_eq!(ok(Minus, int(-128), int(1)), int(-129));
    assert_eq!(ok(Mult, int(127), int(2)), int(254));
    assert_eq!(ok(Mult, int(2147483647), int(2)), int(4294967294));
    assert_eq!(ok(Div, int(127), float(0.5)), float(254.0));
    assert_eq!(ok(Div, int(-128), int(-1)), int(128));
    assert_eq!(
        ok(Sum, float(f64::MAX), float(f64::MAX)),
        float(f64::INFINITY)
    );
    assert_eq!(
        ok(Minus, float(-f64::MAX), float(f64::MAX)),
        float(f64::NEG_INFINITY)
    );
    assert_eq!(ok(Mult, float(f64::MAX), int(2)), float(f64::INFINITY));
    assert_eq!(ok(Div, float(f64::MAX), float(0.5)), float(f64::INFINITY));
}

#[test]
fn integer_overflow_and_division_rules() {
    use Operator::*;
    assert_eq!(
        failure(Sum, int(i64::MAX), int(1)),
        OperatorFailure::Overflow
    );
    assert_eq!(
        failure(Minus, int(i64::MIN), int(1)),
        OperatorFailure::Overflow
    );
    assert_eq!(
        failure(Mult, int(i64::MAX), int(2)),
        OperatorFailure::Overflow
    );
    assert_eq!(ok(Div, int(7), int(2)), int(3));
    assert_eq!(ok(Div, int(-7), int(2)), int(-3));
    assert_eq!(
        failure(Div, int(1), int(0)),
        OperatorFailure::DivisionByZero
    );
    assert_eq!(ok(Div, float(1.0), float(0.0)), float(f64::INFINITY));
    assert_eq!(ok(Div, int(1), float(0.0)), float(f64::INFINITY));
    match ok(Div, float(0.0), float(0.0)) {
        ElementProperty::Number(n) => assert!(n.to_string().contains("NaN"), "{n}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        failure(Div, int(i64::MIN), int(-1)),
        OperatorFailure::Overflow
    );
    incompatible(Sum, int(3), text("a"));
    incompatible(Mult, ElementProperty::Boolean(true), int(1));
}

#[test]
fn aggregate_scenarios_as_folds() {
    use Operator::*;
    let stream = [29, 27, 32, 35];
    let fold = |op: Operator, seed: ElementProperty| {
        stream.iter().fold(seed, |acc, v| ok(op, acc, int(*v)))
    };
    assert_eq!(fold(Minus, int(123)), int(0));
    assert_eq!(fold(Mult, int(2)), int(1753920));
    assert_eq!(fold(Div, int(876960)), int(1));
    assert_eq!(fold(Min, int(100)), int(27));
    assert_eq!(fold(Max, int(1)), int(35));
    assert_eq!(
        ok(
            And,
            ElementProperty::Boolean(true),
            ElementProperty::Boolean(false)
        ),
        ElementProperty::Boolean(false)
    );
    assert_eq!(
        ok(
            Or,
            ElementProperty::Boolean(true),
            ElementProperty::Boolean(false)
        ),
        ElementProperty::Boolean(true)
    );
    assert_eq!(
        ok(AddAll, list(vec![int(1), int(2), int(3)]), int(29)),
        list(vec![int(1), int(2), int(3), int(29)])
    );
    assert_eq!(
        ok(
            AddAll,
            list(vec![int(1), int(2), int(3)]),
            list(vec![int(4)])
        ),
        list(vec![int(1), int(2), int(3), int(4)])
    );
    assert_eq!(ok(Assign, int(1), text("x")), text("x"));
}

#[test]
fn min_max_kinds_and_null_rules() {
    use Operator::*;
    assert_eq!(ok(Min, text("b"), text("a")), text("a"));
    assert_eq!(
        ok(
            Max,
            ElementProperty::Boolean(false),
            ElementProperty::Boolean(true)
        ),
        ElementProperty::Boolean(true)
    );
    assert_eq!(ok(Min, int(1), float(2.5)), float(1.0));
    incompatible(Min, text("a"), int(1));
    incompatible(Max, list(vec![]), list(vec![]));
    let null = || ElementProperty::Null;
    assert_eq!(ok(Sum, null(), int(1)), null());
    assert_eq!(ok(Sum, int(1), null()), int(1));
    assert_eq!(ok(Minus, null(), int(1)), null());
    assert_eq!(ok(Mult, int(2), null()), int(2));
    assert_eq!(ok(Div, int(2), null()), int(2));
    assert_eq!(ok(Max, null(), int(3)), int(3));
    assert_eq!(ok(Min, int(3), null()), int(3));
    assert_eq!(
        ok(And, null(), ElementProperty::Boolean(true)),
        ElementProperty::Boolean(true)
    );
    assert_eq!(
        ok(Or, ElementProperty::Boolean(false), null()),
        ElementProperty::Boolean(false)
    );
    assert_eq!(ok(And, null(), null()), null());
    assert_eq!(ok(Assign, int(1), null()), null());
    incompatible(And, int(1), ElementProperty::Boolean(true));
}

#[test]
fn add_all_maps_and_errors() {
    use Operator::AddAll;
    assert_eq!(
        ok(AddAll, map(vec![("a", int(1))]), map(vec![("b", int(2))])),
        map(vec![("a", int(1)), ("b", int(2))])
    );
    let merged = ok(
        AddAll,
        map(vec![("a", int(1)), ("b", int(2))]),
        map(vec![("b", int(4))]),
    );
    match merged {
        ElementProperty::Object(o) => {
            let entries: Vec<_> = o.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
            assert_eq!(
                entries,
                vec![("a".to_string(), int(1)), ("b".to_string(), int(4))]
            );
        }
        other => panic!("{other:?}"),
    }
    incompatible(AddAll, list(vec![int(1)]), map(vec![("a", int(1))]));
    incompatible(AddAll, map(vec![("a", int(1))]), int(1));
    let keyed = ElementProperty::from_entries([(int(1), text("x"))]);
    let merged = ok(AddAll, keyed, map(vec![("a", int(1))]));
    assert!(matches!(merged, ElementProperty::Map(m) if m.len() == 2));
}

#[test]
fn sum_long_is_int64_only_and_does_not_wrap() {
    use Operator::SumLong;
    assert_eq!(ok(SumLong, int(1), int(2)), int(3));
    incompatible(SumLong, int(1), float(2.0));
    assert_eq!(
        failure(SumLong, int(i64::MAX), int(1)),
        OperatorFailure::Overflow
    );
}

#[cfg(feature = "script")]
mod script {
    use crate::script_host::{eval_value, eval_with_params};
    use graphersal::prelude::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn eval_str(script: &str) -> String {
        eval_value(Arc::new(GraphSource::empty()), script)
            .unwrap_or_else(|e| panic!("{script}: {e}"))
            .to_string()
    }

    #[test]
    fn tokens_print_their_dsl_literal() {
        for (script, expected) in [
            ("Operator.sum", "Operator.sum"),
            ("Operator::sum", "Operator.sum"),
            ("Operator.addAll", "Operator.addAll"),
            ("Operator::sumLong", "Operator.sumLong"),
            ("Operator.minus", "Operator.minus"),
            ("Operator.mult", "Operator.mult"),
            ("Operator.div", "Operator.div"),
            ("Operator.max", "Operator.max"),
            ("Operator.min", "Operator.min"),
            ("Operator.assign", "Operator.assign"),
            ("Operator.and", "Operator.and"),
            ("Operator.or", "Operator.or"),
        ] {
            assert_eq!(
                eval_str(&format!("({script}).to_string()")),
                expected,
                "{script}"
            );
        }
    }

    #[test]
    fn tokens_compare() {
        assert_eq!(eval_str("Operator.sum == Operator::sum"), "true");
        assert_eq!(eval_str("Operator.sum == Operator.minus"), "false");
        assert_eq!(eval_str("Operator.sum != Operator.minus"), "true");
    }

    #[test]
    fn unknown_constant_fails() {
        assert!(eval_value(Arc::new(GraphSource::empty()), "Operator.nope").is_err());
        assert!(eval_value(Arc::new(GraphSource::empty()), "Operator::nope").is_err());
    }

    #[test]
    fn operator_is_a_reserved_parameter_name() {
        let err = eval_with_params(
            Arc::new(GraphSource::tinkerpop_modern()),
            "g.v().count().next()",
            HashMap::from([("Operator".to_string(), ElementProperty::Boolean(true))]),
        )
        .expect_err("a parameter shadowing the Operator token must be rejected");
        assert!(
            err.to_string().contains("reserved by the script scope"),
            "{err}"
        );
    }
}
