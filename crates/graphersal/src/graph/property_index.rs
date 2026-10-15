//! Property indexes (`book/src/users_guide/property_indexes.md`; book page "Property Indexes"):
//! declared, ordered indexes `(element kind, label, key) -> value -> elements` that let a
//! `v()`/`e()` source with folded `has()` conditions read only the matching elements.
//!
//! An index is a catalog definition of kind [`DefinitionKind::PROPERTY_INDEX`]
//! ([`PropertyIndexDefinition`]), so it travels with the store like every definition (WAL
//! `SetDefinition`, snapshots, backups, forks, rollback). Its ENTRIES are never persisted: the
//! graph derives them from the data, like the label index and the statistics:
//!
//! - every mutation path of [`TraversalGraph`] that changes what an index holds (adding,
//!   dropping, relabelling an element, writing or removing a top-level property, a jpath write
//!   that changes its top-level key, and the undo of each of them) takes the element's entries
//!   out before the change and puts them back after it (`pindex_vertex` / `pindex_edge`); with
//!   no index declared that is one branch;
//! - a define, redefine, drop (and the undo of each) rebuilds that one index from the data;
//! - a load (`finish_load`) rebuilds every index after the elements and the catalog are in
//!   place, so a read-only open, a backup open and a recovery are consistent with the data.
//!
//! Keys are the LOGICAL scalar values (`null`, booleans, numbers, strings, uuids, dates and
//! times; a compressed string by its text). Arrays and objects are not indexed: a condition on one is never served
//! by an index. The index is ordered: an equality, `P.within` and the ranges `P.gt/gte/lt/lte/
//! inside/between/outside` read a slice of it. A lookup returns a SUPERSET of the matching
//! elements (numbers are ranged by their `float64` value, both `int64` and `float64` entries
//! included); the source re-checks every condition on each candidate, so a result never differs
//! from the scan's.

use std::collections::{BTreeSet, HashMap};
use std::ops::Bound;

use compact_str::CompactString;

use crate::error::{DefinitionError, GraphError};
use crate::graph::definition::{Definition, DefinitionBody, DefinitionFlags, DefinitionKind};
use crate::graph::schema::ElementKind;
use crate::graph::{EdgeHandle, EdgeIndex, NodeIndex, TraversalGraph, VertexHandle};
use crate::value::number::{N, canonicalize_f64};
use crate::value::temporal::Duration;
use crate::value::{ElementProperty, PropertyMap, Temporal};

/// The longest label of an index, in bytes.
pub const MAX_INDEX_LABEL_BYTES: usize = 1024;
/// The longest property key of an index, in bytes.
pub const MAX_INDEX_KEY_BYTES: usize = 1024;

// ---------------------------------------------------------------------------------------------
// The definition
// ---------------------------------------------------------------------------------------------

/// A property index (catalog kind [`DefinitionKind::PROPERTY_INDEX`]): the `element`s labelled
/// `label`, ordered by the value of their top-level property `key`.
///
/// ```
/// use graphersal::catalog::PropertyIndexDefinition;
/// use graphersal::storage::ElementKind;
///
/// let index = PropertyIndexDefinition::new(ElementKind::Vertex, "person", "title");
/// assert_eq!(index.label(), "person");
/// assert_eq!(index.key(), "title");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyIndexDefinition {
    element: ElementKind,
    label: String,
    key: String,
    extra: PropertyMap,
}

impl PropertyIndexDefinition {
    /// An index of the `element`s labelled `label` on their top-level property `key` (a plain
    /// name, never a jpath). Checked by [`Definition::validate`].
    pub fn new(element: ElementKind, label: impl Into<String>, key: impl Into<String>) -> Self {
        PropertyIndexDefinition {
            element,
            label: label.into(),
            key: key.into(),
            extra: PropertyMap::default(),
        }
    }

    /// The element kind the index covers.
    pub fn element(&self) -> ElementKind {
        self.element
    }

    /// The label the index covers (it need not exist yet).
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The top-level property key the index orders by.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The name an index gets when none is given (the DSL's `g.create_index` without `name`,
    /// the playground's form): `<label>_<key>` (`<label>_<key>_edge` for an edge index), every
    /// character that is not an ASCII letter, digit or `_` replaced by `_`, prefixed with
    /// `index_` when it would not start with a letter.
    ///
    /// ```
    /// use graphersal::catalog::PropertyIndexDefinition;
    /// use graphersal::storage::ElementKind;
    ///
    /// let index = PropertyIndexDefinition::new(ElementKind::Edge, "knows", "weight");
    /// assert_eq!(index.default_name(), "knows_weight_edge");
    /// let index = PropertyIndexDefinition::new(ElementKind::Vertex, "2nd-tier", "e-mail");
    /// assert_eq!(index.default_name(), "index_2nd_tier_e_mail");
    /// ```
    pub fn default_name(&self) -> String {
        let mut name: String = format!("{}_{}", self.label, self.key)
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        if self.element == ElementKind::Edge {
            name.push_str("_edge");
        }
        if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
            name.insert_str(0, "index_");
        }
        name
    }

    /// Payload keys this version does not know, kept and written back.
    pub fn extra(&self) -> &PropertyMap {
        &self.extra
    }

