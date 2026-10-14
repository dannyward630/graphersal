use std::fmt;

use compact_str::CompactString;

/// A key of a [`TraverserValue::Record`](crate::TraverserValue::Record), the map `elementMap()`,
/// `valueMap(true)`, `project()` and `select()` build.
///
/// As in TinkerPop, the keys `elementMap()` and `valueMap(true)` give the element's id, label and
/// (edges) endpoints are the tokens `T.id`, `T.label`, `Direction.OUT` and `Direction.IN`, not
/// the strings `"id"`, `"label"`, `"OUT"`, `"IN"`. A property named `id` or `label` is a
/// [`RecordKey::Name`] beside them, so neither hides the other inside the map:
/// `select("label")` reads the property, `by(T.label)` the token.
///
/// When the map leaves the engine (a terminal, a script value, JSON, a table) the token keys are
/// written as the strings `"id"`, `"label"`, `"OUT"` and `"IN"` ([`RecordKey::as_str`]), the way
/// TinkerPop's untyped JSON writes them; when a property has the same name, the property's entry
/// wins, so a user who names a property `id` or `label` sees it in place of the system value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RecordKey {
    /// A key named by a string: a property name, a `project()` key or a `select()` label.
    Name(CompactString),
    /// The `T.id` token: the element's id.
    Id,
    /// The `T.label` token: the element's label.
    Label,
    /// The `Direction.OUT` token: an edge's outgoing vertex (`elementMap()` of an edge).
    Out,
    /// The `Direction.IN` token: an edge's incoming vertex (`elementMap()` of an edge).
    In,
}

impl RecordKey {
    /// The string the key is written as outside the engine: the name, or `id`, `label`, `OUT`,
    /// `IN` for a token.
    pub fn as_str(&self) -> &str {
        match self {
            RecordKey::Name(name) => name.as_str(),
            RecordKey::Id => "id",
            RecordKey::Label => "label",
            RecordKey::Out => "OUT",
            RecordKey::In => "IN",
        }
    }

    /// Whether the key is a token (`T.id`, `T.label`, `Direction.OUT`, `Direction.IN`) rather
    /// than a name.
    pub fn is_token(&self) -> bool {
        !matches!(self, RecordKey::Name(_))
    }

    /// The name of a [`RecordKey::Name`], `None` for a token.
    pub fn name(&self) -> Option<&str> {
        match self {
            RecordKey::Name(name) => Some(name.as_str()),
            RecordKey::Id | RecordKey::Label | RecordKey::Out | RecordKey::In => None,
        }
    }

    /// The key as the string it is written as outside the engine ([`RecordKey::as_str`]).
    pub fn to_compact(&self) -> CompactString {
        match self {
            RecordKey::Name(name) => name.clone(),
            RecordKey::Id | RecordKey::Label | RecordKey::Out | RecordKey::In => {
                CompactString::new(self.as_str())
            }
        }
    }
}

impl fmt::Display for RecordKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A name key equals its name; a token key equals no string (look it up by its variant).
impl PartialEq<str> for RecordKey {
    fn eq(&self, other: &str) -> bool {
        self.name() == Some(other)
    }
}

impl PartialEq<&str> for RecordKey {
    fn eq(&self, other: &&str) -> bool {
        self.name() == Some(*other)
    }
}

impl PartialEq<String> for RecordKey {
    fn eq(&self, other: &String) -> bool {
        self.name() == Some(other.as_str())
    }
}

impl PartialEq<CompactString> for RecordKey {
    fn eq(&self, other: &CompactString) -> bool {
        self.name() == Some(other.as_str())
    }
}

impl From<CompactString> for RecordKey {
    fn from(name: CompactString) -> Self {
        RecordKey::Name(name)
    }
}

impl From<&CompactString> for RecordKey {
    fn from(name: &CompactString) -> Self {
        RecordKey::Name(name.clone())
    }
}

impl From<&str> for RecordKey {
    fn from(name: &str) -> Self {
        RecordKey::Name(CompactString::new(name))
    }
}

impl From<String> for RecordKey {
    fn from(name: String) -> Self {
        RecordKey::Name(CompactString::from(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The token variants live in `CompactString`'s niche: a key is no larger than a name.
    #[test]
    fn a_record_key_is_as_large_as_a_compact_string() {
        assert_eq!(
            std::mem::size_of::<RecordKey>(),
            std::mem::size_of::<CompactString>()
        );
    }
}
