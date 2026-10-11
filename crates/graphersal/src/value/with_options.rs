use std::fmt;

/// The Apache TinkerPop `WithOptions` tokens: the keys and values of the step-level `with()`
/// modulator ([`with_option`](crate::GraphTraversalSource::with_option) in Rust,
/// `.with(WithOptions.tokens)` in the Rhai DSL).
///
/// Two options exist:
///
/// - [`Tokens`](WithOptions::Tokens) on `valueMap()`: which element tokens the map includes,
///   [`Ids`](WithOptions::Ids), [`Labels`](WithOptions::Labels), [`All`](WithOptions::All) (the
///   default of `with(WithOptions.tokens)`) or [`None`](WithOptions::None).
///   [`Keys`](WithOptions::Keys) and [`Values`](WithOptions::Values) select the key and value
///   tokens of a vertex-property map in TinkerPop; `valueMap()` here only reads vertices and
///   edges, so they add nothing.
/// - [`Indexer`](WithOptions::Indexer) on `index()`: [`List`](WithOptions::List) (the default,
///   `[element, position]` pairs) or [`Map`](WithOptions::Map) (a `{position: element}` map).
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = GraphSource::tinkerpop_modern();
/// let lock = graph.read();
/// let maps = lock
///     .traversal()
///     .v("1")
///     .value_map(vec!["name"])
///     .with_option(WithOptions::Tokens, Some(WithOptions::Labels))
///     .to_list()
///     .unwrap();
/// match &maps[0] {
///     ElementProperty::Object(map) => {
///         assert!(map.get("label").is_some());
///         assert!(map.get("id").is_none());
///     }
///     other => panic!("expected a map, got {other:?}"),
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WithOptions {
    /// `WithOptions.tokens`: the option key of `valueMap()`'s tokens.
    Tokens,
    /// `WithOptions.none`: no token.
    None,
    /// `WithOptions.ids`: the id token.
    Ids,
    /// `WithOptions.labels`: the label token.
    Labels,
    /// `WithOptions.keys`: the key token of a vertex property.
    Keys,
    /// `WithOptions.values`: the value token of a vertex property.
    Values,
    /// `WithOptions.all`: every token.
    All,
    /// `WithOptions.indexer`: the option key of `index()`'s indexer.
    Indexer,
    /// `WithOptions.list`: the list indexer (`[element, position]` pairs).
    List,
    /// `WithOptions.map`: the map indexer (`{position: element}`).
    Map,
}

impl WithOptions {
    /// The TinkerPop member name (`tokens`, `ids`, `indexer`, ...).
    pub fn name(self) -> &'static str {
        match self {
            WithOptions::Tokens => "tokens",
            WithOptions::None => "none",
            WithOptions::Ids => "ids",
            WithOptions::Labels => "labels",
            WithOptions::Keys => "keys",
            WithOptions::Values => "values",
            WithOptions::All => "all",
            WithOptions::Indexer => "indexer",
            WithOptions::List => "list",
            WithOptions::Map => "map",
        }
    }

    /// Every member, in TinkerPop's order (the DSL registers one getter per member).
    #[cfg_attr(not(feature = "script"), allow(dead_code))]
    pub(crate) const ALL: [WithOptions; 10] = [
        WithOptions::Tokens,
        WithOptions::None,
        WithOptions::Ids,
        WithOptions::Labels,
        WithOptions::Keys,
        WithOptions::Values,
        WithOptions::All,
        WithOptions::Indexer,
        WithOptions::List,
        WithOptions::Map,
    ];
}

impl fmt::Display for WithOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WithOptions.{}", self.name())
    }
}
