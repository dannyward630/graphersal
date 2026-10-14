//! Memory footprint inspection: [`MemoryStats`] and the computation behind
//! [`crate::graph::GraphStorage::memory_usage`]'s `TraversalGraph` implementation.
//!
//! This is entirely separate from traversal execution (see `.profile()`'s `bytes_materialized`
//! in `traversal/step/profile.rs` for the other, execution-scoped question). `memory_usage()`
//! takes `&self` and never touches `TraverserContext`/`ProfileContext`/any query path. The only
//! caller on the execution path is the estimated memory budget (`memory.limit` without a
//! tracking allocator), once per run and through the cache of `footprint_bytes`, which walks again
//! only after the graph changed.

/// Snapshot of a graph's approximate in-memory footprint, in bytes, broken down by subsystem.
///
/// Obtained via [`crate::graph::GraphStorage::memory_usage`]. All figures are estimates: they sum
/// `std::mem::size_of`/`size_of_val`-style arithmetic over the structures that back the graph, not
/// a true allocator-level accounting — fragmentation, allocator bookkeeping overhead, and
/// over-allocation from `Vec`/`HashMap` growth are not captured exactly, only via each structure's
/// own `.capacity()` where available.
///
/// `#[non_exhaustive]`: these three fields are, honestly, modeled on [`crate::TraversalGraph`]'s
/// specific internals (a petgraph `StableGraph` plus a `GraphIndex` plus inline `PropertyMap`
/// properties). A genuinely different [`crate::graph::GraphStorage`] backend's memory model might
/// not map onto exactly these three buckets — it may have no separate "index" at all, or an
/// entirely different structure. This is not a settled, backend-agnostic taxonomy; `#[non_exhaustive]`
/// leaves room for a future backend author to propose new fields, or a variant shape, as a
/// semver-minor addition instead of a breaking redesign, once a real second backend exists to
/// design against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MemoryStats {
    /// Vertex and edge storage: an estimate of the underlying petgraph `StableGraph`'s retained
    /// slot capacity (node/edge count times each slot's approximate size), **not** just
    /// `vertex_count() * size_of::<VertexData>()`.
    ///
    /// **Known, documented undercount risk:** `StableGraph` never releases a removed vertex/edge's
    /// slot — it tombstones it to keep other indices stable — so a live-count-based estimate could
    /// be arbitrarily smaller than the graph's actual retained slot count on a graph with heavy
    /// add/remove churn. This field instead uses `StableGraph::capacity()`, which reports the
    /// underlying `Vec`s' raw capacity (including tombstoned slots), specifically to avoid that
    /// undercount. It is still an approximation: petgraph's internal `Node`/`Edge` wrapper types
    /// (`petgraph::graph::Node<N>`/`Edge<E>`) carry bookkeeping beyond the payload (edge-list
    /// links, endpoint indices) which this field's per-slot size accounts for via
    /// `size_of::<petgraph::graph::Node<Option<VertexData>>>()`/`Edge<..>`, since those wrapper
    /// types are public even though their private fields aren't. [`crate::TraversalGraph::shrink_to_fit`]
    /// today only shrinks the index (see [`Self::index_bytes`]); it does nothing to reclaim
    /// `StableGraph`'s tombstoned slots, so a heavily-churned graph should not treat this field as
    /// tight — no operation currently exists to reclaim that space (a future
    /// `TraversalGraph::compact()` rebuilding the underlying graph is a plausible follow-up, not
    /// built here).
    pub structure_bytes: u64,
    /// The label index (`node_labels`/`edge_labels`) and the ID index (`nodes`/`edges`) in
    /// `GraphIndex`, via each collection's `.capacity()` — including every label bucket `Vec`,
    /// not just the outer maps.
    pub index_bytes: u64,
    /// Estimated heap footprint of vertex/edge properties: each element's property map's own
    /// `.capacity()` contribution, plus every stored [`crate::ElementProperty`]'s own owned heap
    /// data, walked recursively via [`crate::ElementProperty::estimated_heap_bytes`].
    ///
    /// **This is the O(n) part of `memory_usage()`** — it visits every vertex, every edge, and
    /// every property. Expected cost is roughly proportional to `vertex_count + edge_count` times
    /// the average property count per element; see the benchmark in `benches/graph.rs` for a
    /// measured number on the `large` (~110k element) fixture.
    pub property_bytes: u64,
    /// Strings stored compressed by compression rules (book page "Compressed Properties").
    /// `property_bytes` counts their stored (compressed) size.
    pub compressed_values: u64,
    /// The plain size of the compressed strings, in bytes.
    pub compressed_plain_bytes: u64,
    /// The stored size of the compressed strings, in bytes (without dictionaries).
    pub compressed_stored_bytes: u64,
    /// The dictionaries of the compression rules, in bytes (each shared dictionary once); part
    /// of [`Self::total_bytes`].
    pub compression_dictionary_bytes: u64,
    /// The entries of the property indexes (book page "Property Indexes"), an estimate; part
    /// of [`Self::total_bytes`]. Not part of [`Self::index_bytes`] (the id and label indexes).
    pub property_index_bytes: u64,
}

