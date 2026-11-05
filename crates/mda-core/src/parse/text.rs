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

/// JS `\s` as a `regex` character class (JS ≠ Rust: JS includes U+FEFF and excludes U+0085). Port
/// a TS `\s` as `JS_S`, never as Rust's `\s`; the same set `js_trim` strips.
///
/// # Examples
///
/// ```
/// use mda_core::parse::text::{JS_S, compile, js_trim};
/// let re = compile(&format!("^{JS_S}$"));
/// for c in (0u32..=0xFFFF).filter_map(char::from_u32) {
///     let s = c.to_string();
///     assert_eq!(re.is_match(&s), js_trim(&s).is_empty(), "U+{:04X}", c as u32);
/// }
/// ```
pub const JS_S: &str = r"[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";

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
