//! [`StreamMap`], the insertion-ordered map a traversal produces (`group()`, `tree()`, ...).
//!
//! A newtype over the internal map so that the map crate behind it (`indexmap` with an FNV hasher
//! today) stays out of the public API: a major version of that crate cannot break a user. It is
//! `#[repr(transparent)]`, so it costs nothing at run time; the crate's own hot paths reach the
//! inner map through the `pub(crate)` field.

use std::fmt;
use std::iter::FusedIterator;

use fnv::FnvBuildHasher;
use indexmap::IndexMap;

use super::TraverserValue;
use crate::graph::GraphStorage;

/// The map behind a [`StreamMap`] (crate-internal).
pub(crate) type StreamMapInner<S> = IndexMap<TraverserValue<S>, TraverserValue<S>, FnvBuildHasher>;

/// A map in the traversal stream: any [`TraverserValue`] key to any [`TraverserValue`] value, in
/// insertion order.
///
/// It is the payload of [`TraverserValue::Object`], the map that `group()`, `group_count()`,
/// `tree()`, `cap()` and the map-valued local steps produce. Keys are values (a vertex, a number,
/// a string, ...). Equality ignores the order of the entries.
///
/// ```
/// use graphersal::prelude::*;
/// use graphersal::prelude::StreamMap;
///
/// let mut map: StreamMap<TraversalGraph> = StreamMap::new();
/// map.insert(TraverserValue::from("marko"), ElementProperty::from(29).into());
/// map.insert(TraverserValue::from("josh"), ElementProperty::from(32).into());
/// assert_eq!(map.len(), 2);
/// assert_eq!(map.get(&TraverserValue::from("josh")), Some(&ElementProperty::from(32).into()));
/// ```
#[repr(transparent)]
pub struct StreamMap<S: GraphStorage>(pub(crate) StreamMapInner<S>);

// `Clone` and `PartialEq` recurse through nested values (a `TraverserValue::Object` holds a
// `StreamMap`); `#[inline(always)]` keeps the wrapper from adding a stack frame per nesting level
// in unoptimized builds, where the deepest values (`traversal.max_value_depth`) run close to the
// 2 MiB test-thread stack.
impl<S: GraphStorage> Clone for StreamMap<S> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl<S: GraphStorage> PartialEq for StreamMap<S> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<S: GraphStorage> Eq for StreamMap<S> {}

impl<S: GraphStorage> Default for StreamMap<S> {
    #[inline]
    fn default() -> Self {
        Self(StreamMapInner::default())
    }
}

impl<S: GraphStorage> StreamMap<S> {
    /// An empty map.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty map with room for `capacity` entries.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(StreamMapInner::with_capacity_and_hasher(
            capacity,
            FnvBuildHasher::default(),
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
    pub fn get(&self, key: &TraverserValue<S>) -> Option<&TraverserValue<S>> {
        self.0.get(key)
    }

    /// The value of `key`, mutably, if present.
    #[inline]
    pub fn get_mut(&mut self, key: &TraverserValue<S>) -> Option<&mut TraverserValue<S>> {
        self.0.get_mut(key)
    }

    /// Whether `key` is present.
    #[inline]
    pub fn contains_key(&self, key: &TraverserValue<S>) -> bool {
        self.0.contains_key(key)
    }

    /// Inserts `value` under `key` and returns the previous value. A new key goes to the end; an
    /// existing key keeps its position.
    #[inline]
    pub fn insert(
        &mut self,
        key: TraverserValue<S>,
        value: TraverserValue<S>,
    ) -> Option<TraverserValue<S>> {
        self.0.insert(key, value)
    }

    /// Removes `key` and returns its value, keeping the order of the other entries.
    #[inline]
    pub fn shift_remove(&mut self, key: &TraverserValue<S>) -> Option<TraverserValue<S>> {
        self.0.shift_remove(key)
    }

