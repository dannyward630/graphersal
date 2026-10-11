use std::fmt;

/// The Apache TinkerPop `Column` token: which half of a map, or of a map entry, to read.
///
/// Used as `select(Column::Keys)` / `select(Column::Values)` (Gremlin: `select(keys)` /
/// `select(values)`). On a whole map (a `group()`/`group_count()` result, `project()`,
/// `element_map()`, ...) it yields one list of all keys/values; on a single entry produced by
/// `unfold()` of a map it yields that entry's key/value.
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let lock = graph.read();
/// let names = lock
///     .traversal()
///     .v(None::<()>)
///     .group_count()
///     .by("name")
///     .unfold()
///     .select(Column::Keys)
///     .to_list()
///     .unwrap();
/// assert_eq!(names.len(), 6);
/// ```
///
/// Closed set, so deliberately not `#[non_exhaustive]`: TinkerPop defines exactly these two columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    /// `Column.keys`: the keys of a map, or the key of an entry.
    Keys,
    /// `Column.values`: the values of a map, or the value of an entry.
    Values,
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Column::Keys => f.write_str("keys"),
            Column::Values => f.write_str("values"),
        }
    }
}
