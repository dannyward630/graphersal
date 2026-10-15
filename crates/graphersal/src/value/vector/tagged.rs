//! The JSON forms of a vector: the plain list of its values (the physical codec: JSON, GraphML,
//! GraphSON export, result views) and the exact tagged payload of
//! `{"@type": "graphersal:Vector", "@value": {..}}` (the serialized `ChangeSet`, read by the
//! GraphSON import too).
//!
//! The tagged payload per element kind (exact, compact):
//!
//! | Element | `@value` |
//! |---|---|
//! | f32 | `{"element": "f32", "data": [0.1, -2.5]}` (each number reads back as that f32) |
//! | f16 | `{"element": "f16", "data": [0.5, 1.0]}` (every half value is exact as a number) |
//! | i8 | `{"element": "i8", "scale": 0.0157, "codes": [127, -3]}` |
//! | bit | `{"element": "bit", "dimension": 10, "bits": "a8c0"}` (the packed bytes in hex) |

use std::borrow::Cow;

use super::{Data, Vector, VectorElement};
use crate::value::physical::PhysicalValue;

/// The tag of a vector in the tagged JSON form.
#[cfg_attr(not(any(feature = "serde", feature = "io")), allow(dead_code))]
pub(crate) const VECTOR_TAG: &str = "graphersal:Vector";

/// The f64 nearest to the shortest decimal text of an f32: `0.1f32` lowers as `0.1`, not as
/// `0.10000000149011612`, and still reads back as the same f32 (checked; the exact widening is
/// the fallback).
pub(crate) fn short_f64(value: f32) -> f64 {
    let exact = f64::from(value);
    match value.to_string().parse::<f64>() {
        Ok(short) if short as f32 == value => short,
        _ => exact,
    }
}

/// The values of a vector as a JSON array of numbers (dequantized: an i8 or bit vector lowers
/// to its f32 values, which [`Vector::quantize`] turns back into it).
pub(crate) fn lower_values(vector: &Vector) -> PhysicalValue<'static> {
    PhysicalValue::Array(
        vector
            .iter_f32()
            .map(|value| PhysicalValue::Float(short_f64(value)))
            .collect(),
    )
}

#[cfg_attr(not(feature = "serde"), allow(dead_code))]
fn field(
    key: &'static str,
    value: PhysicalValue<'static>,
) -> (Cow<'static, str>, PhysicalValue<'static>) {
    (Cow::Borrowed(key), value)
}

#[cfg_attr(not(feature = "serde"), allow(dead_code))]
const HEX: &[u8; 16] = b"0123456789abcdef";

/// The exact `@value` payload of a vector (module docs).
#[cfg_attr(not(feature = "serde"), allow(dead_code))] // the serialized `ChangeSet`
pub(crate) fn payload(vector: &Vector) -> PhysicalValue<'static> {
    let element = field(
        "element",
        PhysicalValue::String(Cow::Borrowed(vector.element().name())),
    );
    match &*vector.0 {
        Data::F32(_) | Data::F16(_) => {
            PhysicalValue::Object(vec![element, field("data", lower_values(vector))])
        }
        Data::I8 { codes, scale } => PhysicalValue::Object(vec![
            element,
            field("scale", PhysicalValue::Float(short_f64(*scale))),
            field(
                "codes",
                PhysicalValue::Array(
                    codes
                        .iter()
                        .map(|code| PhysicalValue::Int(i64::from(*code)))
                        .collect(),
                ),
            ),
        ]),
        Data::Bit { bits, dimension } => {
            let mut hex = String::with_capacity(bits.len() * 2);
            for byte in bits.iter() {
                hex.push(HEX[usize::from(byte >> 4)] as char);
                hex.push(HEX[usize::from(byte & 0x0f)] as char);
            }
            PhysicalValue::Object(vec![
                element,
                field("dimension", PhysicalValue::Int(i64::from(*dimension))),
                field("bits", PhysicalValue::String(Cow::Owned(hex))),
            ])
        }
    }
}

fn number(value: &PhysicalValue<'_>) -> Option<f64> {
    match value {
        PhysicalValue::Int(int) => Some(*int as f64),
        PhysicalValue::Float(float) => Some(*float),
        _ => None,
    }
}

