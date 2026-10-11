//! Authorization: who may do what to which resource.
//!
//! For hosts that run queries on behalf of someone (a server, a multi-user tool): implement an
//! [`Authorizer`] or configure the ready-made [`AccessPolicy`] and attach it to a traversal source
//! (`with_authorizer`) or a script engine. Everything is allowed by default ([`AllowAll`]).
//! Guide: the book page `users_guide/permissions.md`.

pub use crate::graph::{
    AccessPolicy, AccessRequest, AccessRequestBuf, Action, ActionKind, AllowAll, Authorizer,
    Denied, Resource, ResourceKind,
};
