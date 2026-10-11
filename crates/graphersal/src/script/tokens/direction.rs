//! The `Direction` token (`Direction.OUT`/`Direction::OUT`).

use crate::script::*;

/// Which family of vertex steps a `to(Direction, ..)`-style DSL call expands to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectionStepKind {
    /// `to(..)`: `out`/`in`/`both`.
    Vertex,
    /// `toE(..)`: `outE`/`inE`/`bothE`.
    Edge,
}

/// Scope marker that makes the dotted spelling `Direction.OUT` / `.IN` / `.BOTH` (and the
/// Gremlin aliases `Direction.from` = `OUT`, `Direction.to` = `IN`) resolve.
///
/// Mirrors [`ScriptScope`]; the static module gives `Direction::OUT` .... No bare `OUT`/`IN`
/// constants. The token itself is [`crate::value::Direction`], registered with Rhai as
/// `Direction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptDirection;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod DirectionModule {
    use super::Direction;
    use crate::value::literal::TokenLiteral;

    /// `Direction.OUT`: the outgoing side.
    pub const OUT: Direction = Direction::Out;
    /// `Direction.IN`: the incoming side.
    pub const IN: Direction = Direction::In;
    /// `Direction.BOTH`: either side.
    pub const BOTH: Direction = Direction::Both;
    /// `Direction.from`: alias of `Direction.OUT`.
    pub const from: Direction = Direction::Out;
    /// `Direction.to`: alias of `Direction.IN`.
    pub const to: Direction = Direction::In;

    /// Printing
    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(direction: &mut Direction) -> String {
        direction.token_literal()
    }
}