    /// The payload property map (format spec 6.3), in this key order: `element` (`"vertex"` or
    /// `"edge"`), `label`, `key`, then the unknown keys.
    pub fn to_payload(&self) -> PropertyMap {
        let mut map = PropertyMap::default();
        map.insert("element", self.element.as_str().into());
        map.insert("label", self.label.as_str().into());
        map.insert("key", self.key.as_str().into());
        for (key, value) in &self.extra {
            map.0.entry(key.clone()).or_insert_with(|| value.clone());
        }
        map
    }

    /// Reads a payload map (the inverse of [`Self::to_payload`]). A missing `element` is
    /// `"vertex"`; a known key of the wrong type is an error; unknown keys are kept. Not
    /// validated.
    pub fn from_payload(payload: &PropertyMap) -> Result<PropertyIndexDefinition, DefinitionError> {
        Self::read_fields(payload, &[])
    }

    pub(crate) fn read_fields(
        map: &PropertyMap,
        skip: &[&str],
    ) -> Result<PropertyIndexDefinition, DefinitionError> {
        let malformed = |key: &str, reason: &str| DefinitionError::Malformed {
            reason: format!("/{key}: {reason}"),
        };
        let text = |key: &str| -> Result<Option<String>, DefinitionError> {
            match map.get(key) {
                None => Ok(None),
                Some(value) => value
                    .text()
                    .map(|text| Some(text.into_owned()))
                    .ok_or_else(|| malformed(key, "must be a string")),
            }
        };
        let element = match text("element")?.as_deref() {
            None | Some("vertex") => ElementKind::Vertex,
            Some("edge") => ElementKind::Edge,
            Some(other) => {
                return Err(malformed(
                    "element",
                    &format!("'{other}' is not \"vertex\" or \"edge\""),
                ));
            }
        };
        let label = text("label")?.ok_or_else(|| DefinitionError::Malformed {
            reason: "'label' is missing".to_string(),
        })?;
        let key = text("key")?.ok_or_else(|| DefinitionError::Malformed {
            reason: "'key' is missing".to_string(),
        })?;
        let mut index = PropertyIndexDefinition::new(element, label, key);
        const KNOWN: [&str; 3] = ["element", "label", "key"];
        for (key, value) in map {
            if !KNOWN.contains(&key.as_str()) && !skip.contains(&key.as_str()) {
                index.extra.insert(key.clone(), value.to_logical());
            }
        }
        Ok(index)
    }

    /// Writes the exchange-form fields (the payload keys) into `map`.
    pub(crate) fn write_fields(&self, map: &mut PropertyMap) {
        for (key, value) in self.to_payload() {
            map.insert(key, value);
        }
    }

    /// The rules of [`Definition::validate`] for an index named `name`.
    pub(crate) fn validate(&self, name: &str) -> Result<(), DefinitionError> {
        let field = |field: &str, reason: String| DefinitionError::InvalidField {
            name: name.to_string(),
            field: field.to_string(),
            reason,
        };
        if self.label.is_empty() {
            return Err(field("label", "it is empty".to_string()));
        }
        if self.label.len() > MAX_INDEX_LABEL_BYTES {
            return Err(field(
                "label",
                format!("it is longer than {MAX_INDEX_LABEL_BYTES} bytes"),
            ));
        }
        if self.label.contains("::") {
            return Err(field(
                "label",
                "a label cannot contain '::' (the multi-label separator)".to_string(),
            ));
        }
        if self.key.is_empty() {
            return Err(field(
                "key",
                "it is empty; name the top-level property, for example \"title\"".to_string(),
            ));
        }
        if self.key.len() > MAX_INDEX_KEY_BYTES {
            return Err(field(
                "key",
                format!("it is longer than {MAX_INDEX_KEY_BYTES} bytes"),
            ));
        }
        if let Some(key) = self
            .extra
            .keys()
            .find(|key| crate::graph::definition::RESERVED_PAYLOAD_KEYS.contains(&key.as_str()))
        {
            return Err(field(
                "payload",
                format!("the payload key '{key}' is reserved"),
            ));
        }
        Ok(())
    }
}

impl Definition {
    /// A property index named `name` (kind [`DefinitionKind::PROPERTY_INDEX`], not critical:
    /// the index holds no data of its own, a reader that does not know the kind loses nothing
    /// but the speed-up).
    pub fn property_index(name: impl Into<String>, index: PropertyIndexDefinition) -> Self {
        Definition::from_parts(
            DefinitionKind::PROPERTY_INDEX,
            DefinitionFlags::NONE,
            name.into(),
            DefinitionBody::PropertyIndex(index),
        )
    }

    /// The property index, when this is a decoded one.
    pub fn as_property_index(&self) -> Option<&PropertyIndexDefinition> {
        match self.body() {
            DefinitionBody::PropertyIndex(index) => Some(index),
            DefinitionBody::Query(_)
            | DefinitionBody::Compression(_)
            | DefinitionBody::Opaque(_) => None,
        }
    }
}

