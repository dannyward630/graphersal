//! UUID values (`ScriptUuid`) and the `UUID.fromString(..)` literal spellings.

use crate::script::*;

/// A UUID value as the script sees it: the `ElementProperty::Uuid` bits, carried natively.
///
/// Registered with Rhai as `Uuid`, so `type_of(x)` is `"Uuid"`. A value read by a terminal
/// (`next()`, `to_list()`) and fed back into a filter keeps matching the stored `Uuid`, which a
/// round trip through a Rhai string would silently break. It prints as its canonical string and
/// never equals a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptUuid(pub(crate) u128);

impl ScriptUuid {
    /// The canonical lowercase 8-4-4-4-12 rendering.
    pub(crate) fn canonical(&self) -> String {
        crate::value::uuid::format_canonical(self.0).to_string()
    }

    /// The raw 128-bit value, for consumers outside this crate that convert a script-evaluated
    /// UUID into their own representation (e.g. `py-graphersal`'s `uuid.UUID` conversion).
    pub fn as_u128(&self) -> u128 {
        self.0
    }
}

/// `UUID()`: a random (version 4, RFC 9562 variant) UUID, like Java's `UUID.randomUUID()` that
/// the Gremlin grammar's `UUID()` stands for. Drawn from the operating system's entropy, not
/// from `random.seed`, which only seeds the steps of one traversal.
pub(crate) fn script_uuid_random() -> ScriptUuid {
    script_uuid_v4(rand::random())
}

/// A version 4, RFC 9562 variant UUID from 128 random bits (the version and variant bits are
/// overwritten). Shared by `UUID()` and the seeded `Fake.uuid()`.
pub(crate) fn script_uuid_v4(bits: u128) -> ScriptUuid {
    let version_4 = (bits & !(0xF << 76)) | (0x4 << 76);
    ScriptUuid((version_4 & !(0x3 << 62)) | (0x2 << 62))
}

/// Parses a UUID literal for `UUID.fromString(..)` and its spellings.
///
/// Shares the parser and the cast diagnostic with [`ElementProperty::uuid_from_string`].
pub(crate) fn script_uuid_from_string(value: &str) -> Result<ScriptUuid, Box<rhai::EvalAltResult>> {
    match ElementProperty::uuid_from_string(value) {
        Ok(ElementProperty::Uuid(bits)) => Ok(ScriptUuid(bits)),
        Ok(other) => Err(script_error(Box::new(
            crate::error::TraverserError::cast_exception_target(
                "uuid",
                crate::graph::schema::value_type_name(&other),
                Some(crate::value::literal::rhai_literal(&other)),
                None,
            ),
        ))),
        Err(err) => Err(script_error(err)),
    }
}

/// Scope marker that makes the Java Gremlin spelling `UUID.fromString("...")` resolve.
///
/// Mirrors [`ScriptGType`]: the constructors are methods of this marker for the dotted spelling,
/// and a static module provides `UUID::fromString(..)` / `UUID::from_string(..)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptUuidTokens;

#[export_module]
#[allow(non_snake_case, non_upper_case_globals)]
pub(crate) mod UuidModule {
    use super::{ScriptUuid, script_uuid_from_string};

    /// `UUID::fromString("550e8400-e29b-41d4-a716-446655440000")`: a UUID literal from its
    /// canonical string.
    #[rhai_fn(name = "fromString", name = "from_string", return_raw)]
    pub fn from_string(value: &str) -> Result<ScriptUuid, Box<rhai::EvalAltResult>> {
        script_uuid_from_string(value)
    }
}