impl MemoryStats {
    /// Sum of every category. This is what a caller almost always wants.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = TraversalGraph::new();
    /// let stats = graph.memory_usage().expect("TraversalGraph always supports introspection");
    /// assert_eq!(
    ///     stats.total_bytes(),
    ///     stats.structure_bytes
    ///         + stats.index_bytes
    ///         + stats.property_bytes
    ///         + stats.compression_dictionary_bytes
    ///         + stats.property_index_bytes
    /// );
    /// ```
    pub fn total_bytes(&self) -> u64 {
        self.structure_bytes
            + self.index_bytes
            + self.property_bytes
            + self.compression_dictionary_bytes
            + self.property_index_bytes
    }

    /// [`Self::total_bytes`] as mebibytes (bytes / 1_048_576.0), for human-readable reporting.
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = TraversalGraph::new();
    /// let stats = graph.memory_usage().unwrap();
    /// assert_eq!(stats.total_mb(), stats.total_bytes() as f64 / 1_048_576.0);
    /// ```
    pub fn total_mb(&self) -> f64 {
        self.total_bytes() as f64 / 1_048_576.0
    }
}

/// Computes [`MemoryStats`] for a [`crate::TraversalGraph`]. Lives here, rather than inline in
/// `graph/storage.rs`'s `impl GraphStorage for TraversalGraph`, purely to keep that file's trait
/// impl list uncluttered.
///
/// `structure_bytes` uses `StableGraph::capacity()` — the underlying node/edge `Vec`s' raw
/// capacity, which includes tombstoned (removed-but-not-reclaimed) slots — rather than
/// `node_count()`/`edge_count()`, specifically to avoid undercounting a graph with heavy
/// add/remove churn; see [`MemoryStats::structure_bytes`]'s doc comment for the full reasoning.
/// Each vertex's primary label and multi-label set are counted here too (structural data, not a
/// property), while `property_bytes` walks only each element's `PropertyMap`.
pub(crate) fn compute(graph: &super::TraversalGraph) -> MemoryStats {
    use super::{EdgeData, VertexData};

    let (node_cap, edge_cap) = graph.graph.capacity();
    let node_slot_size = std::mem::size_of::<petgraph::graph::Node<Option<VertexData>>>() as u64;
    let edge_slot_size = std::mem::size_of::<petgraph::graph::Edge<Option<EdgeData>>>() as u64;
    let mut structure_bytes = node_cap as u64 * node_slot_size + edge_cap as u64 * edge_slot_size;

    let mut property_bytes = 0u64;
    let mut compressed = CompressedCount::default();

    for (id, labels, properties, _) in graph.graph.node_weights() {
        if id.is_heap_allocated() {
            structure_bytes += id.capacity() as u64;
        }
        structure_bytes +=
            labels.capacity() as u64 * std::mem::size_of::<compact_str::CompactString>() as u64;
        for label in labels.iter() {
            if label.is_heap_allocated() {
                structure_bytes += label.capacity() as u64;
            }
        }
        property_bytes += crate::value::estimated_element_heap_bytes(properties);
        if !graph.compression.is_empty() {
            properties.values().for_each(|value| compressed.add(value));
        }
    }

    for (id, edge_label, properties, _) in graph.graph.edge_weights() {
        if id.is_heap_allocated() {
            structure_bytes += id.capacity() as u64;
        }
        if let Some(label) = edge_label
            && label.is_heap_allocated()
        {
            structure_bytes += label.capacity() as u64;
        }
        if let Some(properties) = properties {
            property_bytes += crate::value::estimated_element_heap_bytes(properties);
            if !graph.compression.is_empty() {
                properties.values().for_each(|value| compressed.add(value));
            }
        }
    }

    // The slot generation arrays of the generational handles are structure, like the slots.
    structure_bytes += graph.node_generations.heap_bytes() + graph.edge_generations.heap_bytes();

    let index_bytes = graph.index.estimated_heap_bytes();

    MemoryStats {
        structure_bytes,
        index_bytes,
        property_bytes,
        compressed_values: compressed.values,
        compressed_plain_bytes: compressed.plain,
        compressed_stored_bytes: compressed.stored,
        compression_dictionary_bytes: graph.compression.dictionary_bytes(),
        property_index_bytes: graph.property_indexes.bytes(),
    }
}

