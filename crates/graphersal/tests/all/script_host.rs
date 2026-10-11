//! The script entry as a trusted local host uses it, for the integration tests.
//!
//! The library's script entry allows everything by default (`graphersal::script::engine()`,
//! `graph_scope(g)`, `eval_value(..)`), but the entries that take explicit limits also take an
//! `Authorizer`. Almost every test exercises the DSL itself, not the authorization check, so it
//! imports the script API from here: everything of `graphersal::script`, with those entry points
//! shadowed by versions that pass `AllowAll`. `permission_tests.rs` uses `graphersal::script`
//! directly.
#![cfg(feature = "script")]
#![allow(dead_code, unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;

pub use graphersal::script::*;

use graphersal::ElementProperty;
use graphersal::auth::{AllowAll, Authorizer};
use graphersal::script::rhai::{Dynamic, Engine, EvalAltResult, Scope};
use graphersal::storage::IntoAnyGraph;

fn trusted() -> Arc<dyn Authorizer> {
    Arc::new(AllowAll)
}

/// `graphersal::script::engine_with_limits(limits, AllowAll)`.
pub fn engine_with_limits(limits: &ScriptLimits) -> Engine {
    graphersal::script::engine_with_limits(limits, trusted())
}

/// `graphersal::script::engine_with_options(limits, AllowAll, options)`.
pub fn engine_with_options(limits: &ScriptLimits, options: &[(&str, ElementProperty)]) -> Engine {
    graphersal::script::engine_with_options(limits, trusted(), options)
}

/// `graphersal::script::graph_scope_with_options(g, options, AllowAll)`.
pub fn graph_scope_with_options<'a>(
    g: impl IntoAnyGraph,
    options: &[(&str, ElementProperty)],
) -> Scope<'a> {
    graphersal::script::graph_scope_with_options(g, options, trusted())
}

/// `graphersal::script::eval_value_with_limits(.., limits, AllowAll)`.
pub fn eval_value_with_limits(
    graph: impl IntoAnyGraph,
    script: &str,
    params: HashMap<String, ElementProperty>,
    limits: &ScriptLimits,
) -> Result<Dynamic, Box<EvalAltResult>> {
    graphersal::script::eval_value_with_limits(graph, script, params, limits, trusted())
}

/// `graphersal::script::eval_value_atomic(.., limits, AllowAll)`.
pub fn eval_value_atomic(
    graph: impl IntoAnyGraph,
    script: &str,
    params: HashMap<String, ElementProperty>,
    limits: &ScriptLimits,
) -> Result<Dynamic, Box<EvalAltResult>> {
    graphersal::script::eval_value_atomic(graph, script, params, limits, trusted())
}

/// `graphersal::script::eval_value_dry_run(.., limits, AllowAll)`.
pub fn eval_value_dry_run(
    graph: impl IntoAnyGraph,
    script: &str,
    params: HashMap<String, ElementProperty>,
    limits: &ScriptLimits,
) -> Result<(Dynamic, graphersal::changes::ChangeSet), Box<EvalAltResult>> {
    graphersal::script::eval_value_dry_run(graph, script, params, limits, trusted())
}
