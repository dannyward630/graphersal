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

/// The version and variant bits of a UUID (`ver` in bits 76..80, variant `10` in bits 62..64).
const VERSION_VARIANT_MASK: u128 = (0xF << 76) | (0b11 << 62);

/// A new random RFC 9562 version-4 UUID: 122 random bits, the version and variant bits.
pub(crate) fn new_v4() -> u128 {
    let random: u128 = rand::random();
    (random & !VERSION_VARIANT_MASK) | (0x4 << 76) | (0b10 << 62)
}

/// The last version-7 UUID this process made, for monotonicity.
static LAST_V7: parking_lot::Mutex<u128> = parking_lot::Mutex::new(0);

/// A new RFC 9562 version-7 UUID: 48 bits of Unix milliseconds (through `web_time`, never
/// `std::time`), the version and variant bits, and 74 random bits.
///
/// Monotonic within the process (RFC 9562 section 6.2, method 2 on the whole random part): when
/// the clock has not moved past the last UUID's millisecond (same millisecond, or a clock
/// step back), the new UUID is the last one plus one in its 74 random bits, carrying into the
/// timestamp when they are exhausted, so UUIDs made one after the other always increase.
pub(crate) fn new_v7() -> u128 {
    let millis = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
        & 0xFFFF_FFFF_FFFF;
    let random: u128 = rand::random();
    let fresh = (millis << 80) | (random & !(VERSION_VARIANT_MASK | (u128::MAX << 80))) | V7_BITS;
    let mut last = LAST_V7.lock();
    let next = if fresh > *last {
        fresh
    } else {
        increment_v7(*last)
    };
    *last = next;
    next
}

/// The version-7 and variant bits.
const V7_BITS: u128 = (0x7 << 76) | (0b10 << 62);

/// `uuid` plus one in its 74 random bits (rand_a: bits 64..76, rand_b: bits 0..62), carrying
/// into the 48-bit timestamp; the version and variant bits stay.
fn increment_v7(uuid: u128) -> u128 {
    let millis = uuid >> 80;
    let rand_a = (uuid >> 64) & 0xFFF;
    let rand_b = uuid & 0x3FFF_FFFF_FFFF_FFFF;
    let counter = ((rand_a << 62) | rand_b).wrapping_add(1) & ((1 << 74) - 1);
    let millis = if counter == 0 {
        (millis + 1) & 0xFFFF_FFFF_FFFF
    } else {
        millis
    };
    (millis << 80) | ((counter >> 62) << 64) | (counter & 0x3FFF_FFFF_FFFF_FFFF) | V7_BITS
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

    #[test]
    fn v4_has_version_and_variant() {
        for _ in 0..100 {
            let uuid = new_v4();
            assert_eq!((uuid >> 76) & 0xF, 4);
            assert_eq!((uuid >> 62) & 0b11, 0b10);
        }
        assert_ne!(new_v4(), new_v4());
    }

    #[test]
    fn v7_has_version_variant_and_increases() {
        let mut previous = new_v7();
        for _ in 0..10_000 {
            let uuid = new_v7();
            assert_eq!((uuid >> 76) & 0xF, 7);
            assert_eq!((uuid >> 62) & 0b11, 0b10);
            assert!(uuid > previous, "v7 UUIDs of one process increase");
            previous = uuid;
        }
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let stamp = previous >> 80;
        assert!(
            stamp + 60_000 > now && stamp <= now + 60_000,
            "48-bit Unix milliseconds"
        );
    }

    #[test]
    fn v7_increment_carries_into_the_timestamp() {
        let full = (5u128 << 80) | (0xFFF << 64) | 0x3FFF_FFFF_FFFF_FFFF | V7_BITS;
        assert_eq!(increment_v7(full), (6u128 << 80) | V7_BITS);
        let low = (5u128 << 80) | 0x3FFF_FFFF_FFFF_FFFF | V7_BITS;
        assert_eq!(increment_v7(low), (5u128 << 80) | (1 << 64) | V7_BITS);
    }
}
