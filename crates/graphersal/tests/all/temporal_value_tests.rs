//! Temporal values (`graphersal::temporal`) as property values through the Rust API: stored and
//! read back, compared by `has()` (a datetime by its instant), ordered, deduplicated, grouped,
//! type-checked with `P::type_of`, converted by casts, rendered, and refused cleanly by the
//! formats that cannot hold them yet (binary snapshot, GraphSON, change-set serde: phase A2).

use graphersal::prelude::*;
use graphersal::schema::ValueType;
use graphersal::temporal::{Date, DateTime, Duration, LocalDateTime, Temporal, Time};

fn dt(text: &str) -> DateTime {
    text.parse().unwrap()
}

fn temporal(value: &ElementProperty) -> Temporal {
    value
        .as_temporal()
        .unwrap_or_else(|| panic!("expected a temporal value, got {value:?}"))
}

/// Every field of a value, so `12:00Z` and `14:00+02:00` are told apart.
fn text(value: &ElementProperty) -> String {
    temporal(value).to_string()
}

/// A graph of events with a `at` datetime, a `day` date, a `span` duration and more.
fn events() -> Graph {
    let graph = GraphSource::empty();
    {
        let mut lock = graph.write();
        let rows: [(&str, &str, &str, &str); 4] = [
            ("a", "2023-08-02T12:00:00Z", "2023-08-02", "P1D"),
            ("b", "2023-08-02T14:00:00+02:00", "2023-08-01", "PT24H"),
            ("c", "2023-08-01T23:00:00-07:00", "2023-08-03", "P1M"),
            ("d", "2020-01-01T00:00:00Z", "2023-08-02", "P30D"),
        ];
        for (name, at, day, span) in rows {
            lock.traversal_mut()
                .add_v("event")
                .property("name", name)
                .property("at", dt(at))
                .property("day", day.parse::<Date>().unwrap())
                .property("span", span.parse::<Duration>().unwrap())
                .property(
                    "local",
                    "2023-08-02T09:30:00".parse::<LocalDateTime>().unwrap(),
                )
                .property("time", "09:30:00.25".parse::<Time>().unwrap())
                .next()
                .unwrap();
        }
    }
    graph
}

#[test]
fn a_temporal_property_is_stored_and_read_back_unchanged() {
    let graph = events();
    let lock = graph.read();
    let at = lock
        .traversal()
        .v(None)
        .has("name", "b")
        .values("at")
        .to_list()
        .unwrap();
    // The offset it was written with is kept.
    assert_eq!(at.len(), 1);
    assert_eq!(text(&at[0]), "2023-08-02T14:00:00+02:00");
    let local = lock
        .traversal()
        .v(None)
        .has("name", "a")
        .values("local")
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(text(&local), "2023-08-02T09:30:00");
    let time = lock
        .traversal()
        .v(None)
        .has("name", "a")
        .values("time")
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(text(&time), "09:30:00.250");
}

#[test]
fn has_compares_a_datetime_by_its_instant() {
    let graph = events();
    let lock = graph.read();
    let names = |value: ElementProperty| -> Vec<String> {
        lock.traversal()
            .v(None)
            .has("at", value)
            .values("name")
            .to_list()
            .unwrap()
            .iter()
            .map(|n| n.as_string().unwrap().to_string())
            .collect()
    };
    // 12:00Z and 14:00+02:00 are one instant; so is 2023-08-01T23:00-07:00 (06:00Z next day)
    // not.
    assert_eq!(names(dt("2023-08-02T12:00:00Z").into()), ["a", "b"]);
    assert_eq!(names(dt("2023-08-02T08:00:00-04:00").into()), ["a", "b"]);
    assert_eq!(names(dt("2023-08-02T06:00:00Z").into()), ["c"]);
    // A string is never a datetime (no content sniffing).
    assert!(names("2023-08-02T12:00:00Z".into()).is_empty());
    // A date is not a datetime of its midnight.
    assert!(names("2020-01-01".parse::<Date>().unwrap().into()).is_empty());
    // Durations compare field by field: P1D is not PT24H, P1M not P30D.
    let spans = |value: &str| -> usize {
        lock.traversal()
            .v(None)
            .has("span", value.parse::<Duration>().unwrap())
            .count()
            .next()
            .unwrap()
            .unwrap()
            .as_i64()
            .unwrap() as usize
    };
    assert_eq!(spans("P1D"), 1);
    assert_eq!(spans("PT24H"), 1);
    assert_eq!(spans("P1M"), 1);
    assert_eq!(spans("P2D"), 0);
}

