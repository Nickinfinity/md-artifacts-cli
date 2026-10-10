//! The per-fence dialect classifier (spec §9.6).

use crate::parse::text::js_trim;

/// Which reader a fence body goes to.
///
/// # Examples
///
/// ```
/// use mda_core::vks::Dialect;
/// assert_ne!(Dialect::Yaml, Dialect::Legacy);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Yaml,
    Legacy,
}

/// JS `\s` as the TS classifier saw it (U+FEFF in, U+0085 out), hand-coded: no Unicode regex class.
fn js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

/// `^[A-Za-z][A-Za-z0-9_-]*:(\s|$)`, hand-scanned (linear, no regex).
fn is_yaml_header(line: &str) -> bool {
    let mut it = line.chars();
    if !it.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    loop {
        match it.next() {
            Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
            Some(':') => return it.next().is_none_or(js_space),
            _ => return false,
        }
    }
}

/// Classify a fence body by its first significant line (not blank, first non-space char not `#`).
///
/// # Examples
///
/// ```
/// use mda_core::vks::{classify, Dialect};
/// assert_eq!(classify("# c\nVK-a: 1"), Dialect::Yaml);
/// assert_eq!(classify("VK-a=1"), Dialect::Legacy);
/// ```
pub fn classify(body: &str) -> Dialect {
    let first = crate::parse::text::js_lines(body).find(|l| {
        let t = js_trim(l);
        !t.is_empty() && !t.starts_with('#')
    });
    match first {
        Some(l) if is_yaml_header(l) => Dialect::Yaml,
        _ => Dialect::Legacy,
    }
}
