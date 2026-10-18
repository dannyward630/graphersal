//! A string a `dedup` compression rule stores once per distinct value ([`DedupedText`]), the
//! payload of the storage-only value
//! [`ElementProperty::Deduped`](crate::value::ElementProperty::Deduped)
//! (`book/src/users_guide/compressed_properties.md`, section "Deduplicating repeated values").
//!
//! The text lives in one shared, immutable allocation (`Arc<str>`) that the rule's table and
//! every element holding the same text point to; an element keeps only the reference. Reads
//! borrow the text directly (no decompression, no allocation), so a deduplicated value reads
//! as fast as a plain one.

use std::sync::Arc;

/// The bytes a shared text costs beyond its UTF-8 bytes: the two reference counts of its
/// allocation and its slot in the rule's table (a fat pointer). 32 on 64-bit targets. The
/// stats and the estimate count every distinct value as `len + DEDUP_ENTRY_OVERHEAD`.
pub(crate) const DEDUP_ENTRY_OVERHEAD: usize =
    2 * std::mem::size_of::<usize>() + std::mem::size_of::<Arc<str>>();

/// A string shared through a `dedup` rule's table: only the graph makes one (the rule's write
/// path or pass); it is never a stream, script or file value (see
/// [`ElementProperty::Deduped`](crate::value::ElementProperty::Deduped)).
#[doc(hidden)]
#[derive(Clone)]
pub struct DedupedText(Arc<str>);

impl DedupedText {
    /// A reference to this shared text.
    pub(crate) fn new(text: Arc<str>) -> Self {
        DedupedText(text)
    }

    /// The text.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text's length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the text is empty (never, for one a rule made).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The shared allocation.
    pub(crate) fn shared(&self) -> &Arc<str> {
        &self.0
    }

    /// Whether both point to the same shared allocation.
    #[inline]
    pub(crate) fn same_allocation(&self, other: &DedupedText) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Logical equality: the same allocation, or the same text.
    #[inline]
    pub(crate) fn logical_eq(&self, other: &DedupedText) -> bool {
        self.same_allocation(other) || *self.0 == *other.0
    }
}

impl std::fmt::Debug for DedupedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &*self.0)
    }
}