#[test]
fn predicates_compare_within_one_temporal_type() {
    let graph = events();
    let lock = graph.read();
    let count = |p: P<PValue>| -> i64 {
        lock.traversal()
            .v(None)
            .values("at")
            .is(p)
            .count()
            .next()
            .unwrap()
            .unwrap()
            .as_i64()
            .unwrap()
    };
    // The instants: 2020-01-01, 2023-08-02T06:00Z, and twice 2023-08-02T12:00Z.
    let noon = PValue::from(dt("2023-08-02T12:00:00Z"));
    assert_eq!(count(P::Gt(noon.clone())), 0);
    assert_eq!(count(P::Gte(noon.clone())), 2);
    assert_eq!(count(P::Lt(noon.clone())), 2);
    assert_eq!(count(P::Eq(noon.clone())), 2);
    assert_eq!(count(P::Neq(noon)), 2);
    let morning = PValue::from(dt("2023-08-02T08:00:00+02:00"));
    assert_eq!(count(P::Gt(morning.clone())), 2);
    assert_eq!(count(P::Lte(morning)), 2);
    // Another temporal type, a string or a number is incomparable: only `neq` holds.
    let date = PValue::from("2023-08-02".parse::<Date>().unwrap());
    assert_eq!(count(P::Lt(date.clone())), 0);
    assert_eq!(count(P::Gt(date.clone())), 0);
    assert_eq!(count(P::Neq(date)), 4);
    assert_eq!(count(P::Gt(PValue::from("2000"))), 0);
    assert_eq!(count(P::Gt(PValue::from(0i64))), 0);
}

