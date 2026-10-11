//! [`PropertyMap`], a graph element's insertion-ordered property map.
//!
//! A newtype over the internal map so that the map crate behind it (`indexmap` with an FNV hasher
//! today) stays out of the public API: a major version of that crate cannot break a user. It is
//! `#[repr(transparent)]`, so it costs nothing at run time; the crate's own hot paths reach the
//! inner map through the `pub(crate)` field.

use std::fmt;
use std::hash::BuildHasherDefault;
use std::iter::FusedIterator;

use compact_str::CompactString;
use fnv::FnvHasher;
use indexmap::IndexMap;

use super::ElementProperty;

/// The map behind a [`PropertyMap`] (crate-internal).
pub(crate) type PropertyMapInner =
    IndexMap<CompactString, ElementProperty, BuildHasherDefault<FnvHasher>>;

/// A graph element's property map: property name to [`ElementProperty`] value, in insertion order.
///
/// It is the payload of [`ElementProperty::Object`], [`ElementProperty::Vertex`] and
/// [`ElementProperty::Edge`], and the property map a [`GraphStorage`](crate::graph::GraphStorage)
/// stores per element. Keys are [`CompactString`]s; every method that looks a key up takes a
/// `&str`. Equality ignores the order of the entries.
///
/// ```
/// use graphersal::prelude::{ElementProperty, PropertyMap};
///
/// let mut props = PropertyMap::new();
/// props.insert("name", ElementProperty::from("marko"));
/// props.insert("age", ElementProperty::from(29));
/// assert_eq!(props.len(), 2);
/// assert_eq!(props["name"], ElementProperty::from("marko"));
/// assert_eq!(props.keys().map(|k| k.as_str()).collect::<Vec<_>>(), ["name", "age"]);
/// ```
#[repr(transparent)]
#[derive(Default, Eq)]
pub struct PropertyMap(pub(crate) PropertyMapInner);

// `Clone` and `PartialEq` recurse through nested values (`ElementProperty::Object` holds a
// `PropertyMap`); `#[inline(always)]` keeps the wrapper from adding a stack frame per nesting
// level in unoptimized builds, where the deepest values (`traversal.max_value_depth`) run close to
// the 2 MiB test-thread stack.
impl Clone for PropertyMap {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl PartialEq for PropertyMap {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PropertyMap {
    /// An empty map.
    #[inline]
    pub fn new() -> Self {
        Self(PropertyMapInner::default())
    }

    /// An empty map with room for `capacity` entries.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(PropertyMapInner::with_capacity_and_hasher(
            capacity,
            BuildHasherDefault::default(),
        ))
    }

    /// The number of entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the map has no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The value of `key`, if present.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&ElementProperty> {
        self.0.get(key)
    }

    /// The value of `key`, mutably, if present.
    #[inline]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut ElementProperty> {
        self.0.get_mut(key)
    }

    /// Whether `key` is present.
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Inserts `value` under `key` and returns the previous value. A new key goes to the end; an
    /// existing key keeps its position.
    #[inline]
    pub fn insert(
        &mut self,
        key: impl Into<CompactString>,
        value: ElementProperty,
    ) -> Option<ElementProperty> {
        self.0.insert(key.into(), value)
    }

    /// Removes `key` and returns its value, keeping the order of the other entries.
    #[inline]
    pub fn shift_remove(&mut self, key: &str) -> Option<ElementProperty> {
        self.0.shift_remove(key)
    }

    /// The entries in order.
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.iter())
    }

    /// The entries in order, values mutable.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut(self.0.iter_mut())
    }

    /// The keys in order.
    #[inline]
    pub fn keys(&self) -> Keys<'_> {
        Keys(self.0.keys())
    }

    /// The values in order.
    #[inline]
    pub fn values(&self) -> Values<'_> {
        Values(self.0.values())
    }

    /// The values in order, mutable.
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_> {
        ValuesMut(self.0.values_mut())
    }

    /// Removes every entry, keeping the allocated capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl fmt::Debug for PropertyMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Looks a key up; panics when it is absent, like the indexing of the standard maps (use
/// [`PropertyMap::get`] for a fallible lookup).
impl std::ops::Index<&str> for PropertyMap {
    type Output = ElementProperty;

    #[inline]
    fn index(&self, key: &str) -> &ElementProperty {
        &self.0[key]
    }
}

impl<K: Into<CompactString>> FromIterator<(K, ElementProperty)> for PropertyMap {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (K, ElementProperty)>>(iter: I) -> Self {
        Self(iter.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }
}

impl<K: Into<CompactString>> Extend<(K, ElementProperty)> for PropertyMap {
    #[inline]
    fn extend<I: IntoIterator<Item = (K, ElementProperty)>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().map(|(k, v)| (k.into(), v)));
    }
}

impl<K: Into<CompactString>, const N: usize> From<[(K, ElementProperty); N]> for PropertyMap {
    #[inline]
    fn from(entries: [(K, ElementProperty); N]) -> Self {
        entries.into_iter().collect()
    }
}

