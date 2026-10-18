use crate::graph::{EdgeHandle, VertexHandle};
use compact_str::CompactString;
use fnv::FnvBuildHasher;
use indexmap::{IndexMap, IndexSet};
pub(crate) use petgraph::graph::{EdgeIndex, NodeIndex};
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub(crate) struct GraphIndex {
    nodes: IndexMap<CompactString, VertexHandle, FnvBuildHasher>,
    edges: IndexMap<CompactString, EdgeHandle, FnvBuildHasher>,
    // Each label's members are a set, not a multiset: a vertex/edge appears in a label's bucket
    // at most once. `IndexSet` gives O(1) average insert/`swap_remove`/contains while preserving
    // insertion order (same guarantee `IndexMap` gives the ID index above), so a large bucket's
    // per-element removal on `drop()` no longer costs a full linear scan of the bucket.
    pub(crate) node_labels:
        HashMap<CompactString, IndexSet<VertexHandle, FnvBuildHasher>, FnvBuildHasher>,
    pub(crate) edge_labels:
        HashMap<CompactString, IndexSet<EdgeHandle, FnvBuildHasher>, FnvBuildHasher>,
}

impl GraphIndex {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn insert_node(&mut self, id: impl Into<CompactString>, index: VertexHandle) {
        self.nodes.insert(id.into(), index);
    }

    #[inline]
    pub fn remove_node(&mut self, id: &str) -> Option<VertexHandle> {
        self.nodes.swap_remove(id)
    }

    #[inline]
    pub fn get_node(&self, id: &str) -> Option<VertexHandle> {
        self.nodes.get(id).copied()
    }

