//! THE `<VK-…>` token grammar (spec §4.1, §10): plain, path, `each`/`end`/`join`. Detection only;
//! W3's renderer imports [`TOKEN_PATTERN`].

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::text::compile;
use crate::model::ParsedVar;

/// The one token-grammar regex source. Capture groups 1–4 each hold the root variable name of one
/// alternative: closing `</VK-x>`, plain/path `<VK-x(.seg)*>`, `each`/`join`, `end`. Hints are ASCII
/// (`[A-Za-z][A-Za-z0-9_]*`, never `\w`, which is Unicode in Rust); a separator is a quoted string
/// without `"` `\` `>` CR LF, bar the escapes `\\ \" \n \t`. A pair/backref is W3's concern.
///
/// # Examples
///
/// ```
/// let re = regex::Regex::new(mda_core::parse::tokens::TOKEN_PATTERN).unwrap();
/// assert!(re.is_match("<VK-each:users:\", \">"));
/// ```
pub const TOKEN_PATTERN: &str = r#"<(?:/VK-([A-Za-z][A-Za-z0-9_]*)>|VK-([A-Za-z][A-Za-z0-9_]*)(?:\.[A-Za-z][A-Za-z0-9_]*)*>|VK-(?:each|join):([A-Za-z][A-Za-z0-9_]*)(?:\.[A-Za-z][A-Za-z0-9_]*)*(?::"(?:[^"\\>\r\n]|\\[\\"nt])*")?>|VK-end:([A-Za-z][A-Za-z0-9_]*)(?:\.[A-Za-z][A-Za-z0-9_]*)*>)"#;

static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| compile(TOKEN_PATTERN));

/// The root `VK-<name>` of every token in `code`, deduped in first-appearance order, each with an
/// empty default.
///
/// # Examples
///
/// ```
/// let v = mda_core::parse::tokens::detect_vars("echo <VK-a> <VK-users.name> </VK-a>");
/// let names: Vec<_> = v.iter().map(|v| v.name.as_str()).collect();
/// assert_eq!(names, ["VK-a", "VK-users"]);
/// ```
pub fn detect_vars(code: &str) -> Vec<ParsedVar> {
    let mut seen = HashSet::new();
    TOKEN_RE
        .captures_iter(code)
        .filter_map(|c| (1..=4).find_map(|i| c.get(i)))
        .filter(|m| seen.insert(m.as_str()))
        .map(|m| ParsedVar::text(format!("VK-{}", m.as_str()), ""))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statics_compile() {
        LazyLock::force(&TOKEN_RE);
    }
}