    /// The entries in order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, S> {
        Iter(self.0.iter())
    }

    /// The entries in order, values mutable.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, S> {
        IterMut(self.0.iter_mut())
    }

    /// The keys in order.
    #[inline]
    pub fn keys(&self) -> Keys<'_, S> {
        Keys(self.0.keys())
    }

    /// The values in order.
    #[inline]
    pub fn values(&self) -> Values<'_, S> {
        Values(self.0.values())
    }

    /// The values in order, mutable.
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_, S> {
        ValuesMut(self.0.values_mut())
    }

    /// Removes every entry, keeping the allocated capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<S: GraphStorage> fmt::Debug for StreamMap<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<S: GraphStorage> FromIterator<(TraverserValue<S>, TraverserValue<S>)> for StreamMap<S> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (TraverserValue<S>, TraverserValue<S>)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<S: GraphStorage> Extend<(TraverserValue<S>, TraverserValue<S>)> for StreamMap<S> {
    #[inline]
    fn extend<I: IntoIterator<Item = (TraverserValue<S>, TraverserValue<S>)>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

impl<S: GraphStorage, const N: usize> From<[(TraverserValue<S>, TraverserValue<S>); N]>
    for StreamMap<S>
{
    #[inline]
    fn from(entries: [(TraverserValue<S>, TraverserValue<S>); N]) -> Self {
        entries.into_iter().collect()
    }
}

impl<S: GraphStorage> IntoIterator for StreamMap<S> {
    type Item = (TraverserValue<S>, TraverserValue<S>);
    type IntoIter = IntoIter<S>;

    #[inline]
    fn into_iter(self) -> IntoIter<S> {
        IntoIter(self.0.into_iter())
    }
}

impl<'a, S: GraphStorage> IntoIterator for &'a StreamMap<S> {
    type Item = (&'a TraverserValue<S>, &'a TraverserValue<S>);
    type IntoIter = Iter<'a, S>;

    #[inline]
    fn into_iter(self) -> Iter<'a, S> {
        self.iter()
    }
}

impl<'a, S: GraphStorage> IntoIterator for &'a mut StreamMap<S> {
    type Item = (&'a TraverserValue<S>, &'a mut TraverserValue<S>);
    type IntoIter = IterMut<'a, S>;

    #[inline]
    fn into_iter(self) -> IterMut<'a, S> {
        self.iter_mut()
    }
}

/// Implements the iterator traits of a wrapper around an `indexmap` iterator.
macro_rules! wrap_iter {
    ($name:ident<$($lt:lifetime,)? S>, $item:ty) => {
        impl<$($lt,)? S: GraphStorage> Iterator for $name<$($lt,)? S> {
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

        impl<$($lt,)? S: GraphStorage> DoubleEndedIterator for $name<$($lt,)? S> {
            #[inline]
            fn next_back(&mut self) -> Option<$item> {
                self.0.next_back()
            }
        }

        impl<$($lt,)? S: GraphStorage> ExactSizeIterator for $name<$($lt,)? S> {}

        impl<$($lt,)? S: GraphStorage> FusedIterator for $name<$($lt,)? S> {}

        impl<$($lt,)? S: GraphStorage> fmt::Debug for $name<$($lt,)? S> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("remaining", &self.0.len())
                    .finish()
            }
        }
    };
}

/// The entries of a [`StreamMap`] in order ([`StreamMap::iter`]).
pub struct Iter<'a, S: GraphStorage>(indexmap::map::Iter<'a, TraverserValue<S>, TraverserValue<S>>);
wrap_iter!(Iter<'a, S>, (&'a TraverserValue<S>, &'a TraverserValue<S>));

impl<S: GraphStorage> Clone for Iter<'_, S> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// The entries of a [`StreamMap`] in order, values mutable ([`StreamMap::iter_mut`]).
pub struct IterMut<'a, S: GraphStorage>(
    indexmap::map::IterMut<'a, TraverserValue<S>, TraverserValue<S>>,
);
wrap_iter!(
    IterMut<'a, S>,
    (&'a TraverserValue<S>, &'a mut TraverserValue<S>)
);

