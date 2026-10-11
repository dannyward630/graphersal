use std::fmt;

/// The Apache TinkerPop `Scope` token: whether a step works on the whole stream (`Global`) or on
/// the collection held by each single traverser (`Local`).
///
/// `Scope::Local` treats the one incoming object as a collection: a list is processed
/// element-wise, a map entry-wise (where the step defines it), and any other value as a
/// one-element collection. `Scope::Global` is the ordinary stream-wide step, so
/// `sum_scoped(Scope::Global)` is exactly `sum()`.
///
/// In the Rust API a scoped step is spelled `<step>_scoped(scope, ...)`, in the Rhai DSL
/// `Scope.local` / `Scope.global` (or `Scope::local` / `Scope::global`), for example
/// `g.V().values("age").fold().sum(Scope.local)`.
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let lock = graph.read();
/// let total = lock
///     .traversal()
///     .v(None::<()>)
///     .values("age")
///     .fold()
///     .sum_scoped(Scope::Local)
///     .to_list()
///     .unwrap();
/// assert_eq!(total, vec![ElementProperty::Number(123.into())]);
/// ```
///
/// Name clash: the embedded script engine has its own `rhai::Scope` (a variable scope). Code that
/// glob-imports `graphersal::prelude::*` and also needs Rhai's type imports `rhai::Scope`
/// explicitly (an explicit import shadows the glob one) or refers to this token by its full path
/// `graphersal::prelude::Scope`.
///
/// Closed set, so deliberately not `#[non_exhaustive]`: TinkerPop defines exactly these two scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Scope {
    /// `Scope.global`: the step works on the whole stream (the unscoped step).
    #[default]
    Global,
    /// `Scope.local`: the step works on the collection held by each traverser.
    Local,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::Global => f.write_str("global"),
            Scope::Local => f.write_str("local"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Scope;

    #[test]
    fn default_is_global() {
        assert_eq!(Scope::default(), Scope::Global);
    }

    #[test]
    fn display() {
        assert_eq!(Scope::Global.to_string(), "global");
        assert_eq!(Scope::Local.to_string(), "local");
    }
}
