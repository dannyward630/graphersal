use ordered_float::OrderedFloat;
use std::fmt;

/// A total-order-comparable `f64` (0.0 and -0.0 canonicalized, `NaN` sorts as greatest).
pub(crate) type OrderedF64 = OrderedFloat<f64>;

#[inline]
pub(crate) fn canonicalize_f64(value: f64) -> OrderedF64 {
    OrderedFloat(if value == 0.0 { 0.0 } else { value })
}

/// A numeric [`ElementProperty`](crate::value::ElementProperty) value: either an `i64` or an `f64`.
#[derive(Clone)]
pub struct Number {
    pub(crate) n: N,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum N {
    NegInt(i64),
    Float(OrderedF64),
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.n == other.n
    }
}

impl Eq for Number {}

/// Total order across `Number`'s sub-types, reused by `value::compare_element_property`
/// (order() parts 2/3) instead of being reimplemented there.
///
/// Same-sub-type pairs defer to the sub-type's own `Ord` (`i64`'s for two `NegInt`s,
/// `OrderedFloat`'s total order — which places `NaN` as greater than every other float and equal
/// to itself, never panicking — for two `Float`s). A cross-sub-type pair (`NegInt` vs `Float`)
/// converts the integer side to `f64` and compares numerically, so `1` and `1.0` compare `Equal`;
/// this can lose precision for an `i64` outside `f64`'s exact-integer range (beyond 2^53), the
/// same trade-off this crate already makes when coercing between the two sub-types elsewhere
/// (see `cast.rs`).
impl Ord for Number {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.n, &other.n) {
            (N::NegInt(a), N::NegInt(b)) => a.cmp(b),
            (N::Float(a), N::Float(b)) => a.cmp(b),
            (N::NegInt(a), N::Float(b)) => canonicalize_f64(*a as f64).cmp(b),
            (N::Float(a), N::NegInt(b)) => a.cmp(&canonicalize_f64(*b as f64)),
        }
    }
}

/// `Number`'s `Ord` impl above is a total order, so `partial_cmp` always returns `Some`.
impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::hash::Hash for Number {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.n.hash(state);
    }
}

impl fmt::Display for N {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            N::NegInt(value) => write!(f, "{}", value),
            N::Float(value) => write!(f, "{}", value),
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.n)
    }
}

impl fmt::Debug for N {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            N::NegInt(value) => write!(f, "{}", value),
            N::Float(value) => write!(f, "{}", value),
        }
    }
}

impl fmt::Debug for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.n {
            N::NegInt(value) => write!(f, "{}", value),
            N::Float(value) => write!(f, "{}", value),
        }
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Number {
            n: N::NegInt(value),
        }
    }
}

impl From<i32> for Number {
    fn from(value: i32) -> Self {
        Number {
            n: N::NegInt(value as i64),
        }
    }
}

impl From<usize> for Number {
    fn from(value: usize) -> Self {
        Number {
            n: N::NegInt(value as i64),
        }
    }
}

impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Number {
            n: N::Float(canonicalize_f64(value)),
        }
    }
}

impl From<Number> for String {
    fn from(value: Number) -> Self {
        match value.n {
            N::NegInt(value) => value.to_string(),
            N::Float(value) => value.to_string(),
        }
    }
}

impl Number {
    /// True if this number is stored as an `i64`.
    #[inline]
    pub fn is_i64(&self) -> bool {
        match self.n {
            N::NegInt(_) => true,
            N::Float(_) => false,
        }
    }

    /// True if this number is stored as an `f64`.
    #[inline]
    pub fn is_f64(&self) -> bool {
        match self.n {
            N::NegInt(_) => false,
            N::Float(_) => true,
        }
    }

    /// The `i64` value, or `None` if this number is stored as an `f64`.
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match self.n {
            N::NegInt(value) => Some(value),
            N::Float(_) => None,
        }
    }

    /// The `f64` value, or `None` if this number is stored as an `i64`.
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match self.n {
            N::NegInt(_) => None,
            N::Float(value) => Some(value.into_inner()),
        }
    }

    /// Builds a `Number` from an `i64`.
    #[inline]
    pub fn from_i64(value: i64) -> Number {
        Number {
            n: N::NegInt(value),
        }
    }

    /// Builds a `Number` from an `f64`, canonicalizing `0.0`/`-0.0`.
    #[inline]
    pub fn from_f64(value: f64) -> Number {
        Number {
            n: N::Float(canonicalize_f64(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fnv::FnvHasher;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = FnvHasher::default();
        t.hash(&mut s);
        s.finish()
    }

    #[test]
    fn test_zero_and_neg_zero_equality_and_hash() {
        let pos_zero = Number::from_f64(0.0);
        let neg_zero = Number::from_f64(-0.0);

        assert_eq!(pos_zero, neg_zero);
        assert_eq!(calculate_hash(&pos_zero), calculate_hash(&neg_zero));
    }

    #[test]
    fn test_number_hashmap_and_indexmap_lookup() {
        let pos_zero = Number::from_f64(0.0);
        let neg_zero = Number::from_f64(-0.0);

        let mut map = HashMap::new();
        map.insert(pos_zero.clone(), "zero");

        assert_eq!(map.get(&neg_zero), Some(&"zero"));

        let float_val = Number::from_f64(2.5);
        map.insert(float_val.clone(), "two_and_half");
        assert_eq!(map.get(&Number::from_f64(2.5)), Some(&"two_and_half"));
    }

    #[test]
    fn test_nan_equality() {
        let nan1 = Number::from_f64(f64::NAN);
        let nan2 = Number::from_f64(f64::NAN);

        assert_eq!(nan1, nan2);
        assert_eq!(calculate_hash(&nan1), calculate_hash(&nan2));
    }
}
