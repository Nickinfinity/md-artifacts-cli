//! Code fences and the vars section (`parser.service.ts:47-48, 210-276, 581-585`).

use std::sync::LazyLock;

use regex::Regex;

use super::frontmatter;
use super::text::{compile, js_trim_end};
use crate::error::VarsError;
use crate::model::ParsedVar;
use crate::vks;

// A fence closes on the first ``` even mid-line, and ```c++ never opens one: TS behaviour.
static CODE_FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"```([A-Za-z0-9_]*)\r?\n((?s:.)*?)```"));
static VKS_FENCE_RE: LazyLock<Regex> = LazyLock::new(|| compile(r"```vks\r?\n((?s:.)*?)```"));
// `\z`, not `$`: JS `$` without `m` is end of input. `(?-u:\b)`: ASCII word boundary like JS.
static VARS_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?-u:\b)vars:?[ \t]*\r?\n((?s:.)+?)(?:\n\n|\z)"));

/// JS line terminators, the set `^`/`$` break at under the `m` flag.
pub(super) fn is_js_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The first code fence of `s`: `(code, language)`; code is end-trimmed, an empty info string is
/// `None`. `None` when `s` has no fence.
pub(super) fn code_block(s: &str) -> Option<(String, Option<String>)> {
    let c = CODE_FENCE_RE.captures(s)?;
    let lang = c.get(1).map(|m| m.as_str()).filter(|l| !l.is_empty());
    let code = c.get(2).map_or("", |m| js_trim_end(m.as_str()));
    Some((code.to_owned(), lang.map(str::to_owned)))
}

/// The raw body of the first ```` ```vks ```` fence in `s`.
pub(super) fn vks_fence(s: &str) -> Option<&str> {
    VKS_FENCE_RE.captures(s)?.get(1).map(|m| m.as_str())
}

/// File-level defaults: the first `vks` fence anywhere in the **unstripped** `content`, else the
/// unfenced `vars:` section after the first code fence. `Ok([])` when neither exists.
pub(super) fn parse_vars(content: &str) -> Result<Vec<ParsedVar>, VarsError> {
    if let Some(body) = vks_fence(content) {
        return vks::read_fence(body);
    }
    let after_code = CODE_FENCE_RE.replacen(frontmatter::strip(content), 1, "");
    match VARS_SECTION_RE.captures(&after_code).and_then(|c| c.get(1)) {
        Some(m) => vks::read_section(m.as_str()),
        None => Ok(vec![]),
    }
}

/// `body` minus its first `vks` fence and the first `vars:` label line (that line's text only; its
/// terminator stays).
pub(super) fn strip_vars_section(body: &str) -> String {
    let body = VKS_FENCE_RE.replacen(body, 1, "");
    let mut done = false;
    let mut out = String::with_capacity(body.len());
    for seg in body.split_inclusive(is_js_terminator) {
        let (text, term) = match seg.char_indices().next_back() {
            Some((i, c)) if is_js_terminator(c) => seg.split_at(i),
            _ => (seg, ""),
        };
        if !done && is_vars_label(text) {
            done = true;
            out.push_str(term);
        } else {
            out.push_str(seg);
        }
    }
    out
}

/// `^[ \t]*vars:?[ \t]*$`
fn is_vars_label(line: &str) -> bool {
    let blank = |c: char| c == ' ' || c == '\t';
    let Some(rest) = line.trim_start_matches(blank).strip_prefix("vars") else {
        return false;
    };
    rest.strip_prefix(':')
        .unwrap_or(rest)
        .trim_start_matches(blank)
        .is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statics_compile() {
        LazyLock::force(&CODE_FENCE_RE);
        LazyLock::force(&VKS_FENCE_RE);
        LazyLock::force(&VARS_SECTION_RE);
    }

    #[test]
    fn vars_label_blanks_only_the_first_label_line() {
        assert_eq!(
            strip_vars_section("a\n  vars: \r\nvars\nb"),
            "a\n\r\nvars\nb"
        );
        assert_eq!(
            strip_vars_section("x\u{2028}vars:\u{2029}y"),
            "x\u{2028}\u{2029}y"
        );
        assert_eq!(strip_vars_section("vars: x"), "vars: x");
    }
}
