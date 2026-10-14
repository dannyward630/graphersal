//! Parsing temporal text (ISO 8601 / RFC 3339 / RFC 9557), stricter than ISO 8601 and never
//! guessing (no locale formats, no two-digit years, no content sniffing: these parsers run only
//! where a temporal value is asked for).
//!
//! - date: `YYYY-MM-DD`, a year before 1 or after 9999 with its sign (`-0044-03-15`);
//! - time: `HH:MM`, `HH:MM:SS`, `HH:MM:SS.f` (1-9 fraction digits; beyond the microsecond they
//!   are cut off);
//! - local date-time: a date, optionally `T` (or `t`, or a space) and a time; a date alone is
//!   midnight;
//! - date-time: a local date-time, then an offset (`Z`, `+02:00`, `+0200`, `+02`, `+00:19:32`)
//!   and/or a time zone in brackets (`[Europe/Berlin]`, RFC 9557). Without either it is UTC (as
//!   TinkerPop's `asDate("2023-08-02")`). With only a zone the wall-clock time is resolved in
//!   it (a skipped time moves forward, a repeated one takes the earlier offset); with both the
//!   offset must be the zone's at that instant;
//! - duration: `[+-]P[nY][nM][nW][nD][T[nH][nM][n[.f]S]]`, each number with an optional sign
//!   of its own (`P1M-2D`), letters in either case.

use super::civil::{self, MICROS_PER_SECOND};
use super::zone::Zone;
use super::{
    Date, DateTime, Duration, LocalDateTime, MAX_OFFSET_SECONDS, TemporalError, TemporalResult,
    Time,
};

struct Cursor<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    expected: &'static str,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str, expected: &'static str) -> Self {
        Cursor {
            text,
            bytes: text.as_bytes(),
            pos: 0,
            expected,
        }
    }

    fn fail<T>(&self, reason: &'static str) -> TemporalResult<T> {
        Err(TemporalError::parse(self.text, self.expected, reason))
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn at_end(&self) -> bool {
        self.pos == self.bytes.len()
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, byte: u8, reason: &'static str) -> TemporalResult<()> {
        if self.eat(byte) {
            Ok(())
        } else {
            self.fail(reason)
        }
    }

    /// Exactly `count` ASCII digits.
    fn digits(&mut self, count: usize, reason: &'static str) -> TemporalResult<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            match self.peek() {
                Some(b @ b'0'..=b'9') => {
                    value = value * 10 + (b - b'0') as u32;
                    self.pos += 1;
                }
                _ => return self.fail(reason),
            }
        }
        Ok(value)
    }

    /// A run of digits (at least one), as `i64`; `None` when there is none.
    fn number(&mut self) -> TemporalResult<Option<i64>> {
        let start = self.pos;
        let mut value: i64 = 0;
        while let Some(b @ b'0'..=b'9') = self.peek() {
            value = match value
                .checked_mul(10)
                .and_then(|v| v.checked_add((b - b'0') as i64))
            {
                Some(v) => v,
                None => return self.fail("a number is too large"),
            };
            self.pos += 1;
        }
        Ok((self.pos > start).then_some(value))
    }

    fn finish(&self) -> TemporalResult<()> {
        if self.at_end() {
            Ok(())
        } else {
            self.fail("unexpected text after the value")
        }
    }
}

fn read_date(c: &mut Cursor) -> TemporalResult<i32> {
    let sign = if c.eat(b'-') {
        Some(-1)
    } else if c.eat(b'+') {
        Some(1)
    } else {
        None
    };
    let start = c.pos;
    let mut year: i64 = 0;
    while let Some(b @ b'0'..=b'9') = c.peek() {
        year = year * 10 + (b - b'0') as i64;
        c.pos += 1;
        if c.pos - start > 6 {
            return c.fail("the year has too many digits");
        }
    }
    let width = c.pos - start;
    if width < 4 || (sign.is_none() && width != 4) {
        return c.fail("expected a four-digit year (YYYY-MM-DD)");
    }
    let year = year * sign.unwrap_or(1);
    c.expect(b'-', "expected '-' after the year (YYYY-MM-DD)")?;
    let month = c.digits(2, "expected a two-digit month (YYYY-MM-DD)")?;
    c.expect(b'-', "expected '-' after the month (YYYY-MM-DD)")?;
    let day = c.digits(2, "expected a two-digit day (YYYY-MM-DD)")?;
    if year < civil::MIN_YEAR as i64 || year > civil::MAX_YEAR as i64 {
        return c.fail("the year is outside -9999 to 9999");
    }
    let year = year as i32;
    if !(1..=12).contains(&month) {
        return c.fail("the month is not 01-12");
    }
    if day == 0 || day > civil::days_in_month(year, month as u8) as u32 {
        return c.fail("the day does not exist in that month");
    }
    Ok(civil::days_from_civil(year, month as u8, day as u8))
}

