//! Canonical RFC 9562 UUID parsing and rendering.
//!
//! Hand-rolled rather than pulled from the `uuid` crate (`AGENTS.md` rule 3, design decision D7
//! open question 4): canonical parsing and rendering is roughly 40 lines of hex handling, well
//! under the bar that would justify a new dependency for a single crate's worth of use.

use crate::value::CompactString;

const HYPHEN_POSITIONS: [usize; 4] = [8, 13, 18, 23];
const GROUP_LENGTHS: [usize; 5] = [8, 4, 4, 4, 12];

/// Parses a canonical, lowercase, hyphenated UUID string (`8-4-4-4-12` hex digits).
///
/// Strict on purpose: no braces, no uppercase, no partial input. A near-miss (wrong hyphen
/// position, wrong length, a non-hex character) is rejected rather than guessed at.
pub(crate) fn parse_canonical(input: &str) -> Option<u128> {
    let bytes = input.as_bytes();
    if bytes.len() != 36 {
        return None;
    }
    for &pos in &HYPHEN_POSITIONS {
        if bytes[pos] != b'-' {
            return None;
        }
    }
    let mut value: u128 = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if HYPHEN_POSITIONS.contains(&i) {
            continue;
        }
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            _ => return None,
        };
        value = (value << 4) | (digit as u128);
    }
    Some(value)
}

/// A new RFC 9562 version-7 UUID: 48 bits of Unix milliseconds (through `web_time`, never
/// `std::time`), the version and variant bits, and 74 random bits.
#[cfg_attr(not(feature = "persist"), allow(dead_code))]
pub(crate) fn new_v7() -> u128 {
    let millis = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
        & 0xFFFF_FFFF_FFFF;
    let random: u128 = rand::random();
    (millis << 80)
        | (0x7 << 76)
        | (random & (0xFFF << 64))
        | (0b10 << 62)
        | (random & 0x3FFF_FFFF_FFFF_FFFF)
}

/// Renders a `u128` as a canonical, lowercase, hyphenated UUID string.
pub(crate) fn format_canonical(value: u128) -> CompactString {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut hex = [0u8; 32];
    for (i, slot) in hex.iter_mut().enumerate() {
        let shift = (31 - i) * 4;
        let nibble = ((value >> shift) & 0xF) as usize;
        *slot = HEX_DIGITS[nibble];
    }

    let mut out = [0u8; 36];
    let mut hi = 0;
    let mut oi = 0;
    for (group_idx, &group_len) in GROUP_LENGTHS.iter().enumerate() {
        if group_idx != 0 {
            out[oi] = b'-';
            oi += 1;
        }
        out[oi..oi + group_len].copy_from_slice(&hex[hi..hi + group_len]);
        oi += group_len;
        hi += group_len;
    }

    // `out` is built exclusively from ASCII hex digits and hyphens.
    CompactString::new(std::str::from_utf8(&out).expect("canonical UUID rendering is ASCII"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_canonical_form() {
        let canonical = "550e8400-e29b-41d4-a716-446655440000";
        let value = parse_canonical(canonical).expect("valid canonical UUID");
        assert_eq!(format_canonical(value), canonical);
    }

    #[test]
    fn round_trips_zero_and_max() {
        assert_eq!(format_canonical(0), "00000000-0000-0000-0000-000000000000");
        let max_str = "ffffffff-ffff-ffff-ffff-ffffffffffff";
        assert_eq!(parse_canonical(max_str), Some(u128::MAX));
        assert_eq!(format_canonical(u128::MAX), max_str);
    }

    #[test]
    fn rejects_near_misses() {
        // Wrong length: 35 and 37 characters.
        assert_eq!(parse_canonical("550e8400-e29b-41d4-a716-44665544000"), None);
        assert_eq!(
            parse_canonical("550e8400-e29b-41d4-a716-4466554400000"),
            None
        );
        // Wrong hyphen positions.
        assert_eq!(
            parse_canonical("550e8400e-29b-41d4-a716-446655440000"),
            None
        );
        // Non-hex characters.
        assert_eq!(
            parse_canonical("550e8400-e29b-41d4-a716-44665544000g"),
            None
        );
        // Uppercase is not accepted on parse.
        assert_eq!(
            parse_canonical("550E8400-E29B-41D4-A716-446655440000"),
            None
        );
        // Braces.
        assert_eq!(
            parse_canonical("{550e8400-e29b-41d4-a716-446655440000}"),
            None
        );
    }
}
