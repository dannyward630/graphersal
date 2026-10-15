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

/// A deterministic pseudo-random stream in -1..1.
fn pseudo_random(seed: u64, n: usize) -> Vec<f32> {
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 40) as f64 / (1u64 << 24) as f64 * 2.0 - 1.0) as f32
        })
        .collect()
}

/// The naive reference: every component dequantized to f64, one sequential sum.
fn reference(a: &Vector, b: &Vector, metric: VectorMetric) -> f64 {
    let x: Vec<f64> = a.iter_f32().map(f64::from).collect();
    let y: Vec<f64> = b.iter_f32().map(f64::from).collect();
    let dot: f64 = x.iter().zip(&y).map(|(p, q)| p * q).sum();
    match metric {
        VectorMetric::Cosine => {
            let na: f64 = x.iter().map(|p| p * p).sum::<f64>().sqrt();
            let nb: f64 = y.iter().map(|q| q * q).sum::<f64>().sqrt();
            if na == 0.0 || nb == 0.0 {
                0.0
            } else {
                (dot / (na * nb)).clamp(-1.0, 1.0)
            }
        }
        VectorMetric::Dot => dot,
        VectorMetric::L2 => x
            .iter()
            .zip(&y)
            .map(|(p, q)| (p - q) * (p - q))
            .sum::<f64>()
            .sqrt(),
        VectorMetric::Hamming => x.iter().zip(&y).filter(|(p, q)| p != q).count() as f64,
    }
}

fn close(left: f64, right: f64) -> bool {
    // i8 x i8 runs on the integer codes (exact for code * scale), the reference on the f32
    // products: they differ in the f32 rounding of the products.
    (left - right).abs() <= 1e-6 * (1.0 + left.abs().max(right.abs()))
}

#[test]
fn kernels_match_the_naive_reference_for_every_kind_pair() {
    // Dimensions around the chunk size (8) and the bit byte boundaries.
    for dimension in [1, 3, 7, 8, 9, 16, 17, 64, 65, 384] {
        for seed in 0..4u64 {
            let a = Vector::from_f32(pseudo_random(seed, dimension)).unwrap();
            let b = Vector::from_f32(pseudo_random(seed + 100, dimension)).unwrap();
            let kinds = [VectorElement::F32, VectorElement::F16, VectorElement::I8];
            for ka in kinds {
                for kb in kinds {
                    let x = a.quantize(ka).unwrap();
                    let y = b.quantize(kb).unwrap();
                    for metric in [VectorMetric::Cosine, VectorMetric::Dot, VectorMetric::L2] {
                        let got = x.similarity(&y, metric).unwrap();
                        let want = reference(&x, &y, metric);
                        assert!(
                            close(got, want),
                            "{ka} x {kb} {metric} d={dimension}: {got} vs {want}"
                        );
                        assert!(close(got, y.similarity(&x, metric).unwrap()), "symmetric");
                        if ka != VectorElement::I8 || kb != VectorElement::I8 {
                            // Dequantized lanes in f64: only the order of the additions
                            // differs from the reference.
                            assert!((got - want).abs() <= 1e-12 * (1.0 + want.abs()));
                        }
                    }
                    assert!(matches!(
                        x.similarity(&y, VectorMetric::Hamming),
                        Err(VectorError::HammingNeedsBits { .. })
                    ));
                }
            }
            let x = a.quantize(VectorElement::Bit).unwrap();
            let y = b.quantize(VectorElement::Bit).unwrap();
            for metric in VectorMetric::ALL {
                let got = x.similarity(&y, metric).unwrap();
                let want = reference(&x, &y, metric);
                assert!(
                    close(got, want),
                    "bit {metric} d={dimension}: {got} vs {want}"
                );
            }
            assert!(matches!(
                x.similarity(&a, VectorMetric::Cosine),
                Err(VectorError::BitMismatch {
                    element: VectorElement::F32
                })
            ));
            assert!(matches!(
                a.similarity(&x, VectorMetric::L2),
                Err(VectorError::BitMismatch { .. })
            ));
        }
    }
}

#[test]
fn similarity_edge_cases() {
    let zero = Vector::from_f32(vec![0.0, 0.0]).unwrap();
    let one = Vector::from_f32(vec![1.0, 0.0]).unwrap();
    // A zero vector has cosine 0.0, never NaN.
    assert_eq!(zero.similarity(&one, VectorMetric::Cosine).unwrap(), 0.0);
    assert_eq!(zero.similarity(&zero, VectorMetric::Cosine).unwrap(), 0.0);
    let zero_i8 = zero.quantize(VectorElement::I8).unwrap();
    assert_eq!(
        zero_i8.similarity(&zero_i8, VectorMetric::Cosine).unwrap(),
        0.0
    );
    // The same direction is 1.0 (clamped, never above).
    let v = Vector::from_f32(pseudo_random(7, 384)).unwrap();
    let cosine = v.similarity(&v, VectorMetric::Cosine).unwrap();
    assert!((cosine - 1.0).abs() < 1e-12 && cosine <= 1.0);
    assert_eq!(v.similarity(&v, VectorMetric::L2).unwrap(), 0.0);
    // Dimension mismatch is an error, never a truncation.
    let three = Vector::from_f32(vec![1.0, 0.0, 0.0]).unwrap();
    assert_eq!(
        one.similarity(&three, VectorMetric::Dot),
        Err(VectorError::DimensionMismatch { left: 2, right: 3 })
    );
    // Large components: f64 accumulation, no overflow.
    let big = Vector::from_f32(vec![f32::MAX; 4]).unwrap();
    let dot = big.similarity(&big, VectorMetric::Dot).unwrap();
    assert!(dot.is_finite() && dot > 1e77);
    for error in [
        VectorError::DimensionMismatch { left: 2, right: 3 },
        VectorError::BitMismatch {
            element: VectorElement::I8,
        },
        VectorError::HammingNeedsBits {
            element: VectorElement::F32,
        },
        VectorError::QueryNotConstant,
        VectorError::MetricConflict {
            first: VectorMetric::Cosine,
            second: VectorMetric::L2,
        },
        VectorError::unknown_metric("manhattan"),
    ] {
        assert!(!error.help().is_empty());
        assert!(!error.to_string().is_empty());
    }
    assert!(VectorMetric::Cosine.higher_is_better() && !VectorMetric::L2.higher_is_better());
}

#[test]
fn mean_accumulator() {
    use super::similarity::MeanAccumulator;
    let mut mean = MeanAccumulator::new();
    mean.add(&Vector::from_f32(vec![1.0, 0.0]).unwrap(), 3)
        .unwrap();
    mean.add(&Vector::from_i8(vec![0, 127], 1.0 / 127.0).unwrap(), 1)
        .unwrap();
    assert_eq!(
        mean.add(&Vector::from_f32(vec![1.0]).unwrap(), 1),
        Err(VectorError::DimensionMismatch { left: 2, right: 1 })
    );
    let result = mean.finish().unwrap().unwrap();
    assert_eq!(result.to_f32_vec(), vec![0.75, 0.25]);
    assert!(MeanAccumulator::new().finish().unwrap().is_none());
}
