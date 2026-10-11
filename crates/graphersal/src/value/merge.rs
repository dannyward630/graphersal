use std::fmt;

/// The Apache TinkerPop `Merge` token: the option keys of the upsert steps `mergeV()`/`mergeE()`.
///
/// `Merge::OnCreate` / `Merge::OnMatch` select the property map applied when the element is
/// created / matched; `Merge::OutV` / `Merge::InV` select the traversal (or value) that resolves
/// the out-/in-vertex of `mergeE()`.
///
/// In the Rhai DSL: `Merge.onCreate` / `Merge.onMatch` / `Merge.outV` / `Merge.inV` (or
/// `Merge::onCreate` ...). Used as a *value* in a map, `Merge.outV`/`Merge.inV` become the
/// reserved strings `"Merge.outV"`/`"Merge.inV"`.
///
/// ```rust
/// use graphersal::prelude::Merge;
///
/// assert_eq!(Merge::OnCreate.to_string(), "onCreate");
/// assert_eq!(Merge::InV.to_string(), "inV");
/// ```
///
/// `#[non_exhaustive]`: TinkerPop has already extended this set once (`outV`/`inV` in 3.6), so
/// downstream matches need a `_` arm.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Merge {
    /// `Merge.onCreate`: properties applied when the merge creates the element.
    OnCreate,
    /// `Merge.onMatch`: properties applied when the merge matches an existing element.
    OnMatch,
    /// `Merge.outV`: resolves the out-vertex of `mergeE()`.
    OutV,
    /// `Merge.inV`: resolves the in-vertex of `mergeE()`.
    InV,
}

impl fmt::Display for Merge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Merge::OnCreate => f.write_str("onCreate"),
            Merge::OnMatch => f.write_str("onMatch"),
            Merge::OutV => f.write_str("outV"),
            Merge::InV => f.write_str("inV"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Merge;

    #[test]
    fn display() {
        assert_eq!(Merge::OnCreate.to_string(), "onCreate");
        assert_eq!(Merge::OnMatch.to_string(), "onMatch");
        assert_eq!(Merge::OutV.to_string(), "outV");
        assert_eq!(Merge::InV.to_string(), "inV");
    }
}
