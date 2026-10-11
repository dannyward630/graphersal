//! [`ValueMap`]: the logical map value whose keys are values, not just strings.

use std::hash::{Hash, Hasher};

use fnv::{FnvBuildHasher, FnvHasher};
use indexmap::{Equivalent, IndexMap};

use super::ElementProperty;

/// A logical map value: any [`ElementProperty`] key to any [`ElementProperty`] value, insertion-ordered.
///
/// This is the payload of [`ElementProperty::Map`], the representation of a map that has at least one
/// non-string key (`{29: 1}`, `{["josh"]: 1}`). A map whose keys are all strings is always an
/// [`ElementProperty::Object`] instead; build maps through [`ElementProperty::from_entries`] or
/// [`ValueMap::into_property`], which enforce that rule.
///
/// Equality and hashing are order-independent (two maps with the same entries in a different order
/// are equal and hash alike), like TinkerPop's `Map.equals`/`hashCode`. Keys follow Java `equals`:
/// `1` and `1.0` are different keys, `NaN` equals `NaN`, `[1, 2]` differs from `[2, 1]`, `"1"` differs from `1`.
///
/// Deviations from TinkerPop (book page "Maps With Non-String Keys", `tinkerpop_deviations.md`): `0.0`
/// and `-0.0` are the same key (Java's `Double.equals` distinguishes them; Graphersal canonicalizes
/// `-0.0` when a number is built), and a map with a non-string key is never storable as a property.
#[derive(Clone, Default)]
pub struct ValueMap {
    entries: IndexMap<ElementProperty, ElementProperty, FnvBuildHasher>,
}

/// Borrowed string key: hashes and compares exactly like `ElementProperty::String`, without building one.
struct StrKey<'a>(&'a str);

impl Hash for StrKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        ElementProperty::hash_string_key(self.0, state);
    }
}

impl Equivalent<ElementProperty> for StrKey<'_> {
    fn equivalent(&self, key: &ElementProperty) -> bool {
        matches!(key, ElementProperty::String(s) if s.as_str() == self.0)
    }
}

