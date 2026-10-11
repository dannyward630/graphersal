//! The stderr log of `--server`, `graphersal store` and every mode on a `--graph <store>`
//! (`-e`, `--in`, a script file, the REPL): a minimal `tracing` subscriber that
//! prints the library's events (store warnings such as a failed automatic checkpoint, a
//! read-only open, damage found) as one line each on stderr, never on stdout.
//!
//! The library only logs through `tracing` (CLAUDE.md rule 9); a binary decides where the
//! lines go. A hand-written subscriber instead of `tracing-subscriber`: the CLI needs one
//! level filter and one line format, not a new dependency tree.

use std::fmt::Write as _;

use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

/// The environment variable that sets the level: `off`, `error`, `warn` (the default),
/// `info`, `debug` or `trace`, case-insensitive.
pub(crate) const LOG_ENV: &str = "GRAPHERSAL_LOG";

/// Installs the stderr subscriber as the process-wide default, at the level of
/// [`LOG_ENV`] (default `warn`). An unknown value prints a note and keeps `warn`.
pub(crate) fn install() {
    let level = match std::env::var(LOG_ENV) {
        Ok(value) => parse_level(&value).unwrap_or_else(|| {
            errln!(
                "note: ignoring {LOG_ENV}={value:?}: expected off, error, warn, info, debug or \
                 trace; logging at warn"
            );
            LevelFilter::WARN
        }),
        Err(_) => LevelFilter::WARN,
    };
    // Fails only if a subscriber is installed already: then that one stays.
    let _ = tracing::subscriber::set_global_default(StderrLog { level });
}

/// Parses a level name of [`LOG_ENV`].
fn parse_level(value: &str) -> Option<LevelFilter> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "off" => LevelFilter::OFF,
        "error" => LevelFilter::ERROR,
        "warn" | "warning" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        _ => return None,
    })
}

/// Prints every enabled event as `LEVEL target: message key=value ...` on stderr; spans are
/// not recorded (the library logs events only).
struct StderrLog {
    level: LevelFilter,
}

impl Subscriber for StderrLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.is_event() && self.level >= *metadata.level()
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(self.level)
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        // Never called for a disabled span; any non-zero id would do.
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        let mut line = format!("{} {}:", metadata.level(), metadata.target());
        event.record(&mut LineVisitor { line: &mut line });
        line.push('\n');
        // A closed stderr loses the line; logging never fails the program.
        crate::out::write_stderr(format_args!("{line}"));
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

/// Appends an event's fields to the line: the message first as text, the others as
/// ` key=value`.
struct LineVisitor<'a> {
    line: &'a mut String,
}

impl Visit for LineVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            let _ = write!(self.line, " {value}");
        } else {
            let _ = write!(self.line, " {}={value}", field.name());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        // The message arrives as `format_args!`, whose Debug is its plain text.
        if field.name() == "message" {
            let _ = write!(self.line, " {value:?}");
        } else {
            let _ = write!(self.line, " {}={value:?}", field.name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_parse_case_insensitively() {
        assert_eq!(parse_level("WARN"), Some(LevelFilter::WARN));
        assert_eq!(parse_level(" info "), Some(LevelFilter::INFO));
        assert_eq!(parse_level("off"), Some(LevelFilter::OFF));
        assert_eq!(parse_level("loud"), None);
    }

    #[test]
    fn the_filter_keeps_events_at_or_above_the_level() {
        let log = StderrLog {
            level: LevelFilter::WARN,
        };
        assert_eq!(log.max_level_hint(), Some(LevelFilter::WARN));
        assert!(log.level >= tracing::Level::ERROR);
        assert!(log.level >= tracing::Level::WARN);
        assert!(log.level < tracing::Level::INFO);
        assert!(LevelFilter::OFF < tracing::Level::ERROR);
    }
}
