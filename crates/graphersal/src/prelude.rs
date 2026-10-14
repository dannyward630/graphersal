//! Everything to write queries: `use graphersal::prelude::*;`.
//!
//! For query writers: the graph and its sources ([`TraversalGraph`], [`Graph`], [`GraphSource`],
//! [`GraphTraversalSource`], [`__`]), the [`GraphStorage`] trait whose methods a graph is read
//! and written with (`vertex_count()`, `statistics()`, `memory_usage()`, ...), the values a
//! traversal produces and takes ([`TraverserValue`], [`ElementProperty`], [`PropertyMap`],
//! [`StreamMap`], [`ValueMap`], [`CompactString`], [`Number`], [`JsonPath`], [`PropKey`]), the
//! tokens ([`P`], [`Direction`], [`Order`], [`VisualizeFormat`], ...), the schema a query sets or
//! reads ([`GraphSchema`], [`SchemaMode`]), [`Execution`], [`TraversalMetrics`] and the errors.
//!
//! Everything else lives in exactly one topic module, never in the prelude: [`crate::storage`]
//! (implementing a storage), [`crate::changes`] (change capture, commit hooks), [`crate::auth`]
//! (authorization), [`crate::catalog`] (saved queries, compression rules, host kinds),
//! [`crate::schema`] (the schema model, validation, diff, patch), [`crate::profile`] (profiling
//! metrics), [`crate::exec`] (limits, cancellation, rendering) and [`crate::error`].

pub use crate::error::{
    BoxedTraverserError, DefinitionError, DiagnosticError, GraphError, IOError, SchemaError,
    ScriptError, StepFrame, StepLocation, StepPathEntry, TraverserError, TraverserResult,
    ValueError, ValueOrigin, format_diagnostic,
};
pub use crate::graph::{
    Graph, GraphSchema, GraphStorage, SchemaMode, Transactional, TraversalGraph, VisualizeFormat,
};
pub use crate::traversal::{
    __, AnonymousTraversal, By, ExecOptions, Execution, GlobPathOptions, GraphSource,
    GraphTraversalSource, Order, ProfileType, SideEffectFn, TraversalMetrics,
};
pub use crate::value::{
    Barrier, Cardinality, CastPolicy, Column, Direction, EdgeValue, ElementProperty, GType,
    JsonPath, Merge, Number, Operator, P, PValue, Pop, PropKey, PropKeys, PropertyHandleKind,
    PropertyMap, RecordKey, Scope, StreamKind, StreamMap, TraverserValue, ValueMap, VertexRef,
    VertexValue, WithOptions,
};
/// The small-string type of labels, property keys and string values (`ElementProperty::String`,
/// `TraverserValue` variants, `GraphStorage` signatures), re-exported from `compact_str` so a
/// user never has to depend on the matching `compact_str` version. A `compact_str` major bump is a
/// `graphersal` minor bump.
pub use compact_str::CompactString;

#[cfg(feature = "io")]
pub use crate::io::{GraphExport, GraphImport};