/// Up to nine fraction digits after a `.` or `,`, as microseconds (cut, not rounded).
fn read_fraction(c: &mut Cursor) -> TemporalResult<i64> {
    if !(c.eat(b'.') || c.eat(b',')) {
        return Ok(0);
    }
    let mut micros = 0i64;
    let mut count = 0;
    while let Some(b @ b'0'..=b'9') = c.peek() {
        if count < 6 {
            micros = micros * 10 + (b - b'0') as i64;
        }
        count += 1;
        c.pos += 1;
        if count > 9 {
            return c.fail("more than nine fraction digits");
        }
    }
    if count == 0 {
        return c.fail("expected digits after the decimal point");
    }
    for _ in count..6 {
        micros *= 10;
    }
    Ok(micros)
}

fn read_time(c: &mut Cursor) -> TemporalResult<i64> {
    let hour = c.digits(2, "expected a two-digit hour (HH:MM:SS)")?;
    c.expect(b':', "expected ':' after the hour (HH:MM:SS)")?;
    let minute = c.digits(2, "expected two-digit minutes (HH:MM:SS)")?;
    let (second, fraction) = if c.eat(b':') {
        let second = c.digits(2, "expected two-digit seconds (HH:MM:SS)")?;
        (second, read_fraction(c)?)
    } else {
        (0, 0)
    };
    if hour > 23 {
        return c.fail("the hour is not 00-23");
    }
    if minute > 59 {
        return c.fail("the minutes are not 00-59");
    }
    if second > 59 {
        return c.fail("the seconds are not 00-59");
    }
    Ok(((hour as i64 * 60 + minute as i64) * 60 + second as i64) * MICROS_PER_SECOND + fraction)
}

/// A date, optionally followed by a separator and a time; local microseconds.
fn read_local(c: &mut Cursor) -> TemporalResult<i64> {
    let days = read_date(c)?;
    let mut micros = days as i64 * civil::MICROS_PER_DAY;
    if matches!(c.peek(), Some(b'T' | b't' | b' ')) {
        c.pos += 1;
        micros += read_time(c)?;
    }
    Ok(micros)
}

/// `Z`, `+HH:MM[:SS]`, `+HHMM`, `+HH`; `None` when no offset follows.
fn read_offset(c: &mut Cursor) -> TemporalResult<Option<i32>> {
    if c.eat(b'Z') || c.eat(b'z') {
        return Ok(Some(0));
    }
    let sign = if c.eat(b'+') {
        1
    } else if c.eat(b'-') {
        -1
    } else {
        return Ok(None);
    };
    let hours = c.digits(2, "expected a two-digit offset hour (+HH:MM)")?;
    let mut minutes = 0;
    let mut seconds = 0;
    if c.eat(b':') {
        minutes = c.digits(2, "expected two-digit offset minutes (+HH:MM)")?;
        if c.eat(b':') {
            seconds = c.digits(2, "expected two-digit offset seconds (+HH:MM:SS)")?;
        }
    } else if matches!(c.peek(), Some(b'0'..=b'9')) {
        minutes = c.digits(2, "expected two-digit offset minutes (+HHMM)")?;
    }
    if minutes > 59 || seconds > 59 {
        return c.fail("the offset minutes or seconds are not 00-59");
    }
    let total = sign * (hours * 3600 + minutes * 60 + seconds) as i32;
    if total.abs() > MAX_OFFSET_SECONDS {
        return c.fail("the offset is beyond ±18:00");
    }
    Ok(Some(total))
}

/// `[Zone/Name]` (RFC 9557; a leading `!` critical flag is accepted); `None` when absent.
fn read_zone<'a>(c: &mut Cursor<'a>) -> TemporalResult<Option<&'a str>> {
    if !c.eat(b'[') {
        return Ok(None);
    }
    c.eat(b'!');
    let start = c.pos;
    while let Some(b) = c.peek() {
        if b == b']' {
            break;
        }
        c.pos += 1;
    }
    let name = &c.text[start..c.pos];
    c.expect(b']', "expected ']' after the time zone name")?;
    if name.is_empty() {
        return c.fail("the time zone name in brackets is empty");
    }
    Ok(Some(name))
}

pub(super) fn date(text: &str) -> TemporalResult<Date> {
    let mut c = Cursor::new(text, "date");
    let days = read_date(&mut c)?;
    c.finish()?;
    Ok(Date { days })
}

pub(super) fn time(text: &str) -> TemporalResult<Time> {
    let mut c = Cursor::new(text, "time");
    let micros = read_time(&mut c)?;
    if !c.at_end() && matches!(c.peek(), Some(b'Z' | b'z' | b'+' | b'-')) {
        return c.fail("a time has no UTC offset");
    }
    c.finish()?;
    Ok(Time { micros })
}

