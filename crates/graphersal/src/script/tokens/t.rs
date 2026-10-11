//! TinkerPop's `T` token (`T.id`/`T.label`).

/// Scope marker for TinkerPop's `T` token: `T.id`/`T.label` (dot, Gremlin-Groovy spelling) and
/// `T::id`/`T::label` (`::`, static module). Both evaluate to the same `ArgBy::Id`/`ArgBy::Label`
/// value as `By.Id`/`By.Label`, so every position that accepts those (`by()` on every
/// by()-modulated step) accepts `T.id`/`T.label` too; `has(T.id, ..)`/`has(T.label, ..)` add
/// the TinkerPop filter form on top. `T.key`/`T.value` are not defined: the engine has no
/// property-element key/value accessor with TinkerPop's `T.key`/`T.value` semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptT;