    #[inline]
    pub fn contains_node(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    #[inline]
    pub fn insert_edge(&mut self, id: impl Into<CompactString>, index: EdgeHandle) {
        self.edges.insert(id.into(), index);
    }

    #[inline]
    pub fn remove_edge(&mut self, id: &str) -> Option<EdgeHandle> {
        self.edges.swap_remove(id)
    }

    #[inline]
    pub fn get_edge(&self, id: &str) -> Option<EdgeHandle> {
        self.edges.get(id).copied()
    }

    // Only exercised by the crate's own unit tests below.
    #[cfg(test)]
    #[inline]
    pub fn get_edge_id_by_index(&self, index: EdgeHandle) -> Option<&str> {
        for (id, &idx) in &self.edges {
            if idx == index {
                return Some(id.as_str());
            }
        }
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn insert_node_label(&mut self, label: impl Into<CompactString>, index: VertexHandle) {
        // `IndexSet::insert` is a no-op when `index` is already a member of this label's bucket:
        // labels are a set, not a multiset, so this is the correct behavior, not a regression
        // relative to the old `Vec::push` (which could accumulate duplicate entries).
        self.node_labels
            .entry(label.into())
            .or_default()
            .insert(index);
    }

    #[allow(dead_code)]
    #[inline]
    pub fn remove_node_label(&mut self, label: &str, index: VertexHandle) {
        if let Some(set) = self.node_labels.get_mut(label) {
            // O(1) average, versus the old `Vec::retain`'s O(bucket size) full scan/shift.
            set.swap_remove(&index);
            if set.is_empty() {
                self.node_labels.remove(label);
            }
        }
    }

    /// Returns the members of `label`'s bucket, in the bucket's insertion order, or `None` if no
    /// vertex currently carries `label`. Iterator-based rather than slice-based because
    /// `IndexSet` does not expose a contiguous slice view the way `Vec` did; every caller in this
    /// crate only ever iterates the result, never needs random access.
    #[allow(dead_code)]
    #[inline]
    pub fn get_nodes_by_label(
        &self,
        label: &str,
    ) -> Option<impl Iterator<Item = VertexHandle> + '_ + use<'_>> {
        self.node_labels.get(label).map(|s| s.iter().copied())
    }

    #[allow(dead_code)]
    #[inline]
    pub fn insert_edge_label(&mut self, label: impl Into<CompactString>, index: EdgeHandle) {
        self.edge_labels
            .entry(label.into())
            .or_default()
            .insert(index);
    }

    #[allow(dead_code)]
    #[inline]
    pub fn remove_edge_label(&mut self, label: &str, index: EdgeHandle) {
        if let Some(set) = self.edge_labels.get_mut(label) {
            set.swap_remove(&index);
            if set.is_empty() {
                self.edge_labels.remove(label);
            }
        }
    }

    /// Returns the members of `label`'s bucket, in the bucket's insertion order, or `None` if no
    /// edge currently carries `label`. See [`Self::get_nodes_by_label`] for why this is
    /// iterator-based rather than slice-based.
    #[allow(dead_code)]
    #[inline]
    pub fn get_edges_by_label(
        &self,
        label: &str,
    ) -> Option<impl Iterator<Item = EdgeHandle> + '_ + use<'_>> {
        self.edge_labels.get(label).map(|s| s.iter().copied())
    }

    #[allow(dead_code)]
    #[inline]
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.node_labels.clear();
        self.edge_labels.clear();
    }

    /// Estimated heap footprint of this index's four collections, in bytes: each `IndexMap`'s
    /// entries and hash index sized from its own `.capacity()`
    /// ([`crate::value::property_map::index_map_heap_bytes`]), plus each label bucket
    /// `IndexSet`'s, plus heap-allocated label strings. Used by
    /// [`crate::graph::MemoryStats::index_bytes`] (D1 of the memory-inspection spec); walking every
    /// bucket, not just the outer maps, is deliberate — the bucket sets are where a
    /// heavily-labeled graph's memory actually lives.
    pub(crate) fn estimated_heap_bytes(&self) -> u64 {
        use crate::value::compact_string_heap_bytes;
        use crate::value::property_map::index_map_heap_bytes;

        let mut total = index_map_heap_bytes::<CompactString, VertexHandle>(self.nodes.capacity());
        total += index_map_heap_bytes::<CompactString, EdgeHandle>(self.edges.capacity());

        total += index_map_heap_bytes::<CompactString, IndexSet<VertexHandle, FnvBuildHasher>>(
            self.node_labels.capacity(),
        );
        for (label, bucket) in self.node_labels.iter() {
            total += compact_string_heap_bytes(label);
            total += index_map_heap_bytes::<VertexHandle, ()>(bucket.capacity());
        }

        total += index_map_heap_bytes::<CompactString, IndexSet<EdgeHandle, FnvBuildHasher>>(
            self.edge_labels.capacity(),
        );
        for (label, bucket) in self.edge_labels.iter() {
            total += compact_string_heap_bytes(label);
            total += index_map_heap_bytes::<EdgeHandle, ()>(bucket.capacity());
        }

        total
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.nodes.shrink_to_fit();
        self.edges.shrink_to_fit();
        self.node_labels.shrink_to_fit();
        for set in self.node_labels.values_mut() {
            set.shrink_to_fit();
        }
        self.edge_labels.shrink_to_fit();
        for set in self.edge_labels.values_mut() {
            set.shrink_to_fit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_label_indexing() {
        let mut index = GraphIndex::new();
        let n0 = VertexHandle::for_test(0);
        let n1 = VertexHandle::for_test(1);
        let n2 = VertexHandle::for_test(2);

        assert!(index.get_nodes_by_label("person").is_none());

        // Insert labels
        index.insert_node_label("person", n0);
        index.insert_node_label("person", n1);
        index.insert_node_label("software", n2);

        let person_nodes: Vec<_> = index.get_nodes_by_label("person").unwrap().collect();
        assert_eq!(person_nodes, vec![n0, n1]);

        let software_nodes: Vec<_> = index.get_nodes_by_label("software").unwrap().collect();
        assert_eq!(software_nodes, vec![n2]);

        // Remove a node from label
        index.remove_node_label("person", n0);
        let person_nodes: Vec<_> = index.get_nodes_by_label("person").unwrap().collect();
        assert_eq!(person_nodes, vec![n1]);

        // Remove non-existent node doesn't alter state
        index.remove_node_label("person", n0);
        let person_nodes: Vec<_> = index.get_nodes_by_label("person").unwrap().collect();
        assert_eq!(person_nodes, vec![n1]);

        // Remove last node cleans up label entry
        index.remove_node_label("person", n1);
        assert!(index.get_nodes_by_label("person").is_none());

        // Clear cleans up all labels
        index.clear();
        assert!(index.get_nodes_by_label("software").is_none());
    }

    #[test]
    fn test_edge_label_indexing() {
        let mut index = GraphIndex::new();
        let e0 = EdgeHandle::for_test(0);
        let e1 = EdgeHandle::for_test(1);
        let e2 = EdgeHandle::for_test(2);

        assert!(index.get_edges_by_label("knows").is_none());

        // Insert labels
        index.insert_edge_label("knows", e0);
        index.insert_edge_label("knows", e1);
        index.insert_edge_label("created", e2);

        let knows_edges: Vec<_> = index.get_edges_by_label("knows").unwrap().collect();
        assert_eq!(knows_edges, vec![e0, e1]);

        let created_edges: Vec<_> = index.get_edges_by_label("created").unwrap().collect();
        assert_eq!(created_edges, vec![e2]);

        // Remove an edge from label
        index.remove_edge_label("knows", e0);
        let knows_edges: Vec<_> = index.get_edges_by_label("knows").unwrap().collect();
        assert_eq!(knows_edges, vec![e1]);

        // Remove last edge cleans up label entry
        index.remove_edge_label("knows", e1);
        assert!(index.get_edges_by_label("knows").is_none());

        // Clear cleans up all labels
        index.clear();
        assert!(index.get_edges_by_label("created").is_none());
    }

    #[test]
    fn test_shrink_to_fit() {
        let mut index = GraphIndex::new();
        for i in 0..100 {
            let n = VertexHandle::for_test(i);
            let e = EdgeHandle::for_test(i);
            index.insert_node(format!("node_{}", i), n);
            index.insert_edge(format!("edge_{}", i), e);
            index.insert_node_label("person", n);
            index.insert_edge_label("knows", e);
        }
        index.shrink_to_fit();
        assert_eq!(index.get_nodes_by_label("person").unwrap().count(), 100);
        assert_eq!(index.get_edges_by_label("knows").unwrap().count(), 100);
        assert!(index.get_node("node_0").is_some());
        assert!(index.get_edge("edge_0").is_some());
    }
}