pub(super) fn local_date_time(text: &str) -> TemporalResult<LocalDateTime> {
    let mut c = Cursor::new(text, "localdatetime");
    let micros = read_local(&mut c)?;
    if !c.at_end() && matches!(c.peek(), Some(b'Z' | b'z' | b'+' | b'-' | b'[')) {
        return c.fail("a local date-time has no UTC offset or time zone (use a datetime)");
    }
    c.finish()?;
    Ok(LocalDateTime { micros })
}

pub(super) fn date_time(text: &str) -> TemporalResult<DateTime> {
    let mut c = Cursor::new(text, "datetime");
    let local = read_local(&mut c)?;
    let offset = read_offset(&mut c)?;
    let zone = read_zone(&mut c)?;
    c.finish()?;
    let local = LocalDateTime { micros: local };
    match (offset, zone) {
        (Some(offset), None) => DateTime::from_local(local, offset),
        (None, None) => DateTime::from_local(local, 0),
        (None, Some(name)) => Zone::parse(name)?.resolve(local),
        (Some(offset), Some(name)) => {
            let value = DateTime::from_local(local, offset)?;
            let zone = Zone::parse(name)?;
            if zone.offset_at(value)? != offset {
                return c.fail("the offset is not the time zone's offset at that time");
            }
            Ok(value)
        }
    }
}

pub(super) fn duration(text: &str) -> TemporalResult<Duration> {
    let mut c = Cursor::new(text, "duration");
    let negative = if c.eat(b'-') {
        true
    } else {
        c.eat(b'+');
        false
    };
    if !(c.eat(b'P') || c.eat(b'p')) {
        return c.fail("expected 'P' (ISO 8601 duration such as P1DT2H)");
    }
    let mut months: i64 = 0;
    let mut days: i64 = 0;
    let mut micros: i64 = 0;
    let mut any = false;
    let mut in_time = false;
    // The units in their required order; the index only grows.
    const DATE_UNITS: &[u8] = b"YMWD";
    const TIME_UNITS: &[u8] = b"HMS";
    let mut next_unit = 0usize;
    let overflow = |c: &Cursor| c.fail::<()>("the duration is too large");
    loop {
        if c.at_end() {
            break;
        }
        if !in_time && (c.eat(b'T') || c.eat(b't')) {
            in_time = true;
            next_unit = 0;
            if c.at_end() {
                return c.fail("expected a time part after 'T'");
            }
            continue;
        }
        let sign = if c.eat(b'-') {
            -1
        } else {
            c.eat(b'+');
            1
        };
        let Some(number) = c.number()? else {
            return c.fail("expected a number before a duration unit");
        };
        let fraction = if in_time { read_fraction(&mut c)? } else { 0 };
        let Some(unit) = c.peek().map(|b| b.to_ascii_uppercase()) else {
            return c.fail("expected a unit (Y, M, W, D, H, M or S) after the number");
        };
        c.pos += 1;
        let units = if in_time { TIME_UNITS } else { DATE_UNITS };
        let Some(index) = units[next_unit..].iter().position(|&u| u == unit) else {
            return c.fail("unknown or out-of-order duration unit");
        };
        next_unit += index + 1;
        if fraction != 0 && unit != b'S' {
            return c.fail("only seconds may have a fraction");
        }
        let n = number * sign;
        let added = match (in_time, unit) {
            (false, b'Y') => n.checked_mul(12).and_then(|v| months.checked_add(v)).map(|v| months = v),
            (false, b'M') => months.checked_add(n).map(|v| months = v),
            (false, b'W') => n.checked_mul(7).and_then(|v| days.checked_add(v)).map(|v| days = v),
            (false, _) => days.checked_add(n).map(|v| days = v),
            (true, b'H') => n
                .checked_mul(3600 * MICROS_PER_SECOND)
                .and_then(|v| micros.checked_add(v))
                .map(|v| micros = v),
            (true, b'M') => n
                .checked_mul(60 * MICROS_PER_SECOND)
                .and_then(|v| micros.checked_add(v))
                .map(|v| micros = v),
            (true, _) => n
                .checked_mul(MICROS_PER_SECOND)
                .and_then(|v| v.checked_add(fraction * sign))
                .and_then(|v| micros.checked_add(v))
                .map(|v| micros = v),
        };
        if added.is_none() {
            overflow(&c)?;
        }
        any = true;
    }
    if !any {
        return c.fail("a duration needs at least one part (PT0S for zero)");
    }
    let (Ok(months), Ok(days)) = (i32::try_from(months), i32::try_from(days)) else {
        return c.fail("the duration is too large");
    };
    let value = Duration::new(months, days, micros);
    if negative {
        value.checked_neg()
    } else {
        Ok(value)
    }
}

/// A UTC offset written alone (`Z`, `+02:00`, `-0530`, `+02`), for a zone argument.
pub(super) fn offset(text: &str) -> TemporalResult<i32> {
    let mut c = Cursor::new(text, "UTC offset");
    let Some(offset) = read_offset(&mut c)? else {
        return c.fail("expected Z or a sign (+HH:MM)");
    };
    c.finish()?;
    Ok(offset)
}
