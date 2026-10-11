//! The `ProfileType` token of `profile_with()` (`ProfileType.Memory`).

/// Scope marker that makes the vanilla dotted spelling `ProfileType.Memory` resolve. See
/// [`ScriptBy`](super::ScriptBy)'s doc comment for the shared rationale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptProfileType;
