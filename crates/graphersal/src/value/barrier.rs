use std::fmt;

/// The Apache TinkerPop `Barrier` token: the built-in consumers of `barrier(Consumer)`.
///
/// TinkerPop's `SackFunctions.Barrier` has exactly one member, `normSack`: it merges the stream
/// like `barrier()` and then scales every numeric sack so that the sacks of all traversers
/// (weighted by bulk) add up to 1. In the Rhai DSL it is `Barrier.normSack` (or
/// `Barrier::normSack`); in Rust, [`GraphTraversalSource::barrier_with`](crate::GraphTraversalSource::barrier_with).
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let lock = graph.read();
/// let sacks = lock
///     .traversal()
///     .with_sack(1.0)
///     .v(None)
///     .out(None)
///     .barrier_with(Barrier::NormSack)
///     .sack()
///     .to_list()
///     .unwrap();
/// let total: f64 = sacks.iter().filter_map(|s| s.as_f64()).sum();
/// assert!((total - 1.0).abs() < 1e-9);
/// ```
///
/// `#[non_exhaustive]`: TinkerPop's set is closed today, but a consumer may be added later, so
/// downstream matches need a `_` arm.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Barrier {
    /// `Barrier.normSack`: normalise the numeric sacks of the merged stream to sum to 1.
    NormSack,
}

impl fmt::Display for Barrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Barrier::NormSack => f.write_str("normSack"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Barrier;

    #[test]
    fn display() {
        assert_eq!(Barrier::NormSack.to_string(), "normSack");
    }
}