/// Checks a property index definition against the catalog it is about to join: no other index
/// of the same `(element, label, key)`.
pub(crate) fn check_conflict(
    graph: &TraversalGraph,
    definition: &Definition,
) -> Result<(), GraphError> {
    let Some(index) = definition.as_property_index() else {
        return Ok(());
    };
    for other in graph.definitions.iter() {
        if other.kind() != DefinitionKind::PROPERTY_INDEX || other.name() == definition.name() {
            continue;
        }
        if let Some(existing) = other.as_property_index()
            && existing.element() == index.element()
            && existing.label() == index.label()
            && existing.key() == index.key()
        {
            return Err(DefinitionError::Conflict {
                kind: "property index".to_string(),
                name: definition.name().to_string(),
                existing: other.name().to_string(),
                reason: format!(
                    "the index '{}' already covers {} '{}' on '{}'",
                    other.name(),
                    index.element().as_str(),
                    index.label(),
                    index.key()
                ),
            }
            .into());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Public read types
// ---------------------------------------------------------------------------------------------

/// What one property index holds right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PropertyIndexStats {
    /// Indexed elements (one entry per element with a scalar value at the key).
    pub entries: u64,
    /// Distinct indexed values.
    pub distinct_values: u64,
    /// An estimate of the index's heap footprint, in bytes.
    pub bytes: u64,
}

impl PropertyIndexStats {
    /// Stats with these figures (for a storage outside this crate).
    pub fn new(entries: u64, distinct_values: u64, bytes: u64) -> Self {
        PropertyIndexStats {
            entries,
            distinct_values,
            bytes,
        }
    }
}

/// A property index a storage serves ([`GraphStorage::property_indexes`]): its name, what it
/// covers and its current stats.
///
/// [`GraphStorage::property_indexes`]: crate::graph::GraphStorage::property_indexes
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PropertyIndexInfo {
    /// The name of the index definition.
    pub name: String,
    /// The element kind it covers.
    pub element: ElementKind,
    /// The label it covers.
    pub label: String,
    /// The top-level property key it orders by.
    pub key: String,
    /// What it holds right now.
    pub stats: PropertyIndexStats,
}

impl PropertyIndexInfo {
    /// An entry with these parts (for a storage outside this crate).
    pub fn new(
        name: impl Into<String>,
        element: ElementKind,
        label: impl Into<String>,
        key: impl Into<String>,
        stats: PropertyIndexStats,
    ) -> Self {
        PropertyIndexInfo {
            name: name.into(),
            element,
            label: label.into(),
            key: key.into(),
            stats,
        }
    }
}

/// The slice of a property index a lookup reads
/// ([`GraphStorage::vertices_by_property_index`]): the values between `lower` and `upper`.
///
/// The bounds are scalar values (`null`, boolean, number, string, uuid, or one temporal type:
/// `date`, `localdatetime`, `datetime`, `time`, `duration`) of ONE kind; values of other kinds
/// are never in the slice. Numbers compare by value across `int64` and `float64`, strings
/// bytewise, booleans `false < true`, uuids by their 128-bit word, a `datetime` by its instant
/// (the offset is not compared), a `duration` by its length then its fields, as the predicates
/// do.
/// A storage may return MORE elements than the slice holds (the caller re-checks every
/// condition on each candidate), never fewer. An equality ([`Self::equal`]) is the slice
/// `[v, v]`.
///
/// [`GraphStorage::vertices_by_property_index`]: crate::graph::GraphStorage::vertices_by_property_index
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PropertyIndexRange {
    /// The lower bound.
    pub lower: Bound<ElementProperty>,
    /// The upper bound.
    pub upper: Bound<ElementProperty>,
}

impl PropertyIndexRange {
    /// The slice between `lower` and `upper`.
    pub fn new(lower: Bound<ElementProperty>, upper: Bound<ElementProperty>) -> Self {
        PropertyIndexRange { lower, upper }
    }

    /// The values equal to `value`.
    pub fn equal(value: ElementProperty) -> Self {
        PropertyIndexRange {
            lower: Bound::Included(value.clone()),
            upper: Bound::Included(value),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The ordered key
// ---------------------------------------------------------------------------------------------

/// The ordered key of an index entry: a logical scalar. Kinds are ordered `null < boolean <
/// number < string < uuid < date < localdatetime < datetime < time < duration`; within a kind
/// the order of the predicates (numbers by their canonical `float64` value first, then `int64`
/// before `float64`, then the exact `int64`; temporal values by `Temporal`'s own order).
///
/// Equal keys are exactly the values `has()` finds equal: each temporal type is its own kind
/// (a `date` never equals a `datetime` or a number), a `datetime` key is its INSTANT
/// (`12:00Z` and `14:00+02:00` are one key), and a `duration` key is the whole [`Duration`],
/// whose equality is field by field (`P1M` is not `P30D`) and whose order is its length (a month
/// as 30 days) with ties broken by the fields: an order consistent with that equality, so
/// equality, `P.within` and ranges on durations are all exact slices.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum IndexKey {
    Null,
    Bool(bool),
    /// `(canonical float64 value, 0 = int64 / 1 = float64, the int64 or 0)`.
    Number(crate::value::number::OrderedF64, u8, i64),
    Str(CompactString),
    /// The uuid's 128-bit word as `(high, low)` (keeps the key 8-aligned).
    Uuid(u64, u64),
    /// Days since 1970-01-01.
    Date(i32),
    /// Wall-clock microseconds since 1970-01-01T00:00:00.
    LocalDateTime(i64),
    /// The instant: microseconds since 1970-01-01T00:00:00Z (the offset is not part of the key).
    DateTime(i64),
    /// Microseconds after midnight.
    Time(i64),
    /// The duration itself (its `Ord` is the length, then the fields).
    Duration(Duration),
}

/// The kinds of [`IndexKey`], in key order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyKind {
    Null,
    Bool,
    Number,
    Str,
    Uuid,
    Date,
    LocalDateTime,
    DateTime,
    Time,
    Duration,
}

impl IndexKey {
    /// The key of a stored (or queried) value; `None` for a value that is not indexed (arrays,
    /// objects, elements, maps).
    fn of(value: &ElementProperty) -> Option<IndexKey> {
        match value {
            ElementProperty::Null => Some(IndexKey::Null),
            ElementProperty::Boolean(flag) => Some(IndexKey::Bool(*flag)),
            ElementProperty::Number(number) => Some(match &number.n {
                N::NegInt(int) => IndexKey::Number(canonicalize_f64(*int as f64), 0, *int),
                N::Float(float) => IndexKey::Number(canonicalize_f64(float.0), 1, 0),
            }),
            ElementProperty::String(text) => Some(IndexKey::Str(text.clone())),
            ElementProperty::Compressed(text) => Some(IndexKey::Str(text.text())),
            ElementProperty::Uuid(bits) => Some(IndexKey::Uuid((*bits >> 64) as u64, *bits as u64)),
            ElementProperty::Temporal(value) => Some(match *value {
                Temporal::Date(date) => IndexKey::Date(date.days_since_epoch()),
                Temporal::LocalDateTime(local) => IndexKey::LocalDateTime(local.micros()),
                Temporal::DateTime(instant) => IndexKey::DateTime(instant.epoch_micros()),
                Temporal::Time(time) => IndexKey::Time(time.micros_of_day()),
                Temporal::Duration(duration) => IndexKey::Duration(duration),
            }),
            ElementProperty::Object(_)
            | ElementProperty::Array(_)
            | ElementProperty::Vertex(_)
            | ElementProperty::Edge(_)
            | ElementProperty::Map(_) => None,
        }
    }

    fn kind(&self) -> KeyKind {
        match self {
            IndexKey::Null => KeyKind::Null,
            IndexKey::Bool(_) => KeyKind::Bool,
            IndexKey::Number(..) => KeyKind::Number,
            IndexKey::Str(_) => KeyKind::Str,
            IndexKey::Uuid(..) => KeyKind::Uuid,
            IndexKey::Date(_) => KeyKind::Date,
            IndexKey::LocalDateTime(_) => KeyKind::LocalDateTime,
            IndexKey::DateTime(_) => KeyKind::DateTime,
            IndexKey::Time(_) => KeyKind::Time,
            IndexKey::Duration(_) => KeyKind::Duration,
        }
    }

    /// The heap bytes the key owns.
    fn heap_bytes(&self) -> u64 {
        match self {
            IndexKey::Str(text) if text.is_heap_allocated() => text.capacity() as u64,
            _ => 0,
        }
    }
}

/// Whether a value can be an index key (a scalar: `null`, boolean, number, string, uuid, a date
/// or time); a
/// condition on any other value is never served by an index.
pub(crate) fn is_indexable(value: &ElementProperty) -> bool {
    IndexKey::of(value).is_some()
}

/// The first entry of a key kind (inclusive).
fn kind_start(kind: KeyKind) -> (IndexKey, u64) {
    let key = match kind {
        KeyKind::Null => IndexKey::Null,
        KeyKind::Bool => IndexKey::Bool(false),
        KeyKind::Number => IndexKey::Number(canonicalize_f64(f64::NEG_INFINITY), 0, i64::MIN),
        KeyKind::Str => IndexKey::Str(CompactString::default()),
        KeyKind::Uuid => IndexKey::Uuid(0, 0),
        KeyKind::Date => IndexKey::Date(i32::MIN),
        KeyKind::LocalDateTime => IndexKey::LocalDateTime(i64::MIN),
        KeyKind::DateTime => IndexKey::DateTime(i64::MIN),
        KeyKind::Time => IndexKey::Time(i64::MIN),
        // Every part at its minimum is the shortest length and the smallest fields.
        KeyKind::Duration => IndexKey::Duration(Duration::new(i32::MIN, i32::MIN, i64::MIN)),
    };
    (key, 0)
}

/// The bound just past a key kind: the first entry of the next kind, excluded.
fn kind_end(kind: KeyKind) -> Bound<(IndexKey, u64)> {
    match kind {
        KeyKind::Null => Bound::Excluded(kind_start(KeyKind::Bool)),
        KeyKind::Bool => Bound::Excluded(kind_start(KeyKind::Number)),
        KeyKind::Number => Bound::Excluded(kind_start(KeyKind::Str)),
        KeyKind::Str => Bound::Excluded(kind_start(KeyKind::Uuid)),
        KeyKind::Uuid => Bound::Excluded(kind_start(KeyKind::Date)),
        KeyKind::Date => Bound::Excluded(kind_start(KeyKind::LocalDateTime)),
        KeyKind::LocalDateTime => Bound::Excluded(kind_start(KeyKind::DateTime)),
        KeyKind::DateTime => Bound::Excluded(kind_start(KeyKind::Time)),
        KeyKind::Time => Bound::Excluded(kind_start(KeyKind::Duration)),
        KeyKind::Duration => Bound::Unbounded,
    }
}

/// The set bounds of a range, `None` when the range holds nothing (bounds of different kinds,
/// a non-scalar bound, no bound at all, or a lower bound past the upper one).
type SetBounds = (Bound<(IndexKey, u64)>, Bound<(IndexKey, u64)>);

fn set_bounds(range: &PropertyIndexRange) -> Option<SetBounds> {
    let bound_key = |bound: &Bound<ElementProperty>| -> Option<Option<(IndexKey, bool)>> {
        match bound {
            Bound::Included(value) => IndexKey::of(value).map(|key| Some((key, true))),
            Bound::Excluded(value) => IndexKey::of(value).map(|key| Some((key, false))),
            Bound::Unbounded => Some(None),
        }
    };
    let lower = bound_key(&range.lower)?;
    let upper = bound_key(&range.upper)?;
    let kind = match (&lower, &upper) {
        (Some((a, _)), Some((b, _))) if a.kind() != b.kind() => return None,
        (Some((a, _)), _) => a.kind(),
        (None, Some((b, _))) => b.kind(),
        (None, None) => return None,
    };
    let start = match lower {
        None => Bound::Included(kind_start(kind)),
        // Numbers: every entry of the same float64 value, int64 and float64 alike (a superset;
        // the caller re-checks).
        Some((IndexKey::Number(value, _, _), _)) => {
            Bound::Included((IndexKey::Number(value, 0, i64::MIN), 0))
        }
        Some((key, true)) => Bound::Included((key, 0)),
        Some((key, false)) => Bound::Excluded((key, u64::MAX)),
    };
    let end = match upper {
        None => kind_end(kind),
        Some((IndexKey::Number(value, _, _), _)) => {
            Bound::Included((IndexKey::Number(value, 1, i64::MAX), u64::MAX))
        }
        Some((key, true)) => Bound::Included((key, u64::MAX)),
        Some((key, false)) => Bound::Excluded((key, 0)),
    };
    // `BTreeSet::range` panics on a start past the end: such a range holds nothing.
    let empty = match (&start, &end) {
        (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b))
            if a > b =>
        {
            true
        }
        (Bound::Excluded(a), Bound::Excluded(b)) if a == b => true,
        (Bound::Included(a), Bound::Excluded(b)) | (Bound::Excluded(a), Bound::Included(b))
            if a == b =>
        {
            true
        }
        _ => false,
    };
    (!empty).then_some((start, end))
}

// ---------------------------------------------------------------------------------------------
// The in-memory state
// ---------------------------------------------------------------------------------------------

/// A handle packed into one word: slot index in the high half, generation in the low half.
fn pack(slot: usize, generation: std::num::NonZeroU32) -> u64 {
    ((slot as u64) << 32) | u64::from(generation.get())
}

fn unpack(word: u64) -> Option<(usize, std::num::NonZeroU32)> {
    let generation = std::num::NonZeroU32::new(word as u32)?;
    Some(((word >> 32) as usize, generation))
}

/// One active index: its entries, ordered by key then handle.
#[derive(Debug, Clone)]
struct IndexState {
    name: CompactString,
    element: ElementKind,
    label: CompactString,
    key: CompactString,
    entries: BTreeSet<(IndexKey, u64)>,
    distinct: u64,
    heap: u64,
}

impl IndexState {
    fn insert(&mut self, key: IndexKey, handle: u64) {
        let heap = key.heap_bytes();
        let fresh = !self.has_key(&key);
        if self.entries.insert((key, handle)) {
            self.heap += heap;
            if fresh {
                self.distinct += 1;
            }
        }
    }

    fn remove(&mut self, key: IndexKey, handle: u64) {
        let heap = key.heap_bytes();
        let entry = (key, handle);
        if self.entries.remove(&entry) {
            self.heap = self.heap.saturating_sub(heap);
            if !self.has_key(&entry.0) {
                self.distinct = self.distinct.saturating_sub(1);
            }
        }
    }

    /// Whether some entry has this key.
    fn has_key(&self, key: &IndexKey) -> bool {
        self.entries
            .range((
                Bound::Included((key.clone(), 0)),
                Bound::Included((key.clone(), u64::MAX)),
            ))
            .next()
            .is_some()
    }

    fn stats(&self) -> PropertyIndexStats {
        let entry = std::mem::size_of::<(IndexKey, u64)>() as u64 + 8;
        PropertyIndexStats {
            entries: self.entries.len() as u64,
            distinct_values: self.distinct,
            bytes: self.entries.len() as u64 * entry + self.heap,
        }
    }
}

/// The indexes of a graph (derived from the catalog), with lookups by element kind and label.
#[derive(Debug, Clone, Default)]
pub(crate) struct PropertyIndexes {
    indexes: Vec<IndexState>,
    vertex: HashMap<CompactString, Vec<usize>>,
    edge: HashMap<CompactString, Vec<usize>>,
}

impl PropertyIndexes {
    /// Whether no index exists at all.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.indexes.is_empty()
    }

    fn by_label(&self, element: ElementKind) -> &HashMap<CompactString, Vec<usize>> {
        match element {
            ElementKind::Vertex => &self.vertex,
            ElementKind::Edge => &self.edge,
        }
    }

    fn reindex_lookup(&mut self) {
        self.vertex.clear();
        self.edge.clear();
        for (position, index) in self.indexes.iter().enumerate() {
            let map = match index.element {
                ElementKind::Vertex => &mut self.vertex,
                ElementKind::Edge => &mut self.edge,
            };
            map.entry(index.label.clone()).or_default().push(position);
        }
    }

    /// Takes the entries of one element out (`insert` false) or puts them in (`insert` true):
    /// the element's labels and properties as stored; `only_key` limits it to one top-level key.
    fn element_entries<'l>(
        &mut self,
        element: ElementKind,
        insert: bool,
        handle: u64,
        labels: impl Iterator<Item = &'l str>,
        properties: Option<&PropertyMap>,
        only_key: Option<&str>,
    ) {
        let Some(properties) = properties else {
            return;
        };
        let by_label = match element {
            ElementKind::Vertex => &self.vertex,
            ElementKind::Edge => &self.edge,
        };
        for label in labels {
            let Some(positions) = by_label.get(label) else {
                continue;
            };
            for position in positions {
                let Some(index) = self.indexes.get_mut(*position) else {
                    continue;
                };
                if only_key.is_some_and(|key| key != index.key.as_str()) {
                    continue;
                }
                let Some(key) = properties.get(index.key.as_str()).and_then(IndexKey::of) else {
                    continue;
                };
                if insert {
                    index.insert(key, handle);
                } else {
                    index.remove(key, handle);
                }
            }
        }
    }

    /// [`Self::element_entries`] of a vertex (borrows only this state, so a caller holding the
    /// vertex's weight can use it).
    pub(crate) fn vertex_entries(
        &mut self,
        insert: bool,
        node: VertexHandle,
        labels: &[CompactString],
        properties: &PropertyMap,
        only_key: Option<&str>,
    ) {
        self.element_entries(
            ElementKind::Vertex,
            insert,
            pack(node.index().index(), node.generation()),
            labels.iter().map(CompactString::as_str),
            Some(properties),
            only_key,
        );
    }

    /// [`Self::element_entries`] of an edge.
    pub(crate) fn edge_entries(
        &mut self,
        insert: bool,
        edge: EdgeHandle,
        label: Option<&str>,
        properties: Option<&PropertyMap>,
        only_key: Option<&str>,
    ) {
        self.element_entries(
            ElementKind::Edge,
            insert,
            pack(edge.index().index(), edge.generation()),
            label.into_iter(),
            properties,
            only_key,
        );
    }

    /// The position of the index on `(element, label, key)`.
    fn find(&self, element: ElementKind, label: &str, key: &str) -> Option<&IndexState> {
        self.by_label(element)
            .get(label)?
            .iter()
            .filter_map(|position| self.indexes.get(*position))
            .find(|index| index.key == key)
    }

    /// The estimated heap footprint of every index, in bytes.
    pub(crate) fn bytes(&self) -> u64 {
        self.indexes.iter().map(|index| index.stats().bytes).sum()
    }
}

