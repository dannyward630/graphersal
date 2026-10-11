//! The ordering of comparable scalars, shared by predicates (`P::compare`) and `min()`/`max()`.
//!
//! Numbers compare by value across `int64`/`float64` (`NaN` is incomparable), strings by
//! `str::cmp`, booleans as `false < true`, UUIDs by their `u128` word. Nothing else is
//! comparable here, and a string is never read as a number: values of different kinds are
//! incomparable (`None`).

use std::cmp::Ordering;

use crate::value::ElementProperty;
use crate::value::number::{N, Number};

/// Orders two numbers by value, across the integer and float representations.
pub(crate) fn compare_numbers(a: &Number, b: &Number) -> Option<Ordering> {
    match (&a.n, &b.n) {
        (N::NegInt(a), N::NegInt(b)) => Some(a.cmp(b)),
        (N::Float(a), N::Float(b)) => a.0.partial_cmp(&b.0),
        (N::NegInt(a), N::Float(b)) => (*a as f64).partial_cmp(&b.0),
        (N::Float(a), N::NegInt(b)) => a.0.partial_cmp(&(*b as f64)),
    }
}

/// Orders two property values of the same comparable kind (number, string, boolean, uuid);
/// `None` for two different kinds, for `null`, and for containers and elements.
pub(crate) fn compare_scalars(a: &ElementProperty, b: &ElementProperty) -> Option<Ordering> {
    match (a, b) {
        (ElementProperty::Number(a), ElementProperty::Number(b)) => compare_numbers(a, b),
        (ElementProperty::String(a), ElementProperty::String(b)) => {
            Some(a.as_str().cmp(b.as_str()))
        }
        // A compressed string compares as the string it holds.
        (a, b) if a.is_text() && b.is_text() => {
            a.with_text(|a| b.with_text(|b| a.cmp(b))).flatten()
        }
        (ElementProperty::Boolean(a), ElementProperty::Boolean(b)) => Some(a.cmp(b)),
        (ElementProperty::Uuid(a), ElementProperty::Uuid(b)) => Some(a.cmp(b)),
        _ => None,
    }
}
