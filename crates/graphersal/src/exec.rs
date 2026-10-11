//! Execution control for hosts: limits and their locks, cancellation, result rendering.
//!
//! For a host that runs other people's queries (a server, a REPL, a notebook): an
//! [`ExecutionPolicy`] sets and locks administrative options, a [`CancelToken`] stops a running
//! execution, [`RenderOptions`] bound a displayed result. Query writers need only
//! [`crate::prelude`]. Guides: the book pages `users_guide/resource_limits.md` and
//! `users_guide/displaying_results.md`.

pub use crate::graph::ExecutionPolicy;
pub use crate::traversal::{
    CancelToken, GlobPathOptions, RenderOptions, RenderedOutput, Spelling, Truncation,
    optimizer_rule_names, traversal_limit_defaults,
};