/// The compressed strings of a walk (only objects can hold one: rule paths name keys).
#[derive(Default)]
struct CompressedCount {
    values: u64,
    plain: u64,
    stored: u64,
}

impl CompressedCount {
    fn add(&mut self, value: &crate::value::ElementProperty) {
        use crate::value::ElementProperty;
        match value {
            ElementProperty::Compressed(text) => {
                self.values += 1;
                self.plain += text.plain_len() as u64;
                self.stored += text.stored_len() as u64;
            }
            ElementProperty::Object(map) => map.values().for_each(|value| self.add(value)),
            _ => {}
        }
    }
}

/// The heap bytes of a `CompactString` (zero when it is stored inline).
fn string_heap(text: &compact_str::CompactString) -> u64 {
    if text.is_heap_allocated() {
        text.capacity() as u64
    } else {
        0
    }
}

/// What one vertex adds to [`compute`]'s figure for a freshly loaded graph (its slot, id,
/// labels, properties, id-index entry and label-bucket entries), without building the graph: a
/// merge checkpoint keeps the manifest's estimate (format spec 6.1, offset 128) current by
/// subtracting the replaced elements and adding the new ones. Like [`compute`], an estimate.
#[cfg_attr(not(feature = "persist"), allow(dead_code))]
pub(crate) fn loaded_vertex_bytes(
    id: &compact_str::CompactString,
    labels: &[compact_str::CompactString],
    properties: &crate::value::PropertyMap,
) -> u64 {
    use std::mem::size_of;
    let slot = size_of::<petgraph::graph::Node<Option<super::VertexData>>>() as u64;
    let index_entry = size_of::<(compact_str::CompactString, super::VertexHandle)>() as u64;
    let mut bytes = slot + index_entry + string_heap(id);
    bytes += labels.len() as u64
        * (size_of::<compact_str::CompactString>() + size_of::<super::VertexHandle>()) as u64;
    bytes += labels.iter().map(string_heap).sum::<u64>();
    bytes + crate::value::estimated_element_heap_bytes(properties)
}

/// The edge counterpart of [`loaded_vertex_bytes`].
#[cfg_attr(not(feature = "persist"), allow(dead_code))]
pub(crate) fn loaded_edge_bytes(
    id: &str,
    label: Option<&str>,
    properties: Option<&crate::value::PropertyMap>,
) -> u64 {
    use std::mem::size_of;
    let slot = size_of::<petgraph::graph::Edge<Option<super::EdgeData>>>() as u64;
    let index_entry = size_of::<(compact_str::CompactString, super::EdgeHandle)>() as u64;
    let heap = |text: &str| -> u64 {
        // A `CompactString` keeps up to 24 bytes inline.
        if text.len() > 24 {
            text.len() as u64
        } else {
            0
        }
    };
    let mut bytes = slot + index_entry + heap(id);
    if let Some(label) = label {
        bytes += heap(label) + size_of::<super::EdgeHandle>() as u64;
    }
    bytes + properties.map_or(0, crate::value::estimated_element_heap_bytes)
}

/// What a cached footprint was taken at: the commit position and the element counts. A
/// committed change advances the commit position; a direct call outside any unit (not counted)
/// that adds or removes elements changes the counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FootprintKey {
    commit_seq: u64,
    vertices: u64,
    edges: u64,
}

/// The cache of [`footprint_bytes`]: the total of the last walk and the position it was taken
/// at. Behind a mutex so that readers under a shared lock can fill it (`TraversalGraph` stays
/// `Sync`); it is locked once per traversal run, never per element.
#[derive(Debug, Default)]
pub(crate) struct FootprintCache {
    entry: parking_lot::Mutex<Option<(FootprintKey, u64)>>,
    /// How many walks [`footprint_bytes`] made (tests count them).
    #[cfg(test)]
    pub(crate) walks: std::sync::atomic::AtomicU64,
}

/// [`compute`]'s total for `graph`, walked again only when the graph changed since the cached
/// walk (the commit position or the element counts differ). A rolled-back unit restores the
/// data and leaves the commit position, so the cached figure stays valid. While an open unit
/// holds uncommitted changes the figure is walked and not cached.
pub(crate) fn footprint_bytes(graph: &super::TraversalGraph) -> u64 {
    let key = FootprintKey {
        commit_seq: graph.commit_seq(),
        vertices: graph.graph.node_count() as u64,
        edges: graph.graph.edge_count() as u64,
    };
    let cacheable = graph.undo.entries().is_empty();
    if cacheable
        && let Some((cached, bytes)) = *graph.footprint.entry.lock()
        && cached == key
    {
        return bytes;
    }
    #[cfg(test)]
    graph
        .footprint
        .walks
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let bytes = compute(graph).total_bytes();
    if cacheable {
        *graph.footprint.entry.lock() = Some((key, bytes));
    }
    bytes
}