// ---------------------------------------------------------------------------------------------
// TraversalGraph: maintenance, rebuild, lookups, check
// ---------------------------------------------------------------------------------------------

impl TraversalGraph {
    /// Takes the index entries of vertex `node` out (`insert` false) or puts them back (`insert`
    /// true), for every index of its labels (`key`: only the indexes on that top-level key).
    /// Called around every mutation that can change what an index holds; one branch without
    /// indexes.
    #[inline]
    pub(crate) fn pindex_vertex(&mut self, node: VertexHandle, insert: bool, key: Option<&str>) {
        if self.property_indexes.is_empty() {
            return;
        }
        self.pindex_vertex_slow(node, insert, key);
    }

    #[inline(never)]
    fn pindex_vertex_slow(&mut self, node: VertexHandle, insert: bool, key: Option<&str>) {
        let Some(vertex) = self
            .graph
            .node_weight(node.index())
            .filter(|vertex| vertex.3 == node.generation())
        else {
            return;
        };
        self.property_indexes
            .vertex_entries(insert, node, &vertex.1, &vertex.2, key);
    }

    /// The edge counterpart of [`Self::pindex_vertex`].
    #[inline]
    pub(crate) fn pindex_edge(&mut self, edge: EdgeHandle, insert: bool, key: Option<&str>) {
        if self.property_indexes.is_empty() {
            return;
        }
        self.pindex_edge_slow(edge, insert, key);
    }

