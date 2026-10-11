//! The generator itself: the SplitMix64 reference sequence, bounds and the shapes of the drawn
//! values. The script-level behaviour (golden values of a seed, sharing, forks, errors) is
//! covered by `tests/all/fake_tests.rs`.

use super::*;

/// SplitMix64 from state 0 must produce the published reference sequence, so the generator is
/// the specified algorithm and not something that merely looks random.
#[test]
fn splitmix64_reference_sequence() {
    let mut rng = Rng { state: 0 };
    assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
}

#[test]
fn fnv1a_is_the_standard_hash() {
    assert_eq!(fnv1a(""), 0xCBF2_9CE4_8422_2325);
    assert_eq!(fnv1a("a"), 0xAF63_DC4C_8601_EC8C);
}

#[test]
fn int_stays_in_bounds_and_hits_both_ends() {
    let mut rng = Rng { state: 7 };
    let mut seen = [false; 6];
    for _ in 0..10_000 {
        let v = rng.int(1, 6);
        assert!((1..=6).contains(&v));
        seen[(v - 1) as usize] = true;
    }
    assert!(seen.iter().all(|s| *s));
    // The full i64 range and a one-value range work without overflow.
    let _ = rng.int(i64::MIN, i64::MAX);
    assert_eq!(rng.int(i64::MAX, i64::MAX), i64::MAX);
    assert_eq!(rng.int(i64::MIN, i64::MIN), i64::MIN);
}

#[test]
fn int_is_roughly_uniform() {
    let mut rng = Rng { state: 11 };
    let mut counts = [0u32; 10];
    for _ in 0..100_000 {
        counts[rng.int(0, 9) as usize] += 1;
    }
    for count in counts {
        assert!((9_000..11_000).contains(&count), "{counts:?}");
    }
}

#[test]
fn below_zero_does_not_panic() {
    let mut rng = Rng { state: 1 };
    assert_eq!(rng.below(0), 0);
    assert_eq!(rng.below(1), 0);
}

#[test]
fn unit_is_half_open() {
    let mut rng = Rng { state: 3 };
    for _ in 0..10_000 {
        let v = rng.unit();
        assert!((0.0..1.0).contains(&v));
    }
}

#[test]
fn float_between_is_half_open_and_allows_an_empty_interval() {
    let f = ScriptFake::new(5);
    for _ in 0..1_000 {
        let v = f.float_between(-2.5, 2.5).unwrap_or(f64::NAN);
        assert!((-2.5..2.5).contains(&v));
    }
    assert_eq!(f.float_between(3.0, 3.0).ok(), Some(3.0));
    assert!(f.float_between(1.0, 0.0).is_err());
    assert!(f.float_between(0.0, f64::INFINITY).is_err());
}

#[test]
fn shuffle_is_a_permutation_and_sample_is_distinct() {
    let f = ScriptFake::new(9);
    let items: Array = (0..50).map(Dynamic::from_int).collect();
    let mut shuffled: Vec<INT> = f
        .shuffle(items.clone())
        .iter()
        .map(|d| d.as_int().unwrap_or(-1))
        .collect();
    shuffled.sort_unstable();
    assert_eq!(shuffled, (0..50).collect::<Vec<_>>());
    let sample = f.sample(items.clone(), 20).unwrap_or_default();
    let mut ints: Vec<INT> = sample.iter().map(|d| d.as_int().unwrap_or(-1)).collect();
    ints.sort_unstable();
    ints.dedup();
    assert_eq!(ints.len(), 20);
    assert_eq!(f.sample(items.clone(), 0).map(|a| a.len()).ok(), Some(0));
    assert_eq!(f.sample(items.clone(), 50).map(|a| a.len()).ok(), Some(50));
    assert!(f.sample(items.clone(), 51).is_err());
    assert!(f.sample(items, -1).is_err());
}

#[test]
fn uuid_is_version_4_rfc_variant() {
    let f = ScriptFake::new(1);
    for _ in 0..100 {
        let text = f.uuid().canonical();
        assert_eq!(&text[14..15], "4", "{text}");
        assert!(matches!(&text[19..20], "8" | "9" | "a" | "b"), "{text}");
    }
}

#[test]
fn generated_text_has_its_shape() {
    let f = ScriptFake::new(123);
    for _ in 0..200 {
        let email = f.email();
        assert!(
            email.contains('@') && EMAIL_DOMAINS.iter().any(|d| email.ends_with(d)),
            "{email}"
        );
        let phone = f.phone();
        assert!(
            phone.starts_with("+1-") && phone.contains("-555-01"),
            "{phone}"
        );
        assert_eq!(f.zip_code().len(), 5);
        let sentence = f.sentence();
        assert!(sentence.ends_with('.'));
        assert!(
            sentence
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
        );
        let words = sentence.split(' ').count();
        assert!((4..=12).contains(&words), "{sentence}");
        let title = f.job_title();
        assert!(!title.starts_with(' ') && !title.contains("  "), "{title}");
    }
}

#[test]
fn an_address_keeps_city_and_state_together() {
    let f = ScriptFake::new(77);
    for _ in 0..200 {
        let address = f.address();
        assert!(
            CITIES
                .iter()
                .any(|(city, state)| address.contains(&format!(", {city}, {state} "))),
            "{address}"
        );
    }
}

#[test]
fn word_lists_have_no_duplicates() {
    fn check(name: &str, items: &[&str]) {
        let mut sorted = items.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "duplicate in {name}");
    }
    check("FIRST_NAMES", FIRST_NAMES);
    check("LAST_NAMES", LAST_NAMES);
    check("COUNTRIES", COUNTRIES);
    check("STREET_ROOTS", STREET_ROOTS);
    check("WORDS", WORDS);
    check("LOREM_WORDS", LOREM_WORDS);
    let states: Vec<&str> = STATES.iter().map(|s| s.1).collect();
    check("STATES", &states);
}

#[test]
fn lorem_has_a_length_in_range_and_its_shape() {
    let f = ScriptFake::new(7);
    for (lo, hi) in [(0, 0), (1, 1), (5, 40), (200, 4000), (10_000, 10_000)] {
        for _ in 0..50 {
            let text = f.lorem(lo, hi).expect("valid range");
            let len = text.len() as INT;
            assert!((lo..=hi).contains(&len), "{lo}..={hi}: {len}");
            if len > 0 {
                assert!(text.ends_with('.'), "{text}");
            }
            assert!(text.is_ascii());
            if len > LOREM_START.len() as INT {
                assert!(text.starts_with(LOREM_START));
            }
        }
    }
    let long = f.lorem(20_000, 20_000).expect("valid range");
    assert!(long.contains("\n\n"), "paragraphs");
}

#[test]
fn lorem_rejects_a_bad_range() {
    let f = ScriptFake::new(1);
    for (lo, hi) in [(-1, 5), (10, 5), (0, MAX_LOREM_BYTES + 1)] {
        assert!(f.lorem(lo, hi).is_err(), "{lo}..={hi}");
    }
}

#[test]
fn lorem_sentences_and_paragraphs_have_their_shape() {
    let f = ScriptFake::new(3);
    for _ in 0..200 {
        let sentence = f.lorem_sentence();
        assert!(sentence.ends_with('.'));
        assert!(
            sentence.starts_with(|c: char| c.is_ascii_uppercase()),
            "{sentence}"
        );
        let words = sentence.split(' ').count();
        assert!((6..=14).contains(&words), "{sentence}");
        let sentences = f.lorem_paragraph().matches('.').count();
        assert!((3..=7).contains(&sentences));
    }
}
