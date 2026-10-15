//! Vector values in scripts (`ScriptVector`, book page "Vectors"): the `vector([..])` /
//! `vector([..], "i8")` constructor, printing as the DSL literal, equality, the `element` and
//! `dimension` getters, `quantize(kind)` and `to_list()`.

use crate::script::*;
use crate::value::{Vector, VectorElement, VectorError};

/// A vector value as the script sees it: the stored [`Vector`], carried natively.
///
/// Registered with Rhai as `Vector`, so `type_of(x)` is `"Vector"`. A value read by a terminal
/// (`next()`, `to_list()`) and fed back into the DSL (a filter, a property write) stays the same
/// vector; it prints as its DSL literal (`vector([0.1, 0.2], "i8")`, cut after 16 components),
/// never as an array, and never equals an array.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptVector(pub(crate) Vector);

impl ScriptVector {
    /// The vector, for hosts that convert a script value into their own representation (e.g.
    /// `py-graphersal`'s `graphersal.Vector`).
    pub fn vector(&self) -> &Vector {
        &self.0
    }
}

/// The script error of a failed vector construction or conversion in `name`.
pub(crate) fn vector_failure(name: &'static str, error: VectorError) -> Box<rhai::EvalAltResult> {
    script_error(crate::value::vector::vector_error(name, error))
}

/// The element kind of a name, or the script error naming the kinds.
pub(crate) fn element_of(
    name: &'static str,
    kind: &str,
) -> Result<VectorElement, Box<rhai::EvalAltResult>> {
    VectorElement::from_name(kind)
        .ok_or_else(|| vector_failure(name, VectorError::unknown_element(kind)))
}

/// `vector(list)`: an f32 vector of a list of numbers.
fn script_vector(items: rhai::Array) -> Result<ScriptVector, Box<rhai::EvalAltResult>> {
    let mut values =
        Vec::with_capacity(items.len().min(crate::value::vector::MAX_VECTOR_DIMENSION));
    for (index, item) in items.iter().enumerate() {
        let value = if let Ok(int) = item.as_int() {
            int as f64
        } else if let Ok(float) = item.as_float() {
            float
        } else {
            return Err(vector_failure(
                "vector",
                VectorError::not_a_number(index, type_name_of(item)),
            ));
        };
        values.push(value);
    }
    Vector::from_f64(&values)
        .map(ScriptVector)
        .map_err(|error| vector_failure("vector", error))
}

/// `vector(list, kind)`: the f32 vector of the list quantized to `kind`.
fn script_vector_of(
    items: rhai::Array,
    kind: &str,
) -> Result<ScriptVector, Box<rhai::EvalAltResult>> {
    let element = element_of("vector", kind)?;
    let vector = script_vector(items)?;
    vector
        .0
        .quantize(element)
        .map(ScriptVector)
        .map_err(|error| vector_failure("vector", error))
}

/// Registers the vector type, its constructor and its methods.
pub(crate) fn register_vector(registry: &mut crate::script::register::Registry) {
    registry
        .register_type_with_name::<ScriptVector>("Vector")
        .register_fn("vector", script_vector)
        .register_fn("vector", script_vector_of)
        .register_fn_as(&["to_string", "to_debug"], |v: &mut ScriptVector| {
            v.0.literal(Some(crate::value::literal::MAX_LITERAL_ITEMS))
        })
        .register_fn("==", |a: ScriptVector, b: ScriptVector| a == b)
        .register_fn("!=", |a: ScriptVector, b: ScriptVector| a != b)
        // A vector never equals a list (a different type, as in a filter).
        .register_fn("==", |_: ScriptVector, _: rhai::Array| false)
        .register_fn("==", |_: rhai::Array, _: ScriptVector| false)
        .register_fn("!=", |_: ScriptVector, _: rhai::Array| true)
        .register_fn("!=", |_: rhai::Array, _: ScriptVector| true)
        .register_get("element", |v: &mut ScriptVector| {
            v.0.element().name().to_string()
        })
        .register_get("dimension", |v: &mut ScriptVector| v.0.dimension() as i64)
        .register_fn("quantize", |v: &mut ScriptVector, kind: &str| {
            let element = element_of("quantize", kind)?;
            v.0.quantize(element)
                .map(ScriptVector)
                .map_err(|error| vector_failure("quantize", error))
        })
        .register_fn_as(&["to_list", "toList"], |v: &mut ScriptVector| {
            v.0.iter_f32()
                .map(|value| Dynamic::from_float(crate::value::vector::tagged::short_f64(value)))
                .collect::<rhai::Array>()
        });
}