    #[inline(never)]
    fn pindex_edge_slow(&mut self, edge: EdgeHandle, insert: bool, key: Option<&str>) {
        let Some(weight) = self
            .graph
            .edge_weight(edge.index())
            .filter(|weight| weight.3.own == edge.generation())
        else {
            return;
        };
        self.property_indexes.edge_entries(
            insert,
            edge,
            weight.1.as_deref(),
            weight.2.as_ref(),
            key,
        );
    }

    /// Builds the state of one index from the data.
    fn build_index(&self, name: &str, definition: &PropertyIndexDefinition) -> IndexState {
        let mut state = IndexState {
            name: CompactString::from(name),
            element: definition.element(),
            label: CompactString::from(definition.label()),
            key: CompactString::from(definition.key()),
            entries: BTreeSet::new(),
            distinct: 0,
            heap: 0,
        };
        match definition.element() {
            ElementKind::Vertex => {
                if let Some(nodes) = self.index.get_nodes_by_label(definition.label()) {
                    for node in nodes {
                        if let Some(vertex) = self.vertex_weight(node)
                            && let Some(key) = vertex.2.get(definition.key()).and_then(IndexKey::of)
                        {
                            state.insert(key, pack(node.index().index(), node.generation()));
                        }
                    }
                }
            }
            ElementKind::Edge => {
                if let Some(edges) = self.index.get_edges_by_label(definition.label()) {
                    for edge in edges {
                        if let Some(weight) = self.edge_weight(edge)
                            && let Some(key) = weight
                                .2
                                .as_ref()
                                .and_then(|properties| properties.get(definition.key()))
                                .and_then(IndexKey::of)
                        {
                            state.insert(key, pack(edge.index().index(), edge.generation()));
                        }
                    }
                }
            }
        }
        state
    }

