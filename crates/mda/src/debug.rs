//! The one logger: `--debug[=trace]`, `MDA_LOG`, `--log-file`. Never stdout. Stub landed by
//! H0.0b; T0.6 implements it.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

/// How much to log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLevel {
    Off,
    Debug,
    Trace,
}

/// Why the logger could not be installed.
#[derive(Debug)]
pub enum DebugError {
    Io(std::io::ErrorKind),
}

/// Resolve the level: the flag wins over `MDA_LOG`; anything unrecognised is `Off`.
///
/// # Examples
/// ```
/// use mda::debug::{level_from, DebugLevel};
/// assert_eq!(level_from(Some("trace"), Some("debug")), DebugLevel::Trace);
/// assert_eq!(level_from(None, Some("nope")), DebugLevel::Off);
/// ```
pub fn level_from(flag: Option<&str>, env: Option<&str>) -> DebugLevel {
    match flag.or(env) {
        Some("debug") => DebugLevel::Debug,
        Some("trace") => DebugLevel::Trace,
        _ => DebugLevel::Off,
    }
}

/// Where lines go. Never stdout: it carries protocol bytes in `serve`.
enum Sink {
    Stderr,
    File(File),
}

struct Logger(Mutex<Sink>);

impl Logger {
    fn write_line(&self, line: &str) {
        let mut sink = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // A failing log write must not take the process down: drop the line.
        let _ = match &mut *sink {
            Sink::Stderr => writeln!(std::io::stderr(), "{line}"),
            Sink::File(f) => writeln!(f, "{line}"),
        };
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true // the max level filter does the work
    }

    fn log(&self, r: &log::Record) {
        // Messages and targets can carry untrusted text (vault file names, client
        // frames): escape control chars so they cannot forge lines or inject escapes.
        self.write_line(&format!(
            "{} {} {} {}",
            now_stamp(),
            r.level(),
            escape(r.target()),
            escape(&r.args().to_string())
        ));
    }

    fn flush(&self) {}
}

fn open_log(p: &Path) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.create(true).append(true);
    // Trace logs can hold secrets. ponytail: mode applies on creation only; an existing
    // file keeps its mode — chmod it too if that ever matters.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(p)
}

/// Install the global logger, writing to stderr or `log_file`, never stdout.
///
/// `Off` installs nothing and never opens `log_file`. At `Trace` the first line
/// warns that output may include secrets.
///
/// # Examples
/// ```
/// use mda::debug::{init, DebugLevel};
/// assert!(init(DebugLevel::Off, None).is_ok());
/// ```
pub fn init(level: DebugLevel, log_file: Option<&Path>) -> Result<(), DebugError> {
    let filter = match level {
        DebugLevel::Off => return Ok(()),
        DebugLevel::Debug => log::LevelFilter::Debug,
        DebugLevel::Trace => log::LevelFilter::Trace,
    };
    let sink = match log_file {
        Some(p) => Sink::File(open_log(p).map_err(|e| DebugError::Io(e.kind()))?),
        None => Sink::Stderr,
    };
    let logger = Logger(Mutex::new(sink));
    if level == DebugLevel::Trace {
        logger.write_line(&format!(
            "{} WARN mda::debug trace output may include secrets (variable values, file contents)",
            now_stamp()
        ));
    }
    // Already installed (second init in one process): keep the first logger.
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(filter);
    }
    Ok(())
}

fn escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339_utc(secs)
}

/// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
///
/// # Examples
/// ```
/// assert_eq!(mda::debug::rfc3339_utc(0), "1970-01-01T00:00:00Z");
/// ```
pub fn rfc3339_utc(unix_secs: u64) -> String {
    let (days, rem) = (unix_secs / 86_400, unix_secs % 86_400);
    // Howard Hinnant's civil-from-days, shifted to a March-based year.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    let (h, mi, s) = (rem / 3_600, rem % 3_600 / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_wins_over_env() {
        assert_eq!(level_from(Some("trace"), Some("debug")), DebugLevel::Trace);
        assert_eq!(level_from(None, Some("debug")), DebugLevel::Debug);
    }

    #[test]
    fn unknown_is_off() {
        assert_eq!(level_from(None, Some("bogus")), DebugLevel::Off);
        assert_eq!(level_from(None, None), DebugLevel::Off);
    }

    #[test]
    fn rfc3339_known_dates() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(86_399), "1970-01-01T23:59:59Z");
    }
}
