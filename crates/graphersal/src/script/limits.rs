//! Resource limits of the Rhai script engine.
//!
//! A script is untrusted input as soon as a front end faces a user: `loop {}`, a string that
//! doubles forever, a huge array or unbounded recursion would hang or exhaust the process. A
//! [`ScriptLimits`] value bounds each of those; [`engine_with_limits`](super::engine_with_limits)
//! installs it on the engine. The limits are counters Rhai checks itself, so they are portable
//! (including `wasm32`, where there is no wall clock to read). A wall-clock budget for a whole
//! script is a front-end concern; traversals have their own `evaluationTimeout`.

use rhai::{Engine, EvalAltResult};

use crate::error::TraverserError;

/// Option name reported in a [`TraverserError::ResourceLimitExceeded`] for
/// [`ScriptLimits::max_operations`].
pub const LIMIT_MAX_OPERATIONS: &str = "script.max_operations";
/// Limit name for [`ScriptLimits::max_string_size`].
pub const LIMIT_MAX_STRING_SIZE: &str = "script.max_string_size";
/// Limit name for [`ScriptLimits::max_array_size`].
pub const LIMIT_MAX_ARRAY_SIZE: &str = "script.max_array_size";
/// Limit name for [`ScriptLimits::max_map_size`].
pub const LIMIT_MAX_MAP_SIZE: &str = "script.max_map_size";
/// Limit name for [`ScriptLimits::max_call_levels`].
pub const LIMIT_MAX_CALL_LEVELS: &str = "script.max_call_levels";
/// Limit name for [`ScriptLimits::max_value_depth`].
pub const LIMIT_MAX_VALUE_DEPTH: &str = "script.max_value_depth";

/// Default for [`ScriptLimits::max_operations`]: ten million Rhai operations, a few seconds of
/// pure script work in a debug build.
pub const DEFAULT_MAX_OPERATIONS: u64 = 10_000_000;
/// Default for [`ScriptLimits::max_string_size`]: 16 MiB of text.
pub const DEFAULT_MAX_STRING_SIZE: usize = 16 * 1024 * 1024;
/// Default for [`ScriptLimits::max_array_size`]: 100 000 items. Rhai re-measures an array on
/// every method call that touches it (`push` in a loop is quadratic), so a large cap is also a
/// CPU budget: measured, a loop pushing to a one-million-item cap ran for about half an hour in a
/// debug build before the cap stopped it.
pub const DEFAULT_MAX_ARRAY_SIZE: usize = 100_000;
/// Default for [`ScriptLimits::max_map_size`]: 100 000 entries (same reasoning as the array cap).
pub const DEFAULT_MAX_MAP_SIZE: usize = 100_000;
/// Default for [`ScriptLimits::max_call_levels`]. Also the level [`ScriptLimits::unlimited`]
/// keeps: it protects the native stack of the thread, which no host can make larger by
/// configuration, so it is a robustness bound, not a resource budget.
pub const DEFAULT_MAX_CALL_LEVELS: usize = 32;
/// Default for [`ScriptLimits::max_value_depth`]: 128 nested arrays/maps, the recursion limit
/// `serde_json` applies to JSON input. Also the default of the execution option
/// `traversal.max_value_depth`, and the level [`ScriptLimits::unlimited`] keeps: a deeper value
/// would overflow the native stack of the recursive code that reads it, so it is a robustness
/// bound, not a resource budget.
pub const DEFAULT_MAX_VALUE_DEPTH: usize = crate::value::depth::DEFAULT_MAX_VALUE_DEPTH;

/// Bounds on what one script may do, checked by the Rhai engine itself.
///
/// [`Default`] is the SAFE configuration (sized so every documented example and the TinkerPop
/// scenarios run unchanged); [`ScriptLimits::unlimited`] is for trusted local front ends only.
/// `0` means "no limit" for the four counters.
///
/// ```rust
/// use graphersal::script::ScriptLimits;
///
/// let limits = ScriptLimits::default().with_max_operations(1_000_000);
/// let engine = graphersal::script::engine_with_limits(
///     &limits,
///     std::sync::Arc::new(graphersal::auth::AccessPolicy::deny_all()),
/// );
/// assert!(engine.eval::<i64>("let n = 0; loop { n += 1; }").is_err());
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLimits {
    /// Rhai operations one script evaluation may execute (`loop {}` ends here). `0`: unlimited.
    pub max_operations: u64,
    /// Longest string a script may build, in bytes. `0`: unlimited.
    pub max_string_size: usize,
    /// Largest array (or BLOB) a script may build, in items. `0`: unlimited.
    pub max_array_size: usize,
    /// Largest object map a script may build, in entries. `0`: unlimited.
    pub max_map_size: usize,
    /// Deepest nesting of script function calls (recursion). Never unlimited: the default call
    /// depth (`DEFAULT_MAX_CALL_LEVELS`) is kept even by [`ScriptLimits::unlimited`]. `0` is read
    /// as the default.
    pub max_call_levels: usize,
    /// Deepest nesting of arrays and maps in a value the script hands to the engine (a step
    /// argument such as `inject(x)` or `property("k", x)`, a parameter, a value read back into
    /// the script), counted as in `[[1]]` = 2 levels. A deeper value fails with
    /// [`TraverserError::ResourceLimitExceeded`] (`script.max_value_depth`) instead of
    /// overflowing the native stack. Never unlimited: [`ScriptLimits::unlimited`] keeps the
    /// default (`DEFAULT_MAX_VALUE_DEPTH`), and `0` is read as the default. Values the engine
    /// builds itself are bounded by the execution option `traversal.max_value_depth`.
    pub max_value_depth: usize,
}

