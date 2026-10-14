//! `py-graphersal`: PyO3 bindings that run Gremlin scripts through the Graphersal engine.
//!
//! This crate adds zero new query execution logic. It evaluates script text through
//! `graphersal::script::eval_value_with_limits` / `eval_value_atomic` / `eval_value_dry_run`
//! and converts the `Dynamic` result into native Python structures. The public Python names are
//! listed in `graphersal.pyi`.

// The extension installs the tracking allocator as its own global allocator (a `cdylib` may; a
// library crate cannot do it for its consumer). It counts only the Rust heap of this extension
// (graphs, queries, their results before conversion), never Python objects: Memory profiles
// (`.profile_with(ProfileType.Memory)`) report `tracked` bytes, and the memory budget compares the
// live Rust heap (one atomic load) instead of walking the graph once per traversal. The cost is a
// few counter increments per allocation (see `graphersal::alloc_tracking`).
#[global_allocator]
static ALLOC: ::graphersal::alloc_tracking::TrackingAllocator =
    ::graphersal::alloc_tracking::TrackingAllocator::new();

mod convert;
mod element;
mod error;
mod graph;
mod policy;
mod script_thread;
mod store;
mod temporal;

use pyo3::prelude::*;

pub use element::{Edge, Vertex};
pub use error::{
    CommitRejectedError, GraphersalError, PermissionDeniedError, ReturnedClosureError,
};
pub use graph::{DryRunResult, Graph};
pub use policy::AccessPolicy;
pub use store::Store;
pub use temporal::Interval;

/// Python bindings of the Graphersal in-memory graph traversal engine: run Gremlin scripts against
/// a `Graph` and get native Python values back. See `Graph`, `AccessPolicy` and `GraphersalError`.
#[pymodule]
fn graphersal(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Graph>()?;
    m.add_class::<AccessPolicy>()?;
    m.add_class::<DryRunResult>()?;
    m.add_class::<Store>()?;
    m.add_class::<Vertex>()?;
    m.add_class::<Edge>()?;
    m.add_class::<Interval>()?;
    let error_type = py.get_type::<GraphersalError>();
    // A class-level default so every `GraphersalError` has `.help` even when raised through a
    // path that never calls `error::new_graphersal_error` (e.g. the `Dynamic` conversion
    // fallback in `convert.rs`, which raises via `GraphersalError::new_err` directly because it
    // never carries help text to begin with). An instance `setattr` in `new_graphersal_error`
    // still shadows this per Python's normal attribute lookup; the subclasses inherit it.
    error_type.setattr("help", py.None())?;
    m.add("GraphersalError", error_type)?;
    m.add(
        "PermissionDeniedError",
        py.get_type::<PermissionDeniedError>(),
    )?;
    m.add("CommitRejectedError", py.get_type::<CommitRejectedError>())?;
    m.add(
        "ReturnedClosureError",
        py.get_type::<ReturnedClosureError>(),
    )?;
    Ok(())
}
