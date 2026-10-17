//! IEEE 754 binary16 ("half") conversion, own code instead of the `half` crate: two functions,
//! round to nearest, ties to even, tested over every half value.

/// The largest finite half value (65504).
#[cfg(test)]
pub(crate) const F16_MAX: f32 = 65504.0;

/// Shifts `value` right by `shift` (1..=31) bits, rounding to nearest, ties to even.
fn round_shift(value: u32, shift: u32) -> u32 {
    let kept = value >> shift;
    let rest = value & ((1u32 << shift) - 1);
    let half = 1u32 << (shift - 1);
    if rest > half || (rest == half && kept & 1 == 1) {
        kept + 1
    } else {
        kept
    }
}

/// The half bits nearest to `value` (ties to even). A value beyond the half range becomes an
/// infinity, a NaN stays a NaN: the callers refuse both.
pub(crate) fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;
    if exponent == 0xff {
        // Infinity or NaN (a NaN keeps a mantissa bit).
        return sign | 0x7c00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        // A half subnormal (or zero): value = m * 2^-24 with m = significand >> (14 - e).
        if exponent == 0 {
            // An f32 subnormal is far below the smallest half subnormal.
            return sign;
        }
        let shift = (14 - half_exponent) as u32;
        if shift >= 25 {
            return sign;
        }
        let significand = mantissa | 0x0080_0000;
        // Rounding up past 0x3ff gives 0x400: the smallest normal half, by the bit layout.
        return sign | round_shift(significand, shift) as u16;
    }
    // A normal half: exponent and mantissa shifted together, so a carry out of the mantissa
    // raises the exponent (and past the largest exponent gives the infinity 0x7c00).
    let combined = ((half_exponent as u32) << 23) | mantissa;
    sign | round_shift(combined, 13) as u16
}

/// The f32 value of half bits (exact: every half value is an f32 value).
pub(crate) fn f16_to_f32(half: u16) -> f32 {
    let negative = half & 0x8000 != 0;
    let exponent = u32::from((half >> 10) & 0x1f);
    let mantissa = u32::from(half & 0x03ff);
    let magnitude = if exponent == 0 {
        // Zero or subnormal: mantissa * 2^-24, exact in f32.
        mantissa as f32 * (1.0 / 16_777_216.0)
    } else if exponent == 0x1f {
        f32::from_bits(0x7f80_0000 | (mantissa << 13))
    } else {
        f32::from_bits(((exponent + 112) << 23) | (mantissa << 13))
    };
    if negative { -magnitude } else { magnitude }
}

/// Whether half bits are a finite value (not an infinity, not a NaN).
pub(crate) fn f16_is_finite(half: u16) -> bool {
    half & 0x7c00 != 0x7c00
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_finite_half_round_trips_through_f32() {
        for bits in 0..=u16::MAX {
            if !f16_is_finite(bits) {
                continue;
            }
            let value = f16_to_f32(bits);
            assert_eq!(f32_to_f16(value), bits, "half {bits:#06x} = {value}");
        }
    }

    #[test]
    fn rounding_is_to_nearest_even() {
        // 1 + 2^-11 lies exactly between 1 and 1 + 2^-10: ties to the even mantissa (1).
        assert_eq!(f32_to_f16(1.0 + 2f32.powi(-11)), 0x3c00);
        // 1 + 3 * 2^-11 lies between 1 + 2^-10 (odd) and 1 + 2^-9 (even): rounds up.
        assert_eq!(f32_to_f16(1.0 + 3.0 * 2f32.powi(-11)), 0x3c02);
        // Slightly above the tie rounds up.
        assert_eq!(f32_to_f16(1.0 + 2f32.powi(-11) + 2f32.powi(-20)), 0x3c01);
        // The smallest subnormal and half of it (a tie to the even zero).
        assert_eq!(f32_to_f16(2f32.powi(-24)), 0x0001);
        assert_eq!(f32_to_f16(2f32.powi(-25)), 0x0000);
        assert_eq!(f32_to_f16(1.5 * 2f32.powi(-24)), 0x0002);
        // Overflow, the largest value, and a value rounding up to infinity.
        assert_eq!(f32_to_f16(F16_MAX), 0x7bff);
        assert_eq!(f32_to_f16(65520.0), 0x7c00);
        assert_eq!(f32_to_f16(1e10), 0x7c00);
        assert_eq!(f32_to_f16(-0.0), 0x8000);
        assert!(f32_to_f16(f32::NAN) & 0x03ff != 0);
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
        // The largest subnormal rounding up into the smallest normal.
        assert_eq!(f32_to_f16(2f32.powi(-14) - 2f32.powi(-26)), 0x0400);
    }
}
