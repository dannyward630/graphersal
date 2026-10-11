use std::fmt;

/// The Apache TinkerPop `Cardinality` token: how a vertex property write treats values that
/// already exist under the same key.
///
/// `Cardinality::Single` replaces the existing value(s), `Cardinality::List` appends another
/// value, `Cardinality::Set` appends a value only if it is not present yet. Graphersal stores one
/// value per vertex property key, so only `Single` can be honoured by writes; `List`/`Set` are
/// accepted tokens that fail with a clear error where a write would need them.
///
/// In the Rhai DSL: `Cardinality.single` / `Cardinality.list` / `Cardinality.set` (or
/// `Cardinality::single` ...); `Cardinality.single(v)` wraps a value with a cardinality for
/// `mergeV()` option maps.
///
/// ```rust
/// use graphersal::prelude::Cardinality;
///
/// assert_eq!(Cardinality::Single.to_string(), "single");
/// assert_ne!(Cardinality::List, Cardinality::Set);
/// ```
///
/// Closed set, so deliberately not `#[non_exhaustive]`: TinkerPop's property model defines exactly
/// these three cardinalities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cardinality {
    /// `Cardinality.single`: one value per key; a write replaces the existing value.
    Single,
    /// `Cardinality.list`: several values per key, duplicates allowed.
    List,
    /// `Cardinality.set`: several distinct values per key.
    Set,
}

impl fmt::Display for Cardinality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cardinality::Single => f.write_str("single"),
            Cardinality::List => f.write_str("list"),
            Cardinality::Set => f.write_str("set"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cardinality;

    #[test]
    fn display() {
        assert_eq!(Cardinality::Single.to_string(), "single");
        assert_eq!(Cardinality::List.to_string(), "list");
        assert_eq!(Cardinality::Set.to_string(), "set");
    }
}