#[test]
fn order_sorts_by_instant_and_types_sit_between_numbers_and_strings() {
    let graph = events();
    let lock = graph.read();
    let ordered = lock
        .traversal()
        .v(None)
        .values("at")
        .order()
        .to_list()
        .unwrap();
    let texts: Vec<String> = ordered.iter().map(text).collect();
    // 2023-08-01T23:00-07:00 is 06:00Z on the 2nd; 12:00Z and 14:00+02:00 are one instant
    // (their order between each other is unspecified).
    assert_eq!(texts[0], "2020-01-01T00:00:00Z", "{texts:?}");
    assert_eq!(texts[1], "2023-08-01T23:00:00-07:00", "{texts:?}");
    let mut tied = texts[2..].to_vec();
    tied.sort();
    assert_eq!(tied, ["2023-08-02T12:00:00Z", "2023-08-02T14:00:00+02:00"]);
    // Durations order by length (a month = 30 days), ties by their fields.
    let spans: Vec<String> = lock
        .traversal()
        .v(None)
        .values("span")
        .order()
        .to_list()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert_eq!(spans, ["PT24H", "P1D", "P30D", "P1M"]);
    // Mixed values: numbers, then temporal values (date, localdatetime, datetime, time,
    // duration), then strings.
    let mixed = lock
        .traversal()
        .inject(vec![
            ElementProperty::from("text"),
            ElementProperty::from("P1D".parse::<Duration>().unwrap()),
            ElementProperty::from("10:00".parse::<Time>().unwrap()),
            ElementProperty::from(dt("1999-01-01T00:00:00Z")),
            ElementProperty::from("2030-01-01T00:00".parse::<LocalDateTime>().unwrap()),
            ElementProperty::from("2040-01-01".parse::<Date>().unwrap()),
            ElementProperty::from(7i64),
        ])
        .order()
        .to_list()
        .unwrap();
    let kinds: Vec<String> = mixed
        .iter()
        .map(|value| match value.as_temporal() {
            Some(t) => t.kind().name().to_string(),
            None => format!("{value:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "7",
            "date",
            "localdatetime",
            "datetime",
            "time",
            "duration",
            "\"text\""
        ]
    );
}

#[test]
fn dedup_and_group_count_treat_one_instant_as_one_value() {
    let graph = events();
    let lock = graph.read();
    let distinct = lock
        .traversal()
        .v(None)
        .values("at")
        .dedup()
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(distinct.as_i64(), Some(3));
    let days = lock
        .traversal()
        .v(None)
        .values("day")
        .group_count()
        .next()
        .unwrap()
        .unwrap();
    // Temporal keys are not strings: the result is a value-keyed map.
    let ElementProperty::Map(counts) = &days else {
        panic!("expected a value-keyed map, got {days:?}");
    };
    let count_of = |day: &str| {
        let key = ElementProperty::from(day.parse::<Date>().unwrap());
        counts
            .iter()
            .find(|(k, _)| **k == key)
            .and_then(|(_, v)| v.as_i64())
    };
    assert_eq!(count_of("2023-08-02"), Some(2));
    assert_eq!(count_of("2023-08-01"), Some(1));
    assert_eq!(count_of("2023-08-03"), Some(1));
    let spans = lock
        .traversal()
        .v(None)
        .values("span")
        .dedup()
        .count()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(spans.as_i64(), Some(4), "P1D/PT24H and P1M/P30D stay apart");
}

#[test]
fn min_and_max_of_temporal_values() {
    let graph = events();
    let lock = graph.read();
    let earliest = lock
        .traversal()
        .v(None)
        .values("at")
        .min()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(text(&earliest), "2020-01-01T00:00:00Z");
    let latest = lock
        .traversal()
        .v(None)
        .values("day")
        .max()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(text(&latest), "2023-08-03");
    // Two temporal types do not compare.
    let err = lock
        .traversal()
        .inject(vec![
            ElementProperty::from(dt("2023-08-02T00:00:00Z")),
            ElementProperty::from("2023-08-02".parse::<Date>().unwrap()),
        ])
        .max()
        .next()
        .unwrap_err();
    let err = format!("{err:?}");
    assert!(err.contains("IncomparableValues"), "{err}");
}

#[test]
fn type_of_matches_the_temporal_tokens() {
    let graph = events();
    let lock = graph.read();
    let count = |key: &str, ty: GType| -> i64 {
        lock.traversal()
            .v(None)
            .values(key)
            .is(P::type_of(ty))
            .count()
            .next()
            .unwrap()
            .unwrap()
            .as_i64()
            .unwrap()
    };
    assert_eq!(count("at", GType::DATETIME), 4);
    assert_eq!(count("at", GType::DATE), 0);
    assert_eq!(count("at", GType::STRING), 0);
    assert_eq!(count("day", GType::DATE), 4);
    assert_eq!(count("span", GType::DURATION), 4);
    assert_eq!(count("local", GType::LOCALDATETIME), 4);
    assert_eq!(count("time", GType::TIME), 4);
    assert_eq!(count("name", GType::DATETIME), 0);
    assert_eq!(GType::DATETIME.to_string(), "datetime");
    assert_eq!(ValueType::LocalDateTime.to_string(), "localdatetime");
}

#[test]
fn casts_of_temporal_values() {
    let at = ElementProperty::from(dt("2023-09-06T16:28:27Z"));
    // as_string: the canonical text; as_number: TinkerPop's epoch milliseconds.
    assert_eq!(
        at.cast(&ValueType::String).unwrap(),
        ElementProperty::from("2023-09-06T16:28:27Z")
    );
    assert_eq!(
        at.cast(&ValueType::Int64).unwrap().as_i64(),
        Some(1_694_017_707_000)
    );
    let day = ElementProperty::from("2023-08-02".parse::<Date>().unwrap());
    let err = day.cast(&ValueType::Int64).unwrap_err().to_string();
    assert!(err.contains("date"), "{err}");
    assert!(day.cast(&ValueType::Boolean).is_err());
    // An explicit conversion parses a string; a number is epoch milliseconds.
    assert_eq!(
        ElementProperty::from("2023-08-02T00:00:00-07:00")
            .cast(&ValueType::DateTime)
            .unwrap(),
        ElementProperty::from(dt("2023-08-02T07:00:00Z"))
    );
    assert_eq!(
        ElementProperty::from(1_694_017_707_000i64)
            .cast(&ValueType::DateTime)
            .unwrap(),
        at
    );
    assert!(
        ElementProperty::from("not a date")
            .cast(&ValueType::Date)
            .is_err()
    );
    assert!(at.cast(&ValueType::Date).is_err());
    assert_eq!(day.cast(&ValueType::Date).unwrap(), day);
}

#[test]
fn rendering_names_the_type() {
    let graph = events();
    let lock = graph.read();
    // A step with a temporal literal renders it as its DSL constructor.
    let metrics = lock
        .traversal()
        .v(None)
        .has("at", dt("2023-08-02T12:00:00Z"))
        .count()
        .profile()
        .unwrap();
    let rendered = metrics.to_string();
    assert!(
        rendered.contains(r#"datetime("2023-08-02T12:00:00Z")"#),
        "{rendered}"
    );
    // The table shows the canonical text.
    let table = lock
        .traversal()
        .v(None)
        .has("name", "b")
        .values("at")
        .to_table()
        .unwrap();
    assert!(table.contains("2023-08-02T14:00:00+02:00"), "{table}");
}

#[test]
fn a_schema_inferred_from_temporal_data_still_admits_it() {
    let graph = events();
    let mut lock = graph.write();
    let inferred = lock.traversal().infer_schema().unwrap();
    lock.traversal_mut()
        .set_schema(inferred.with_mode(SchemaMode::Closed))
        .unwrap();
    // A2: temporal formats; until then the position is unconstrained.
    lock.traversal_mut()
        .add_v("event")
        .property("name", "e")
        .property("at", dt("2024-01-01T00:00:00Z"))
        .property("day", "2024-01-01".parse::<Date>().unwrap())
        .property("span", "P1D".parse::<Duration>().unwrap())
        .property(
            "local",
            "2024-01-01T00:00".parse::<LocalDateTime>().unwrap(),
        )
        .property("time", "00:00".parse::<Time>().unwrap())
        .next()
        .unwrap();
}

#[cfg(feature = "io")]
#[test]
fn graphson_export_refuses_a_temporal_value_cleanly() {
    use graphersal::io::GraphExport;
    let graph = events();
    let mut out = Vec::new();
    let err = graph.export_graphson_writer(&mut out).unwrap_err();
    assert!(err.to_string().contains("cannot be written"), "{err}");
}

#[cfg(feature = "persist")]
#[test]
fn the_binary_format_refuses_a_temporal_value_cleanly() {
    use graphersal::persist::{self, WriteOptions};
    let graph = events();
    let lock = graph.read();
    let mut bytes = Vec::new();
    let err = persist::write_snapshot(&*lock, &mut bytes, &WriteOptions::new()).unwrap_err();
    assert!(err.to_string().contains("binary format"), "{err}");
}
