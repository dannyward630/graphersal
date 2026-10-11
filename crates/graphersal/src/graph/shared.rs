//! [`Graph`]: a storage behind a read-write lock, the shape a graph is shared in.

use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::graph::{GraphStorage, TraversalGraph};

/// A shareable, lock-protected graph: a [`GraphStorage`] `S` behind a read-write lock.
///
/// It is what [`GraphSource`](crate::GraphSource)'s constructors return and what the `script`
/// feature's entry points (`graph_scope`, every `eval_*`) take, as an `Arc<Graph<S>>` of any
/// storage `S: GraphStorage + Default + Send + Sync + 'static` (through
/// [`IntoAnyGraph`](crate::graph::IntoAnyGraph)); a host storage is shared the same way
/// (the book page `for_developers/custom_storage.md`); `Graph::new_persistent` (feature
/// `persist`) wraps a `PersistentStorage`. The type parameter defaults to [`TraversalGraph`], so `Graph` and `Arc<Graph>` in type position mean
/// `Graph<TraversalGraph>` and `Arc<Graph<TraversalGraph>>`; an expression takes `S` from its
/// argument (`Graph::new(storage)`).
///
/// [`read`](Self::read) shares the lock, [`write`](Self::write) takes it exclusively; the
/// guards dereference to `S`. `Graph<S>` is `Send`/`Sync` exactly when `S` is.
///
/// ```rust
/// use graphersal::prelude::*;
///
/// let graph = Graph::new(TraversalGraph::tinkerpop_modern());
/// assert_eq!(graph.read().vertex_count(), 6);
/// GraphTraversalSource::new(&mut *graph.write()).add_v("person").next().unwrap();
/// assert_eq!(graph.into_inner().vertex_count(), 7);
/// ```
pub struct Graph<S: GraphStorage = TraversalGraph> {
    lock: parking_lot::RwLock<S>,
    /// The packed-snapshot writer of `S`, set by [`Graph::new_persistent`]: the dyn boundary
    /// ([`AnyGraph::write_snapshot`](crate::graph::AnyGraph::write_snapshot)) cannot test
    /// `S: PersistentStorage` itself (design `roadmap/generic-front-ends.md` 5.3).
    #[cfg(feature = "persist")]
    snapshot_writer: Option<SnapshotWriter<S>>,
}

/// Writes a storage as a packed snapshot with the default options.
#[cfg(feature = "persist")]
pub(crate) type SnapshotWriter<S> =
    fn(
        &S,
        &mut dyn std::io::Write,
    ) -> Result<crate::persist::SnapshotInfo, crate::persist::PersistError>;

/// Shared read access to the storage of a [`Graph`] ([`Graph::read`]); dereferences to `S`.
/// The lock is released when the guard is dropped.
pub struct GraphRead<'a, S: GraphStorage = TraversalGraph>(parking_lot::RwLockReadGuard<'a, S>);

/// Exclusive write access to the storage of a [`Graph`] ([`Graph::write`],
/// [`Graph::try_write`]); dereferences (mutably) to `S`. The lock is released when the guard is
/// dropped.
pub struct GraphWrite<'a, S: GraphStorage = TraversalGraph>(parking_lot::RwLockWriteGuard<'a, S>);

impl<S: GraphStorage> Graph<S> {
    /// Wraps `storage` for sharing.
    ///
    /// With feature `persist`: a graph built here can be written as a snapshot through the dyn
    /// boundary (`AnyGraph::write_snapshot`, the DSL's `g.export_snapshot(path)`) only when `S` is [`TraversalGraph`]; any other
    /// storage fails there with `GraphError::Unsupported`. A storage that implements
    /// `PersistentStorage` is wrapped with `Graph::new_persistent` to be writable there too.
    /// (The typed snapshot functions in `graphersal::persist` take any persistent storage
    /// directly.)
    pub fn new(storage: S) -> Self {
        Graph {
            lock: parking_lot::RwLock::new(storage),
            #[cfg(feature = "persist")]
            snapshot_writer: None,
        }
    }

    /// A new graph around `storage` with the same capabilities at the dyn boundary as `self`
    /// (the snapshot writer of [`Graph::new_persistent`]): a whole-script unit moves the
    /// storage into such a private graph.
    #[cfg_attr(not(feature = "script"), allow(dead_code))]
    pub(crate) fn new_like(&self, storage: S) -> Self {
        Graph {
            lock: parking_lot::RwLock::new(storage),
            #[cfg(feature = "persist")]
            snapshot_writer: self.snapshot_writer,
        }
    }

    /// The snapshot writer set by [`Graph::new_persistent`].
    #[cfg(feature = "persist")]
    pub(crate) fn snapshot_writer(&self) -> Option<SnapshotWriter<S>> {
        self.snapshot_writer
    }