/// Reads the `@value` payload written by [`payload`], strictly: exactly the keys of the element
/// kind, values of their type, every check of the constructors.
#[cfg_attr(not(any(feature = "serde", feature = "io")), allow(dead_code))]
pub(crate) fn from_payload(value: &PhysicalValue<'_>) -> Result<Vector, String> {
    let PhysicalValue::Object(fields) = value else {
        return Err(format!("{VECTOR_TAG} needs an object payload"));
    };
    let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
    let Some(PhysicalValue::String(name)) = get("element") else {
        return Err(format!("{VECTOR_TAG} needs a string \"element\""));
    };
    let element = VectorElement::from_name(name)
        .ok_or_else(|| format!("{VECTOR_TAG}: unknown element \"{name}\""))?;
    let allowed: &[&str] = match element {
        VectorElement::F32 | VectorElement::F16 => &["element", "data"],
        VectorElement::I8 => &["element", "scale", "codes"],
        VectorElement::Bit => &["element", "dimension", "bits"],
    };
    if let Some((key, _)) = fields.iter().find(|(k, _)| !allowed.contains(&k.as_ref())) {
        return Err(format!(
            "{VECTOR_TAG} of element {element} has no key \"{key}\" (keys: {})",
            allowed.join(", ")
        ));
    }
    if fields.len() != allowed.len() {
        return Err(format!(
            "{VECTOR_TAG} of element {element} needs the keys {}",
            allowed.join(", ")
        ));
    }
    let bad = |err: super::VectorError| format!("{VECTOR_TAG}: {err}");
    match element {
        VectorElement::F32 | VectorElement::F16 => {
            let Some(PhysicalValue::Array(items)) = get("data") else {
                return Err(format!("{VECTOR_TAG} needs an array \"data\""));
            };
            let mut values = Vec::with_capacity(items.len().min(super::MAX_VECTOR_DIMENSION));
            for item in items {
                let value = number(item)
                    .ok_or_else(|| format!("{VECTOR_TAG}: \"data\" holds numbers only"))?;
                values.push(value);
            }
            let vector = Vector::from_f64(&values).map_err(bad)?;
            if element == VectorElement::F32 {
                return Ok(vector);
            }
            let half = vector.quantize(VectorElement::F16).map_err(bad)?;
            // Every half value is exact as a number; anything else was not written by us.
            if half.to_f32_vec() != vector.to_f32_vec() {
                return Err(format!(
                    "{VECTOR_TAG}: \"data\" of an f16 vector holds half values"
                ));
            }
            Ok(half)
        }
        VectorElement::I8 => {
            let scale = get("scale")
                .and_then(number)
                .ok_or_else(|| format!("{VECTOR_TAG} needs a number \"scale\""))?;
            let Some(PhysicalValue::Array(items)) = get("codes") else {
                return Err(format!("{VECTOR_TAG} needs an array \"codes\""));
            };
            let mut codes = Vec::with_capacity(items.len().min(super::MAX_VECTOR_DIMENSION));
            for item in items {
                match item {
                    PhysicalValue::Int(code) => codes.push(
                        i8::try_from(*code)
                            .map_err(|_| format!("{VECTOR_TAG}: code {code} is not an i8"))?,
                    ),
                    _ => return Err(format!("{VECTOR_TAG}: \"codes\" holds integers only")),
                }
            }
            let narrowed = scale as f32;
            if f64::from(narrowed) != scale && short_f64(narrowed) != scale {
                return Err(format!("{VECTOR_TAG}: the scale {scale} is not an f32"));
            }
            Vector::from_i8(codes, narrowed).map_err(bad)
        }
        VectorElement::Bit => {
            let Some(PhysicalValue::Int(dimension)) = get("dimension") else {
                return Err(format!("{VECTOR_TAG} needs an integer \"dimension\""));
            };
            let Some(PhysicalValue::String(hex)) = get("bits") else {
                return Err(format!("{VECTOR_TAG} needs a hex string \"bits\""));
            };
            let dimension = usize::try_from(*dimension)
                .map_err(|_| format!("{VECTOR_TAG}: dimension {dimension}"))?;
            let digits = hex.as_bytes();
            if digits.len() % 2 != 0 {
                return Err(format!(
                    "{VECTOR_TAG}: \"bits\" has an odd number of hex digits"
                ));
            }
            let nibble = |digit: u8| match digit {
                b'0'..=b'9' => Some(digit - b'0'),
                b'a'..=b'f' => Some(digit - b'a' + 10),
                _ => None,
            };
            let mut bytes = Vec::with_capacity(digits.len() / 2);
            for pair in digits.as_chunks::<2>().0 {
                match (nibble(pair[0]), nibble(pair[1])) {
                    (Some(high), Some(low)) => bytes.push(high << 4 | low),
                    _ => return Err(format!("{VECTOR_TAG}: \"bits\" is lowercase hex")),
                }
            }
            Vector::from_bits(bytes, dimension).map_err(bad)
        }
    }
}
