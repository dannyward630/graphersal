//! The proleptic Gregorian calendar: days since 1970-01-01 <-> (year, month, day), weekdays and
//! ISO 8601 weeks.
//!
//! Own code rather than a dependency: these are a few exact integer formulas (H. Hinnant,
//! "chrono-Compatible Low-Level Date Algorithms"), and keeping them here makes every calendar
//! answer independent of the time zone library, which is used for IANA zones only.

/// The first supported year (ISO 8601 four-digit years, the same range as the zone library).
pub(crate) const MIN_YEAR: i32 = -9999;
/// The last supported year.
pub(crate) const MAX_YEAR: i32 = 9999;
/// Microseconds in one day (a day is always 24 hours outside a time zone).
pub(crate) const MICROS_PER_DAY: i64 = 86_400_000_000;
/// Microseconds in one second.
pub(crate) const MICROS_PER_SECOND: i64 = 1_000_000;

/// Days since 1970-01-01 of `-9999-01-01`.
pub(crate) const MIN_DAY: i32 = days_from_civil(MIN_YEAR, 1, 1);
/// Days since 1970-01-01 of `9999-12-31`.
pub(crate) const MAX_DAY: i32 = days_from_civil(MAX_YEAR, 12, 31);
/// The first microsecond of the supported range (local or UTC).
pub(crate) const MIN_MICROS: i64 = MIN_DAY as i64 * MICROS_PER_DAY;
/// The last microsecond of the supported range (local or UTC).
pub(crate) const MAX_MICROS: i64 = (MAX_DAY as i64 + 1) * MICROS_PER_DAY - 1;

/// Whether `year` is a leap year.
pub(crate) const fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// The number of days of `month` (1-12) in `year`.
pub(crate) const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a valid civil date (no range check).
pub(crate) const fn days_from_civil(year: i32, month: u8, day: u8) -> i32 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month as i32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day number (no range check).
pub(crate) const fn civil_from_days(days: i32) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// Whether a day number lies in the supported range.
pub(crate) const fn day_in_range(days: i64) -> bool {
    days >= MIN_DAY as i64 && days <= MAX_DAY as i64
}

/// Whether a microsecond count (local or UTC) lies in the supported range.
pub(crate) const fn micros_in_range(micros: i64) -> bool {
    micros >= MIN_MICROS && micros <= MAX_MICROS
}

/// The day of the week, 0 = Sunday .. 6 = Saturday (PostgreSQL `dow`).
pub(crate) const fn day_of_week(days: i32) -> u8 {
    // 1970-01-01 was a Thursday.
    (days as i64 + 4).rem_euclid(7) as u8
}

/// The ISO day of the week, 1 = Monday .. 7 = Sunday (PostgreSQL `isodow`).
pub(crate) const fn iso_day_of_week(days: i32) -> u8 {
    match day_of_week(days) {
        0 => 7,
        d => d,
    }
}

/// The day of the year, 1-366.
pub(crate) const fn day_of_year(days: i32) -> u16 {
    let (year, _, _) = civil_from_days(days);
    (days - days_from_civil(year, 1, 1) + 1) as u16
}

/// The ISO 8601 week-numbering year and week (1-53) of a day.
pub(crate) const fn iso_week(days: i32) -> (i32, u8) {
    // The week belongs to the year of its Thursday.
    let thursday = days - iso_day_of_week(days) as i32 + 4;
    let (year, _, _) = civil_from_days(thursday);
    let week = (thursday - days_from_civil(year, 1, 1)) / 7 + 1;
    (year, week as u8)
}

/// Adds `months` to a civil date, clamping the day to the end of the target month
/// (Jan 31 + 1 month = Feb 28/29, as PostgreSQL and `java.time` do). `None` outside the range.
pub(crate) fn add_months(days: i32, months: i64) -> Option<i32> {
    let (year, month, day) = civil_from_days(days);
    let index = (year as i64)
        .checked_mul(12)?
        .checked_add(month as i64 - 1)?
        .checked_add(months)?;
    let new_year = index.div_euclid(12);
    if new_year < MIN_YEAR as i64 || new_year > MAX_YEAR as i64 {
        return None;
    }
    let new_year = new_year as i32;
    let new_month = (index.rem_euclid(12) + 1) as u8;
    let new_day = day.min(days_in_month(new_year, new_month));
    Some(days_from_civil(new_year, new_month, new_day))
}

/// Splits a microsecond count into (days, microsecond of the day), flooring.
pub(crate) const fn split_micros(micros: i64) -> (i64, i64) {
    (
        micros.div_euclid(MICROS_PER_DAY),
        micros.rem_euclid(MICROS_PER_DAY),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(civil_from_days(19_571), (2023, 8, 2));
        assert_eq!(civil_from_days(MIN_DAY), (-9999, 1, 1));
        assert_eq!(civil_from_days(MAX_DAY), (9999, 12, 31));
    }

    #[test]
    fn round_trip_every_day_of_the_range_edges_and_a_stride() {
        let check = |d: i32| {
            let (y, m, dd) = civil_from_days(d);
            assert!((1..=12).contains(&m));
            assert!(dd >= 1 && dd <= days_in_month(y, m));
            assert_eq!(days_from_civil(y, m, dd), d);
        };
        for d in MIN_DAY..MIN_DAY + 1000 {
            check(d);
        }
        for d in -200_000..200_000 {
            check(d);
        }
        for d in MAX_DAY - 1000..=MAX_DAY {
            check(d);
        }
        let mut d = MIN_DAY;
        while d <= MAX_DAY {
            check(d);
            d += 37;
        }
    }

    #[test]
    fn weekdays_and_iso_weeks() {
        // 2023-08-02 was a Wednesday.
        let d = days_from_civil(2023, 8, 2);
        assert_eq!(day_of_week(d), 3);
        assert_eq!(iso_day_of_week(d), 3);
        assert_eq!(day_of_week(days_from_civil(2023, 8, 6)), 0);
        assert_eq!(iso_day_of_week(days_from_civil(2023, 8, 6)), 7);
        // 2021-01-03 belongs to week 53 of 2020; 2024-12-30 to week 1 of 2025.
        assert_eq!(iso_week(days_from_civil(2021, 1, 3)), (2020, 53));
        assert_eq!(iso_week(days_from_civil(2021, 1, 4)), (2021, 1));
        assert_eq!(iso_week(days_from_civil(2024, 12, 30)), (2025, 1));
        assert_eq!(day_of_year(days_from_civil(2024, 12, 31)), 366);
        // Negative years.
        assert_eq!(day_of_week(days_from_civil(-1, 12, 31)), 5);
    }

    #[test]
    fn month_arithmetic_clamps() {
        let jan31 = days_from_civil(2024, 1, 31);
        assert_eq!(civil_from_days(add_months(jan31, 1).unwrap()), (2024, 2, 29));
        assert_eq!(civil_from_days(add_months(jan31, 13).unwrap()), (2025, 2, 28));
        assert_eq!(civil_from_days(add_months(jan31, -2).unwrap()), (2023, 11, 30));
        assert_eq!(add_months(MAX_DAY, 1), None);
        assert_eq!(add_months(MIN_DAY, -1), None);
        assert_eq!(add_months(0, i64::MAX), None);
    }
}