    /// Locks the graph for reading, blocking while a writer holds it. Several readers may hold
    /// it at once.
    pub fn read(&self) -> GraphRead<'_, S> {
        GraphRead(self.lock.read())
    }

    /// Locks the graph for writing, blocking until no reader or writer holds it.
    pub fn write(&self) -> GraphWrite<'_, S> {
        GraphWrite(self.lock.write())
    }

    /// Locks the graph for writing if nobody holds it right now, else returns `None` without
    /// blocking (a thread that already holds the lock gets `None` instead of a deadlock).
    pub fn try_write(&self) -> Option<GraphWrite<'_, S>> {
        self.lock.try_write().map(GraphWrite)
    }

    /// The storage itself, without locking: the exclusive borrow proves nobody else holds it.
    pub fn get_mut(&mut self) -> &mut S {
        self.lock.get_mut()
    }

    /// Unwraps the storage.
    pub fn into_inner(self) -> S {
        self.lock.into_inner()
    }
}

#[cfg(feature = "persist")]
impl<S: crate::graph::PersistentStorage> Graph<S> {
    /// Wraps a persistent `storage` for sharing, so the dyn boundary can write it as a packed
    /// snapshot too: [`AnyGraph::write_snapshot`](crate::graph::AnyGraph::write_snapshot) and
    /// the DSL's `g.export_snapshot(path)` call
    /// [`write_snapshot`](crate::persist::write_snapshot) for `S`. Otherwise the same as
    /// [`Graph::new`] (a [`TraversalGraph`] is writable through either).
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use graphersal::prelude::*;
    /// use graphersal::storage::AnyGraph;
    ///
    /// let graph = Arc::new(Graph::new_persistent(TraversalGraph::tinkerpop_modern()));
    /// let mut bytes = Vec::new();
    /// let info = graph.write_snapshot(&mut bytes).unwrap();
    /// assert_eq!(info.vertex_count, 6);
    /// ```
    pub fn new_persistent(storage: S) -> Self {
        fn write<S: crate::graph::PersistentStorage>(
            storage: &S,
            writer: &mut dyn std::io::Write,
        ) -> Result<crate::persist::SnapshotInfo, crate::persist::PersistError> {
            crate::persist::write_snapshot(storage, writer, &crate::persist::WriteOptions::new())
        }
        Graph {
            lock: parking_lot::RwLock::new(storage),
            snapshot_writer: Some(write::<S>),
        }
    }
}

impl<S: GraphStorage + Default> Default for Graph<S> {
    /// An empty graph (`S::default()`).
    fn default() -> Self {
        Graph::new(S::default())
    }
}

impl<S: GraphStorage> From<S> for Graph<S> {
    fn from(storage: S) -> Self {
        Graph::new(storage)
    }
}

impl<S: GraphStorage> fmt::Debug for Graph<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Graph").finish_non_exhaustive()
    }
}

impl<S: GraphStorage> Deref for GraphRead<'_, S> {
    type Target = S;

    #[inline]
    fn deref(&self) -> &S {
        &self.0
    }
}

impl<S: GraphStorage> Deref for GraphWrite<'_, S> {
    type Target = S;

    #[inline]
    fn deref(&self) -> &S {
        &self.0
    }
}

impl<S: GraphStorage> DerefMut for GraphWrite<'_, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut S {
        &mut self.0
    }
}

impl<S: GraphStorage> fmt::Debug for GraphRead<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphRead").finish_non_exhaustive()
    }
}

impl<S: GraphStorage> fmt::Debug for GraphWrite<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphWrite").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::PropertyMap;

    #[test]
    fn new_read_write_try_write_get_mut_into_inner() {
        let mut graph = Graph::new(TraversalGraph::new());
        assert_eq!(graph.read().vertex_count(), 0);
        graph
            .write()
            .add_vertex(Some("a".into()), vec!["n".into()], PropertyMap::default())
            .expect("a writable storage");
        {
            let first = graph.read();
            let second = graph.read();
            assert_eq!((first.vertex_count(), second.vertex_count()), (1, 1));
            assert!(graph.try_write().is_none(), "readers hold the lock");
        }
        {
            let held = graph.try_write().expect("nobody holds the lock");
            assert!(graph.try_write().is_none(), "a writer holds the lock");
            drop(held);
        }
        graph
            .get_mut()
            .add_vertex(Some("b".into()), vec![], PropertyMap::default())
            .expect("a writable storage");
        let storage = graph.into_inner();
        assert_eq!(storage.vertex_count(), 2);
        assert!(storage.get_vertex_by_id("b").is_some());
    }

    #[test]
    fn default_is_empty_and_the_type_is_shareable() {
        fn shareable<T: Send + Sync>() {}
        shareable::<Graph>();
        let graph: Graph = Graph::default();
        assert_eq!(graph.read().vertex_count(), 0);
        assert_eq!(format!("{graph:?}"), "Graph { .. }");
    }
}