/// The keys of a [`StreamMap`] in order ([`StreamMap::keys`]).
pub struct Keys<'a, S: GraphStorage>(indexmap::map::Keys<'a, TraverserValue<S>, TraverserValue<S>>);
wrap_iter!(Keys<'a, S>, &'a TraverserValue<S>);

impl<S: GraphStorage> Clone for Keys<'_, S> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// The values of a [`StreamMap`] in order ([`StreamMap::values`]).
pub struct Values<'a, S: GraphStorage>(
    indexmap::map::Values<'a, TraverserValue<S>, TraverserValue<S>>,
);
wrap_iter!(Values<'a, S>, &'a TraverserValue<S>);

impl<S: GraphStorage> Clone for Values<'_, S> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// The values of a [`StreamMap`] in order, mutable ([`StreamMap::values_mut`]).
pub struct ValuesMut<'a, S: GraphStorage>(
    indexmap::map::ValuesMut<'a, TraverserValue<S>, TraverserValue<S>>,
);
wrap_iter!(ValuesMut<'a, S>, &'a mut TraverserValue<S>);

/// The entries of a [`StreamMap`] by value, in order (its `IntoIterator`).
pub struct IntoIter<S: GraphStorage>(indexmap::map::IntoIter<TraverserValue<S>, TraverserValue<S>>);
wrap_iter!(IntoIter<S>, (TraverserValue<S>, TraverserValue<S>));

#[cfg(test)]
mod tests {
    use super::{StreamMap, StreamMapInner};
    use crate::graph::TraversalGraph;
    use crate::value::{ElementProperty, TraverserValue};

    type V = TraverserValue<TraversalGraph>;

    fn n(i: i64) -> V {
        V::from(ElementProperty::from(i))
    }

    #[test]
    fn the_newtype_costs_nothing() {
        assert_eq!(
            size_of::<StreamMap<TraversalGraph>>(),
            size_of::<StreamMapInner<TraversalGraph>>()
        );
        assert_eq!(
            align_of::<StreamMap<TraversalGraph>>(),
            align_of::<StreamMapInner<TraversalGraph>>()
        );
        assert_eq!(size_of::<V>(), 64);
    }

    #[test]
    fn keeps_insertion_order_and_compares_unordered() {
        let mut a: StreamMap<TraversalGraph> = StreamMap::with_capacity(2);
        a.insert(V::from("b"), n(1));
        a.insert(V::from("a"), n(2));
        a.insert(V::from("b"), n(3));
        assert_eq!(
            a.keys().cloned().collect::<Vec<_>>(),
            [V::from("b"), V::from("a")]
        );
        assert_eq!(a.get(&V::from("b")), Some(&n(3)));
        let b: StreamMap<TraversalGraph> =
            StreamMap::from([(V::from("a"), n(2)), (V::from("b"), n(3))]);
        assert_eq!(a, b);
        assert_eq!(a.shift_remove(&V::from("b")), Some(n(3)));
        assert!(!a.contains_key(&V::from("b")));
        a.extend([(V::from("c"), n(4))]);
        for value in a.values_mut() {
            *value = n(0);
        }
        if let Some(v) = a.get_mut(&V::from("c")) {
            *v = n(5);
        }
        assert_eq!(a.values().next_back(), Some(&n(5)));
        let owned: Vec<_> = a.clone().into_iter().collect();
        assert_eq!(owned.len(), 2);
        assert_eq!((&a).into_iter().len(), 2);
        assert_eq!(a.iter_mut().len(), 2);
        a.clear();
        assert!(a.is_empty());
        assert!(StreamMap::<TraversalGraph>::new().is_empty());
    }
}