    /// After a catalog change of kind property index: rebuilds the index `name` from the data
    /// (or removes it when the catalog no longer holds it).
    pub(crate) fn property_index_after_change(&mut self, name: &str) {
        let definition = self
            .definitions
            .get(DefinitionKind::PROPERTY_INDEX, name)
            .and_then(|definition| definition.as_property_index().cloned());
        let built = definition.map(|definition| self.build_index(name, &definition));
        let indexes = &mut self.property_indexes.indexes;
        match (indexes.iter().position(|index| index.name == name), built) {
            (Some(position), Some(state)) => indexes[position] = state,
            (Some(position), None) => {
                indexes.remove(position);
            }
            (None, Some(state)) => indexes.push(state),
            (None, None) => return,
        }
        self.property_indexes.reindex_lookup();
    }

    /// Rebuilds every index from the catalog and the data (a load: read-only opens, backups and
    /// recoveries are consistent with the data).
    #[cfg_attr(not(feature = "persist"), allow(dead_code))]
    pub(crate) fn property_index_rebuild_all(&mut self) {
        let definitions: Vec<(String, PropertyIndexDefinition)> = self
            .definitions
            .iter()
            .filter_map(|definition| {
                definition
                    .as_property_index()
                    .map(|index| (definition.name().to_string(), index.clone()))
            })
            .collect();
        let indexes: Vec<IndexState> = definitions
            .iter()
            .map(|(name, definition)| self.build_index(name, definition))
            .collect();
        self.property_indexes = PropertyIndexes {
            indexes,
            ..PropertyIndexes::default()
        };
        self.property_indexes.reindex_lookup();
    }

