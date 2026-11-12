//! The one scalar-quoting authority for the canonical emitters (W7 Q-76): no emitter quotes by hand.
//! Each function is the inverse of the reader it writes for, proven by round-trip tests.

use crate::leetcode::config::is_ws;
use crate::parse::text::js_trim;

/// A value for a `key: value` line read back through `unquote_pair` (config fences: `libs`,
/// `packages`, checks): written bare when bare reads back unchanged, else wrapped in `"`. `None` when
/// no form reads back (a line break inside the value): the caller refuses.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::emit::pair_quoted;
/// assert_eq!(pair_quoted("web").as_deref(), Some("web"));
/// assert_eq!(pair_quoted(" padded ").as_deref(), Some("\" padded \""));
/// assert_eq!(pair_quoted("'q'").as_deref(), Some("\"'q'\""));
/// assert_eq!(pair_quoted("a\nb"), None);
/// ```
pub fn pair_quoted(s: &str) -> Option<String> {
    if s.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    let paired = s.len() >= 2
        && ['"', '\'']
            .iter()
            .any(|&q| s.starts_with(q) && s.ends_with(q));
    Some(if s.is_empty() || js_trim(s) != s || paired {
        format!("\"{s}\"")
    } else {
        s.to_owned()
    })
}

/// An info-string attribute value (`path=…` on a ```` ``` ```` line), read back by the project
/// parser's `attr` (`key=(?:"([^"]*)"|(\S+))`, JS `\s`): bare when non-empty, free of JS whitespace and
/// not opening with `"`; else `"…"` when it holds no `"` and no line break; else `None` (refuse).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::emit::attr_quoted;
/// assert_eq!(attr_quoted("src/a.ts").as_deref(), Some("src/a.ts"));
/// assert_eq!(attr_quoted("my file.ts").as_deref(), Some("\"my file.ts\""));
/// assert_eq!(attr_quoted("a \"b"), None);
/// ```
pub fn attr_quoted(s: &str) -> Option<String> {
    if s.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    if !s.is_empty() && !s.starts_with('"') && !s.chars().any(is_ws) {
        return Some(s.to_owned());
    }
    (!s.contains('"')).then(|| format!("\"{s}\""))
}

/// A YAML double-quoted string for a test-case value, read back by the cases reader's `read_quoted`
/// (escapes `\\`, `\"`, `\n`, `\t`, `\r`; every other character literal — the reader splits lines on
/// `\n` only, so U+2028 stays inside). Always representable.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::emit::case_quoted;
/// assert_eq!(case_quoted("a\"b\\c\n"), r#""a\"b\\c\n""#);
/// ```
pub fn case_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leetcode::CaseValue;
    use crate::leetcode::cases::parse_yaml_cases;
    use crate::leetcode::config::unquote_pair;

    const HOSTILE: &[&str] = &[
        "",
        "x",
        " x",
        "x ",
        "\"x\"",
        "'x'",
        "\"",
        "'",
        "a\"b",
        "a'b",
        "#x",
        "x # y",
        "---",
        "```",
        "\u{feff}x",
        "x\u{a0}",
        "a: b",
        "[a, b]",
        "{a}",
        "\\",
        "__proto__",
        "\u{2028}",
        "\t",
    ];

    #[test]
    fn pair_quoted_reads_back_through_unquote_pair() {
        for s in HOSTILE {
            match pair_quoted(s) {
                Some(q) => assert_eq!(unquote_pair(&q), *s, "{s:?} -> {q:?}"),
                None => assert!(s.contains(['\n', '\r', '\u{2028}', '\u{2029}']), "{s:?}"),
            }
        }
        assert_eq!(pair_quoted("a\rb"), None);
    }

    #[test]
    fn case_quoted_reads_back_through_the_cases_reader() {
        let mut all: Vec<String> = HOSTILE.iter().map(|s| (*s).to_owned()).collect();
        all.extend([
            "a\nb".to_owned(),
            "x\r\n".to_owned(),
            "\\n".to_owned(),
            "\u{2029}y".to_owned(),
        ]);
        for s in all {
            let doc = format!("k: {}\n", case_quoted(&s));
            let CaseValue::Object(m) = parse_yaml_cases(&doc) else {
                panic!("{doc:?}");
            };
            assert_eq!(
                m.0.first().map(|(_, v)| v),
                Some(&CaseValue::Str(s.clone())),
                "{doc:?}"
            );
        }
    }
}
