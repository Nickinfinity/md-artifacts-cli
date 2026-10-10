//! JS string semantics in one place, so every parser module ports TS `trim`, `split(/\r?\n/)` and
//! file decoding identically. Orchestrator-owned: read-only to every task.

use regex::Regex;

/// Decode file bytes: lossy UTF-8, then strip **one** leading BOM (Q-B; TS keeps it).
///
/// # Examples
///
/// ```
/// use mda_core::parse::decode;
/// assert_eq!(decode(b"\xEF\xBB\xBFhi"), "hi");
/// assert_eq!(decode(b"a\xFFb"), "a\u{FFFD}b");
/// ```
pub fn decode(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    match s.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => s.into_owned(),
    }
}

/// JS `String.prototype.trim` whitespace: strips U+FEFF, keeps U+0085 (Rust's `trim` does the reverse).
fn js_ws(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

/// JS `s.trim()`.
///
/// # Examples
///
/// ```
/// use mda_core::parse::text::js_trim;
/// assert_eq!(js_trim("\u{feff} a \n"), "a");
/// assert_eq!(js_trim("\u{85}a"), "\u{85}a");
/// ```
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(js_ws)
}

/// JS `s.trimEnd()`.
///
/// # Examples
///
/// ```
/// use mda_core::parse::text::js_trim_end;
/// assert_eq!(js_trim_end(" a \u{feff}"), " a");
/// ```
pub fn js_trim_end(s: &str) -> &str {
    s.trim_end_matches(js_ws)
}

/// JS `s.split(/\r?\n/)`: keeps a trailing `""` (unlike `str::lines`), strips one `\r` per line.
///
/// # Examples
///
/// ```
/// use mda_core::parse::text::js_lines;
/// assert_eq!(js_lines("a\r\nb\n").collect::<Vec<_>>(), ["a", "b", ""]);
/// ```
pub fn js_lines(s: &str) -> impl Iterator<Item = &str> {
    s.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}

/// Compile a **constant** pattern. The only `expect` in the parser: every module's unit test
/// forces its `LazyLock` statics, so a bad pattern fails the gate, never a user.
///
/// # Examples
///
/// ```
/// let re = mda_core::parse::text::compile("a+");
/// assert!(re.is_match("aa"));
/// ```
#[allow(clippy::expect_used)] // reason: constant patterns only; each module's test forces its LazyLock
pub fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("constant regex compiles")
}