impl ValueMap {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty map with room for `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: IndexMap::with_capacity_and_hasher(capacity, FnvBuildHasher::default()),
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The value stored under `key`.
    pub fn get(&self, key: &ElementProperty) -> Option<&ElementProperty> {
        self.entries.get(key)
    }

    /// The value stored under the string key `key` (no allocation: no `CompactString` is built).
    pub fn get_str(&self, key: &str) -> Option<&ElementProperty> {
        self.entries.get(&StrKey(key))
    }

    /// Inserts an entry; an existing key keeps its position and gets the new value (the old value is returned).
    pub fn insert(
        &mut self,
        key: ElementProperty,
        value: ElementProperty,
    ) -> Option<ElementProperty> {
        self.entries.insert(key, value)
    }

    /// The entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&ElementProperty, &ElementProperty)> {
        self.entries.iter()
    }

    /// The keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &ElementProperty> {
        self.entries.keys()
    }

    /// The values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &ElementProperty> {
        self.entries.values()
    }

    /// `true` when every key is a string, that is when this map is representable as a plain object.
    pub fn is_string_keyed(&self) -> bool {
        self.entries
            .keys()
            .all(|k| matches!(k, ElementProperty::String(_)))
    }

    /// Turns this map into a property value, canonicalized: a map whose keys are all strings becomes an
    /// [`ElementProperty::Object`], any other becomes an [`ElementProperty::Map`].
    pub fn into_property(self) -> ElementProperty {
        if self.is_string_keyed() {
            let mut object = super::PropertyMap::with_capacity(self.entries.len());
            for (key, value) in self.entries {
                if let ElementProperty::String(name) = key {
                    object.insert(name, value);
                }
            }
            ElementProperty::Object(Box::new(object))
        } else {
            ElementProperty::Map(Box::new(self))
        }
    }
}

impl FromIterator<(ElementProperty, ElementProperty)> for ValueMap {
    fn from_iter<I: IntoIterator<Item = (ElementProperty, ElementProperty)>>(iter: I) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for ValueMap {
    type Item = (ElementProperty, ElementProperty);
    type IntoIter = indexmap::map::IntoIter<ElementProperty, ElementProperty>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a ValueMap {
    type Item = (&'a ElementProperty, &'a ElementProperty);
    type IntoIter = indexmap::map::Iter<'a, ElementProperty, ElementProperty>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl PartialEq for ValueMap {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for ValueMap {}

/// Order-independent hash of a sequence of entries: each `(k, v)` is hashed with a fresh hasher and the
/// results are summed, then the length and the sum are fed to `state`. O(n), no allocation.
pub(crate) fn hash_entries_unordered<'a, K, V, H>(
    entries: impl ExactSizeIterator<Item = (&'a K, &'a V)>,
    state: &mut H,
) where
    K: Hash + 'a,
    V: Hash + 'a,
    H: Hasher,
{
    let len = entries.len();
    let mut sum = 0u64;
    for (k, v) in entries {
        let mut entry_hasher = FnvHasher::default();
        k.hash(&mut entry_hasher);
        v.hash(&mut entry_hasher);
        sum = sum.wrapping_add(entry_hasher.finish());
    }
    len.hash(state);
    sum.hash(state);
}

impl Hash for ValueMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_entries_unordered(self.entries.iter(), state);
    }
}

impl std::fmt::Debug for ValueMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.entries.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Number;

    fn int(n: i64) -> ElementProperty {
        ElementProperty::Number(Number::from(n))
    }
    fn float(n: f64) -> ElementProperty {
        ElementProperty::Number(Number::from(n))
    }
    fn text(s: &str) -> ElementProperty {
        ElementProperty::String(s.into())
    }
    fn hash_of<T: Hash>(v: &T) -> u64 {
        let mut h = FnvHasher::default();
        v.hash(&mut h);
        h.finish()
    }

    #[test]
    fn constructors_canonicalize() {
        assert!(matches!(
            ElementProperty::from_entries([(text("a"), int(1))]),
            ElementProperty::Object(_)
        ));
        assert!(matches!(
            ElementProperty::from_entries([(int(29), int(1))]),
            ElementProperty::Map(_)
        ));
        assert!(matches!(
            ElementProperty::from_entries([(text("a"), int(1)), (int(2), int(1))]),
            ElementProperty::Map(_)
        ));
        assert!(matches!(
            ElementProperty::from_entries(Vec::new()),
            ElementProperty::Object(_)
        ));
    }

    #[test]
    fn eq_and_hash_are_order_independent() {
        let a: ValueMap = [(int(1), text("x")), (text("k"), int(2))]
            .into_iter()
            .collect();
        let b: ValueMap = [(text("k"), int(2)), (int(1), text("x"))]
            .into_iter()
            .collect();
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        let pa = a.into_property();
        let pb = b.into_property();
        assert_eq!(hash_of(&pa), hash_of(&pb));
        assert_eq!(pa, pb);
    }

    #[test]
    fn object_hash_is_order_independent() {
        let mut x = crate::value::PropertyMap::default();
        x.insert("a", int(1));
        x.insert("b", int(2));
        let mut y = crate::value::PropertyMap::default();
        y.insert("b", int(2));
        y.insert("a", int(1));
        let (x, y) = (
            ElementProperty::Object(Box::new(x)),
            ElementProperty::Object(Box::new(y)),
        );
        assert_eq!(x, y);
        assert_eq!(hash_of(&x), hash_of(&y));
    }

    #[test]
    fn get_str_finds_string_keys_only() {
        let m: ValueMap = [(text("name"), int(1)), (int(7), int(2))]
            .into_iter()
            .collect();
        assert_eq!(m.get_str("name"), Some(&int(1)));
        assert_eq!(m.get_str("7"), None);
        assert_eq!(m.get(&int(7)), Some(&int(2)));
    }

    #[test]
    fn key_identity_follows_java_equals() {
        let m: ValueMap = [(int(1), int(1)), (float(1.0), int(2))]
            .into_iter()
            .collect();
        assert_eq!(m.len(), 2);
        let m: ValueMap = [(float(f64::NAN), int(1)), (float(f64::NAN), int(2))]
            .into_iter()
            .collect();
        assert_eq!(m.len(), 1);
        let l12 = ElementProperty::Array(vec![int(1), int(2)]);
        let l21 = ElementProperty::Array(vec![int(2), int(1)]);
        assert_ne!(l12, l21);
        let m: ValueMap = [(l12, int(1)), (l21, int(2))].into_iter().collect();
        assert_eq!(m.len(), 2);
        let m: ValueMap = [(text("1"), int(1)), (int(1), int(2))]
            .into_iter()
            .collect();
        assert_eq!(m.len(), 2);
    }
}
