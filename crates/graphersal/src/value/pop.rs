use std::fmt;

/// The Apache TinkerPop `Pop` token: which occurrence of a repeated step label `select` reads.
///
/// A label occurs several times on a path when `as("a")` sits inside `repeat()` or is set at
/// several steps. `select("a")` reads the newest occurrence (`Pop::Last`); the other pops choose
/// differently (`select(Pop.first, "a")` in the Rhai DSL, [`select_pop`](crate::GraphTraversalSource::select_pop)
/// in Rust):
///
/// - [`First`](Pop::First): the oldest occurrence (path order).
/// - [`Last`](Pop::Last): the newest occurrence; the default and what `select("a")` does.
/// - [`All`](Pop::All): a list of every occurrence in path order, always a list (one occurrence
///   gives a one-element list). A label on no position of the path filters the traverser.
/// - [`Mixed`](Pop::Mixed): the occurrence itself when the label occurs once, a list in path
///   order when it occurs several times.
///
/// Several labels on one position (`as("a").as("a")`) are one occurrence. `Pop` applies only to a
/// label read from the path: a key of the traverser's own map and a side-effect key win over the
/// path and ignore the pop. See `book/src/users_guide/select_pop.md` for the semantics and the
/// verified TinkerPop sources.
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let lock = graph.read();
/// let names = lock
///     .traversal()
///     .v("1")
///     .as_("a")
///     .repeat(__::out(None).as_("a"))
///     .times(2)
///     .select_pop(Pop::All, "a")
///     .to_list()
///     .unwrap();
/// assert_eq!(names.len(), 2);
/// assert_eq!(Pop::default(), Pop::Last);
/// ```
///
/// Closed set, so deliberately not `#[non_exhaustive]` (rule 15): TinkerPop has defined exactly
/// these four members since 3.0, as for [`Scope`](crate::Scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Pop {
    /// `Pop.first`: the oldest occurrence of the label.
    First,
    /// `Pop.last`: the newest occurrence of the label (what `select("a")` reads).
    #[default]
    Last,
    /// `Pop.all`: a list of every occurrence, in path order.
    All,
    /// `Pop.mixed`: the value itself for one occurrence, a list for several.
    Mixed,
}

impl Pop {
    /// The TinkerPop name of the pop: `first`, `last`, `all` or `mixed`.
    pub fn name(self) -> &'static str {
        match self {
            Pop::First => "first",
            Pop::Last => "last",
            Pop::All => "all",
            Pop::Mixed => "mixed",
        }
    }
}

impl fmt::Display for Pop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::Pop;

    #[test]
    fn default_is_last() {
        assert_eq!(Pop::default(), Pop::Last);
    }

    #[test]
    fn display_is_the_tinkerpop_name() {
        assert_eq!(Pop::First.to_string(), "first");
        assert_eq!(Pop::Last.to_string(), "last");
        assert_eq!(Pop::All.to_string(), "all");
        assert_eq!(Pop::Mixed.to_string(), "mixed");
    }
}