impl IntoIterator for PropertyMap {
    type Item = (CompactString, ElementProperty);
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl<'a> IntoIterator for &'a PropertyMap {
    type Item = (&'a CompactString, &'a ElementProperty);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut PropertyMap {
    type Item = (&'a CompactString, &'a mut ElementProperty);
    type IntoIter = IterMut<'a>;

    #[inline]
    fn into_iter(self) -> IterMut<'a> {
        self.iter_mut()
    }
}

/// Implements the iterator traits of a wrapper around an `indexmap` iterator.
macro_rules! wrap_iter {
    ($name:ident<$lt:lifetime>, $item:ty) => {
        impl<$lt> Iterator for $name<$lt> {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<$item> {
                self.0.next()
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }

        impl<$lt> DoubleEndedIterator for $name<$lt> {
            #[inline]
            fn next_back(&mut self) -> Option<$item> {
                self.0.next_back()
            }
        }

        impl<$lt> ExactSizeIterator for $name<$lt> {}

        impl<$lt> FusedIterator for $name<$lt> {}
    };
}

/// The entries of a [`PropertyMap`] in order ([`PropertyMap::iter`]).
#[derive(Clone)]
pub struct Iter<'a>(indexmap::map::Iter<'a, CompactString, ElementProperty>);
wrap_iter!(Iter<'a>, (&'a CompactString, &'a ElementProperty));

impl fmt::Debug for Iter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// The entries of a [`PropertyMap`] in order, values mutable ([`PropertyMap::iter_mut`]).
pub struct IterMut<'a>(indexmap::map::IterMut<'a, CompactString, ElementProperty>);
wrap_iter!(IterMut<'a>, (&'a CompactString, &'a mut ElementProperty));

impl fmt::Debug for IterMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IterMut")
            .field("remaining", &self.0.len())
            .finish()
    }
}

/// The keys of a [`PropertyMap`] in order ([`PropertyMap::keys`]).
#[derive(Clone)]
pub struct Keys<'a>(indexmap::map::Keys<'a, CompactString, ElementProperty>);
wrap_iter!(Keys<'a>, &'a CompactString);

impl fmt::Debug for Keys<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// The values of a [`PropertyMap`] in order ([`PropertyMap::values`]).
#[derive(Clone)]
pub struct Values<'a>(indexmap::map::Values<'a, CompactString, ElementProperty>);
wrap_iter!(Values<'a>, &'a ElementProperty);

impl fmt::Debug for Values<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// The values of a [`PropertyMap`] in order, mutable ([`PropertyMap::values_mut`]).
pub struct ValuesMut<'a>(indexmap::map::ValuesMut<'a, CompactString, ElementProperty>);
wrap_iter!(ValuesMut<'a>, &'a mut ElementProperty);

impl fmt::Debug for ValuesMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValuesMut")
            .field("remaining", &self.0.len())
            .finish()
    }
}

/// The entries of a [`PropertyMap`] by value, in order (its `IntoIterator`).
pub struct IntoIter(indexmap::map::IntoIter<CompactString, ElementProperty>);

impl Iterator for IntoIter {
    type Item = (CompactString, ElementProperty);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for IntoIter {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl ExactSizeIterator for IntoIter {}

impl FusedIterator for IntoIter {}

impl fmt::Debug for IntoIter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoIter")
            .field("remaining", &self.0.len())
            .finish()
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{PropertyMap, PropertyMapInner};

    /// Serializes as a JSON object, entries in order.
    impl Serialize for PropertyMap {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.0.serialize(serializer)
        }
    }

    /// Deserializes a JSON object, entries in order.
    impl<'de> Deserialize<'de> for PropertyMap {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            PropertyMapInner::deserialize(deserializer).map(PropertyMap)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PropertyMap, PropertyMapInner};
    use crate::value::ElementProperty;

    #[test]
    fn the_newtype_costs_nothing() {
        assert_eq!(size_of::<PropertyMap>(), size_of::<PropertyMapInner>());
        assert_eq!(align_of::<PropertyMap>(), align_of::<PropertyMapInner>());
        assert_eq!(size_of::<ElementProperty>(), 32);
    }

    #[test]
    fn keeps_insertion_order_and_compares_unordered() {
        let mut a = PropertyMap::new();
        a.insert("b", ElementProperty::from(1));
        a.insert("a", ElementProperty::from(2));
        a.insert("b", ElementProperty::from(3));
        assert_eq!(a.keys().map(|k| k.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(a["b"], ElementProperty::from(3));
        let b: PropertyMap = [
            ("a", ElementProperty::from(2)),
            ("b", ElementProperty::from(3)),
        ]
        .into_iter()
        .collect();
        assert_eq!(a, b);
        assert_eq!(a.shift_remove("b"), Some(ElementProperty::from(3)));
        assert!(!a.contains_key("b"));
        a.extend([("c", ElementProperty::Null)]);
        for value in a.values_mut() {
            *value = ElementProperty::Null;
        }
        let owned: Vec<_> = a.clone().into_iter().collect();
        assert_eq!(owned.len(), 2);
        assert_eq!((&a).into_iter().len(), 2);
        a.clear();
        assert!(a.is_empty());
    }
}