impl Default for ScriptLimits {
    /// The SAFE limits: 10 000 000 operations, 16 MiB strings, 100 000 array items, 100 000 map
    /// entries, the default call depth and value depth (`DEFAULT_MAX_*` constants of this
    /// module).
    fn default() -> Self {
        Self {
            max_operations: DEFAULT_MAX_OPERATIONS,
            max_string_size: DEFAULT_MAX_STRING_SIZE,
            max_array_size: DEFAULT_MAX_ARRAY_SIZE,
            max_map_size: DEFAULT_MAX_MAP_SIZE,
            max_call_levels: DEFAULT_MAX_CALL_LEVELS,
            max_value_depth: DEFAULT_MAX_VALUE_DEPTH,
        }
    }
}

impl ScriptLimits {
    /// No operation, string, array or map limit. Only for a trusted local front end (a command
    /// line the user types into); never for a service facing other people. The call depth and the
    /// value depth stay bounded at their defaults (`DEFAULT_MAX_CALL_LEVELS`,
    /// `DEFAULT_MAX_VALUE_DEPTH`) to protect the native stack.
    pub fn unlimited() -> Self {
        Self {
            max_operations: 0,
            max_string_size: 0,
            max_array_size: 0,
            max_map_size: 0,
            max_call_levels: DEFAULT_MAX_CALL_LEVELS,
            max_value_depth: DEFAULT_MAX_VALUE_DEPTH,
        }
    }

    /// Sets [`Self::max_operations`] (`0`: unlimited).
    pub fn with_max_operations(mut self, max: u64) -> Self {
        self.max_operations = max;
        self
    }

    /// Sets [`Self::max_string_size`] (`0`: unlimited).
    pub fn with_max_string_size(mut self, max: usize) -> Self {
        self.max_string_size = max;
        self
    }

    /// Sets [`Self::max_array_size`] (`0`: unlimited).
    pub fn with_max_array_size(mut self, max: usize) -> Self {
        self.max_array_size = max;
        self
    }

    /// Sets [`Self::max_map_size`] (`0`: unlimited).
    pub fn with_max_map_size(mut self, max: usize) -> Self {
        self.max_map_size = max;
        self
    }

    /// Sets [`Self::max_call_levels`].
    pub fn with_max_call_levels(mut self, max: usize) -> Self {
        self.max_call_levels = max;
        self
    }

    /// Sets [`Self::max_value_depth`] (`0`: the default).
    pub fn with_max_value_depth(mut self, max: usize) -> Self {
        self.max_value_depth = max;
        self
    }

    /// [`Self::max_value_depth`] with `0` read as the default.
    pub(crate) fn effective_max_value_depth(&self) -> usize {
        if self.max_value_depth == 0 {
            DEFAULT_MAX_VALUE_DEPTH
        } else {
            self.max_value_depth
        }
    }

    /// Installs these limits on `engine`.
    pub fn apply(&self, engine: &mut Engine) {
        engine.set_max_operations(self.max_operations);
        engine.set_max_string_size(self.max_string_size);
        engine.set_max_array_size(self.max_array_size);
        engine.set_max_map_size(self.max_map_size);
        let levels = if self.max_call_levels == 0 {
            DEFAULT_MAX_CALL_LEVELS
        } else {
            self.max_call_levels
        };
        engine.set_max_call_levels(levels);
    }
}

/// Classifies a Rhai evaluation error as one of the [`ScriptLimits`] being hit.
///
/// Returns the matching [`TraverserError::ResourceLimitExceeded`] (its `help()` names the field to
/// raise), or `None` when `err` is any other failure. `limits` supplies the configured maximum for
/// the message. The `eval_*` functions of this module already apply this, so it is only needed by
/// a host that drives its own engine.
pub fn resource_limit_from_rhai(
    err: &EvalAltResult,
    limits: &ScriptLimits,
) -> Option<TraverserError> {
    let (limit, max) = match err {
        EvalAltResult::ErrorInFunctionCall(_, _, inner, _)
        | EvalAltResult::ErrorInModule(_, inner, _) => {
            return resource_limit_from_rhai(inner, limits);
        }
        EvalAltResult::ErrorTooManyOperations(_) => (LIMIT_MAX_OPERATIONS, limits.max_operations),
        EvalAltResult::ErrorStackOverflow(_) => {
            (LIMIT_MAX_CALL_LEVELS, limits.max_call_levels as u64)
        }
        EvalAltResult::ErrorDataTooLarge(what, _) => {
            let what = what.to_ascii_lowercase();
            if what.contains("string") {
                (LIMIT_MAX_STRING_SIZE, limits.max_string_size as u64)
            } else if what.contains("array") || what.contains("blob") {
                (LIMIT_MAX_ARRAY_SIZE, limits.max_array_size as u64)
            } else if what.contains("map") {
                (LIMIT_MAX_MAP_SIZE, limits.max_map_size as u64)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(TraverserError::ResourceLimitExceeded {
        limit: limit.to_string(),
        max: (max > 0).then_some(max),
        observed: None,
    })
}
