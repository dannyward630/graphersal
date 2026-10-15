use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::*;

fn hash_of(vector: &Vector) -> u64 {
    let mut hasher = DefaultHasher::new();
    vector.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn construction_checks_dimension_and_finiteness() {
    assert!(matches!(
        Vector::from_f32(Vec::new()),
        Err(VectorError::Dimension { dimension: 0 })
    ));
    assert!(Vector::from_f32(vec![0.0; MAX_VECTOR_DIMENSION]).is_ok());
    assert!(matches!(
        Vector::from_f32(vec![0.0; MAX_VECTOR_DIMENSION + 1]),
        Err(VectorError::Dimension { .. })
    ));
    assert!(matches!(
        Vector::from_f32(vec![1.0, f32::NAN]),
        Err(VectorError::NotFinite { index: 1, .. })
    ));
    assert!(matches!(
        Vector::from_f32(vec![f32::NEG_INFINITY]),
        Err(VectorError::NotFinite { index: 0, .. })
    ));
    assert!(matches!(
        Vector::from_f64(&[1.0, 1e300]),
        Err(VectorError::NotFinite { index: 1, .. })
    ));
    assert!(matches!(
        Vector::from_f16_bits(vec![0x3c00, 0x7c00]),
        Err(VectorError::NotFinite { index: 1, .. })
    ));
    assert!(matches!(
        Vector::from_i8(vec![1, 2], -1.0),
        Err(VectorError::Scale { .. })
    ));
    assert!(matches!(
        Vector::from_i8(vec![1, 2], f32::NAN),
        Err(VectorError::Scale { .. })
    ));
    assert!(matches!(
        Vector::from_bits(vec![0xff], 9),
        Err(VectorError::Bits { .. })
    ));
    // Padding bits of the last byte must be 0.
    assert!(matches!(
        Vector::from_bits(vec![0b1010_0001], 3),
        Err(VectorError::Bits { .. })
    ));
    let bits = Vector::from_bits(vec![0b1010_0000], 3).unwrap();
    assert_eq!(bits.to_f32_vec(), vec![1.0, -1.0, 1.0]);
}

#[test]
fn negative_zero_is_kept_and_equals_zero() {
    let a = Vector::from_f32(vec![-0.0, 1.0]).unwrap();
    let b = Vector::from_f32(vec![0.0, 1.0]).unwrap();
    assert!(a.as_f32().unwrap()[0].is_sign_negative());
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_eq!(a.cmp(&b), Ordering::Equal);
    // A -0.0 scale is stored as 0.0.
    let z = Vector::from_i8(vec![0, 0], -0.0).unwrap();
    assert!(z.as_i8().unwrap().1.is_sign_positive());
}

#[test]
fn quantization_between_every_kind() {
    let v = Vector::from_f32(vec![0.5, -1.0, 0.25, 0.0, 2.0]).unwrap();
    let i8 = v.quantize(VectorElement::I8).unwrap();
    let (codes, scale) = i8.as_i8().unwrap();
    assert_eq!(scale, 2.0 / 127.0);
    assert_eq!(codes, &[32, -64, 16, 0, 127]);
    let f16 = v.quantize(VectorElement::F16).unwrap();
    assert_eq!(f16.to_f32_vec(), vec![0.5, -1.0, 0.25, 0.0, 2.0]);
    let bit = v.quantize(VectorElement::Bit).unwrap();
    assert_eq!(bit.as_bits().unwrap(), &[0b1010_1000]);
    assert_eq!(bit.dimension(), 5);
    assert_eq!(bit.to_f32_vec(), vec![1.0, -1.0, 1.0, -1.0, 1.0]);
    // Back to f32 dequantizes; the same kind is the same vector.
    assert_eq!(
        f16.quantize(VectorElement::F32).unwrap(),
        v,
        "halves of these values are exact"
    );
    assert!(Arc::ptr_eq(
        &v.quantize(VectorElement::F32).unwrap().0,
        &v.0
    ));
    // An f16 overflow is an error, never an infinity.
    let big = Vector::from_f32(vec![1.0, 70000.0]).unwrap();
    assert!(matches!(
        big.quantize(VectorElement::F16),
        Err(VectorError::OutOfRange { index: 1, .. })
    ));
    // The zero vector quantizes to zero codes with scale 0.
    let zero = Vector::from_f32(vec![0.0; 3]).unwrap();
    let q = zero.quantize(VectorElement::I8).unwrap();
    assert_eq!(q.as_i8().unwrap(), (&[0i8, 0, 0][..], 0.0));
    // A literal of a quantized vector evaluates (quantizing its values) to the same vector.
    let again = Vector::from_f32(i8.to_f32_vec())
        .unwrap()
        .quantize(VectorElement::I8)
        .unwrap();
    assert_eq!(again, i8);
    let again = Vector::from_f32(bit.to_f32_vec())
        .unwrap()
        .quantize(VectorElement::Bit)
        .unwrap();
    assert_eq!(again, bit);
}

#[test]
fn order_by_kind_dimension_then_components() {
    let f32a = Vector::from_f32(vec![1.0, 2.0]).unwrap();
    let f32b = Vector::from_f32(vec![1.0, 3.0]).unwrap();
    let f32c = Vector::from_f32(vec![0.0, 0.0, 0.0]).unwrap();
    let f16 = f32a.quantize(VectorElement::F16).unwrap();
    let i8 = f32a.quantize(VectorElement::I8).unwrap();
    let bit = f32a.quantize(VectorElement::Bit).unwrap();
    let mut all = vec![
        bit.clone(),
        i8.clone(),
        f16.clone(),
        f32c.clone(),
        f32b.clone(),
        f32a.clone(),
    ];
    all.sort();
    assert_eq!(all, vec![f32a, f32b, f32c, f16, i8, bit]);
    // Zero i8 vectors with different scales are different and ordered by the scale.
    let z1 = Vector::from_i8(vec![0, 0], 0.0).unwrap();
    let z2 = Vector::from_i8(vec![0, 0], 1.0).unwrap();
    assert_ne!(z1, z2);
    assert_eq!(z1.cmp(&z2), Ordering::Less);
}

#[test]
fn equality_is_per_kind() {
    let v = Vector::from_f32(vec![1.0, -1.0]).unwrap();
    let bit = v.quantize(VectorElement::Bit).unwrap();
    // Same values, different kinds: not equal.
    assert_eq!(bit.to_f32_vec(), v.to_f32_vec());
    assert_ne!(bit, v);
    assert_ne!(hash_of(&bit), hash_of(&v));
}

#[test]
fn literals_and_summary() {
    let v = Vector::from_f32(vec![0.1, -2.0, 1e20]).unwrap();
    assert_eq!(v.to_string(), "vector([0.1, -2.0, 1e20])");
    assert_eq!(v.summary(), "vector[3 f32]");
    let i8 = Vector::from_i8(vec![127, -127], 0.5).unwrap();
    assert_eq!(i8.to_string(), "vector([63.5, -63.5], \"i8\")");
    let long = Vector::from_f32(vec![1.0; 20]).unwrap();
    assert_eq!(long.literal(Some(2)), "vector([1.0, 1.0, …]) (20 items)");
    assert_eq!(VectorElement::from_name("bit"), Some(VectorElement::Bit));
    assert_eq!(VectorElement::from_name("F32"), None);
    for kind in VectorElement::ALL {
        assert_eq!(VectorElement::from_code(kind.code()), Some(kind));
    }
}