    /// Every index this graph serves, with its stats, by catalog order.
    pub(crate) fn property_index_infos(&self) -> Vec<PropertyIndexInfo> {
        self.property_indexes
            .indexes
            .iter()
            .map(|index| PropertyIndexInfo {
                name: index.name.to_string(),
                element: index.element,
                label: index.label.to_string(),
                key: index.key.to_string(),
                stats: index.stats(),
            })
            .collect()
    }

    /// The entries of the index on `(element, label, key)` inside `range`, as packed handles;
    /// `None` when there is no such index.
    fn index_slice<'a>(
        &'a self,
        element: ElementKind,
        label: &str,
        key: &str,
        range: &PropertyIndexRange,
    ) -> Option<Box<dyn Iterator<Item = (usize, std::num::NonZeroU32)> + 'a>> {
        let index = self.property_indexes.find(element, label, key)?;
        Some(match set_bounds(range) {
            Some(bounds) => Box::new(
                index
                    .entries
                    .range(bounds)
                    .filter_map(|(_, word)| unpack(*word)),
            ),
            None => Box::new(std::iter::empty()),
        })
    }

    /// The vertices labelled `label` whose `key` lies in `range` (a superset), from the index on
    /// `(vertex, label, key)`; `None` without such an index.
    pub(crate) fn property_index_vertices<'a>(
        &'a self,
        label: &str,
        key: &str,
        range: &PropertyIndexRange,
    ) -> Option<Box<dyn Iterator<Item = VertexHandle> + 'a>> {
        let slice = self.index_slice(ElementKind::Vertex, label, key, range)?;
        Some(Box::new(slice.map(|(slot, generation)| {
            VertexHandle::new(NodeIndex::new(slot), generation)
        })))
    }

    /// The edge counterpart of [`Self::property_index_vertices`].
    pub(crate) fn property_index_edges<'a>(
        &'a self,
        label: &str,
        key: &str,
        range: &PropertyIndexRange,
    ) -> Option<Box<dyn Iterator<Item = EdgeHandle> + 'a>> {
        let slice = self.index_slice(ElementKind::Edge, label, key, range)?;
        Some(Box::new(slice.map(|(slot, generation)| {
            EdgeHandle::new(EdgeIndex::new(slot), generation)
        })))
    }

    /// Compares every maintained index with a fresh rebuild from the data and the catalog:
    /// `Err` names the first index that differs. A test helper; it walks every index's label.
    #[doc(hidden)]
    pub fn check_property_indexes(&self) -> Result<(), String> {
        let mut expected: Vec<(String, PropertyIndexDefinition)> = self
            .definitions
            .iter()
            .filter_map(|definition| {
                definition
                    .as_property_index()
                    .map(|index| (definition.name().to_string(), index.clone()))
            })
            .collect();
        expected.sort_by(|a, b| a.0.cmp(&b.0));
        let mut actual: Vec<&IndexState> = self.property_indexes.indexes.iter().collect();
        actual.sort_by(|a, b| a.name.cmp(&b.name));
        if expected.len() != actual.len() {
            return Err(format!(
                "{} indexes maintained, the catalog declares {}",
                actual.len(),
                expected.len()
            ));
        }
        for ((name, definition), state) in expected.iter().zip(actual) {
            let fresh = self.build_index(name, definition);
            if state.name != fresh.name
                || state.element != fresh.element
                || state.label != fresh.label
                || state.key != fresh.key
            {
                return Err(format!("index '{name}': its definition differs"));
            }
            if state.entries != fresh.entries {
                return Err(format!(
                    "index '{name}': {} entries maintained, {} after a rebuild (or other values)",
                    state.entries.len(),
                    fresh.entries.len()
                ));
            }
            if state.distinct != fresh.distinct {
                return Err(format!(
                    "index '{name}': {} distinct values counted, {} after a rebuild",
                    state.distinct, fresh.distinct
                ));
            }
        }
        // The lookup tables match the indexes.
        let mut lookup = self.property_indexes.clone();
        lookup.reindex_lookup();
        if lookup.vertex != self.property_indexes.vertex
            || lookup.edge != self.property_indexes.edge
        {
            return Err("the label lookup of the indexes is stale".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphStorage;

    fn int(n: i64) -> ElementProperty {
        ElementProperty::from(n)
    }

    #[test]
    fn keys_order_kinds_and_numbers() {
        let one = IndexKey::of(&int(1)).unwrap();
        let one_f = IndexKey::of(&ElementProperty::from(1.0)).unwrap();
        let two = IndexKey::of(&int(2)).unwrap();
        assert!(one < one_f && one_f < two);
        assert!(IndexKey::of(&ElementProperty::Null).unwrap() < one);
        assert!(two < IndexKey::of(&ElementProperty::from("a")).unwrap());
        assert!(IndexKey::of(&ElementProperty::Array(vec![])).is_none());
    }

    #[test]
    fn temporal_keys_are_one_kind_per_type_and_follow_has() {
        let key = |text: &str, kind: crate::value::temporal::TemporalKind| {
            IndexKey::of(&ElementProperty::Temporal(
                Temporal::parse_as(kind, text).unwrap(),
            ))
            .unwrap()
        };
        use crate::value::temporal::TemporalKind as K;
        let noon = key("2023-08-02T12:00:00Z", K::DateTime);
        assert_eq!(noon, key("2023-08-02T14:00:00+02:00", K::DateTime));
        assert_ne!(key("P1M", K::Duration), key("P30D", K::Duration));
        assert!(key("PT720H", K::Duration) < key("P30D", K::Duration));
        assert!(key("P30D", K::Duration) < key("P1M", K::Duration));
        assert!(key("P1M", K::Duration) < key("P30DT1S", K::Duration));
        // Kinds in order, each inside its own slice.
        let kinds = [
            key("9999-12-31", K::Date),
            key("-9999-01-01T00:00:00", K::LocalDateTime),
            noon,
            key("00:00:00", K::Time),
            key("P-100Y", K::Duration),
        ];
        assert!(IndexKey::Uuid(u64::MAX, u64::MAX) < kinds[0]);
        for pair in kinds.windows(2) {
            assert!(pair[0] < pair[1]);
            assert_ne!(pair[0].kind(), pair[1].kind());
        }
        for key in &kinds {
            let start = kind_start(key.kind());
            assert!(start <= (key.clone(), 0));
            match kind_end(key.kind()) {
                Bound::Excluded(end) => assert!((key.clone(), u64::MAX) < end),
                _ => assert_eq!(key.kind(), KeyKind::Duration),
            }
        }
    }

    #[test]
    fn inverted_and_mixed_ranges_hold_nothing() {
        let range = PropertyIndexRange::new(Bound::Included(int(5)), Bound::Excluded(int(1)));
        assert!(set_bounds(&range).is_none());
        let range = PropertyIndexRange::new(Bound::Included(int(5)), Bound::Excluded("a".into()));
        assert!(set_bounds(&range).is_none());
        let range = PropertyIndexRange::new(Bound::Excluded(int(5)), Bound::Excluded(int(5)));
        assert!(
            set_bounds(&range).is_some(),
            "numbers are widened to their float value"
        );
        let range =
            PropertyIndexRange::new(Bound::Excluded("a".into()), Bound::Excluded("a".into()));
        assert!(set_bounds(&range).is_none());
    }

    #[test]
    fn maintained_through_writes_and_rollback() {
        let mut graph = TraversalGraph::new();
        let mut props = PropertyMap::default();
        props.insert("age", int(29));
        let v = graph
            .insert_vertex(None, vec!["person".into()], props)
            .unwrap();
        graph
            .set_definition(Definition::property_index(
                "age",
                PropertyIndexDefinition::new(ElementKind::Vertex, "person", "age"),
            ))
            .unwrap();
        graph.check_property_indexes().unwrap();
        let found: Vec<_> = graph
            .property_index_vertices("person", "age", &PropertyIndexRange::equal(int(29)))
            .unwrap()
            .collect();
        assert_eq!(found, vec![v]);
        let mark = graph.begin_unit();
        graph.set_vertex_property(v, "age".into(), int(30)).unwrap();
        graph.check_property_indexes().unwrap();
        graph.rollback_unit(mark, &crate::graph::RollbackReason::failed(&"test"));
        graph.check_property_indexes().unwrap();
        graph.drop_vertex(v).unwrap();
        graph.check_property_indexes().unwrap();
    }
}
