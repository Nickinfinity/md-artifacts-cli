//! Config blocks and the frontmatter/body split (TS `leetcode-config-blocks.helpers.ts`): the
//! body-set key list, the top-level `key:` grammar, the shared block-line grammar and
//! [`extract_config_blocks`] (in `extract.rs`). Everything here is a line scanner; no `\w`/`\s`.

mod extract;
#[cfg(test)]
mod tests;

use crate::parse::text::{js_lines, js_trim};
use crate::parse::{body_range, frontmatter_end};

#[cfg(test)]
pub(crate) use extract::ConfigBlocksResult;
pub(crate) use extract::{ConfigSpan, extract_config_blocks};

/// Frontmatter text longer than this is "no frontmatter" (Rust-only cap, Q-37/Q-39).
const MAX_FRONTMATTER_BYTES: usize = 64 * 1024;

/// The D2 body-set keys: execution config that lives in a body fence, never in frontmatter.
pub(crate) const BODY_SET_KEYS: &[&str] = &[
    "function",
    "functions",
    "params",
    "returns",
    "test",
    "practice",
    "libs",
    "checks",
    "packages",
    "program",
];

/// The keys that stay in frontmatter, in the canonical order a clean artifact writes them.
pub(crate) const CANONICAL_ORDER: &[&str] = &[
    "artifactType",
    "leetcodeType",
    "title",
    "difficulty",
    "status",
    "algorithm",
    "tags",
];

/// [`CANONICAL_ORDER`] as the membership set most callers need (order is the authority).
pub(crate) const RETAINED_FM_KEYS: &[&str] = CANONICAL_ORDER;

/// JS `\s` over one char (the `js_trim` set).
pub(super) fn is_ws(c: char) -> bool {
    js_trim(c.encode_utf8(&mut [0; 4])).is_empty()
}

/// JS `/^\s/.test(line)`.
pub(super) fn starts_with_ws(line: &str) -> bool {
    line.chars().next().is_some_and(is_ws)
}

/// TS `TOP_LEVEL_KEY_RE` `/^(\w+):/`: the key a column-0 line opens (`\w` is ASCII in JS).
pub(super) fn top_level_key(line: &str) -> Option<&str> {
    let end = line.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    (end > 0 && line[end..].starts_with(':')).then(|| &line[..end])
}

/// TS `KV_RE` `/^(\w+):\s*(.*)$/`: `(key, value)`, the value untrimmed on the right. `.` and `$`
/// refuse a line terminator in the value (`\n`, `\r`, U+2028, U+2029).
pub(crate) fn kv_line(line: &str) -> Option<(&str, &str)> {
    let key = top_level_key(line)?;
    let rest = line.get(key.len() + 1..)?;
    let value = rest.trim_start_matches(is_ws);
    (!value.contains(['\n', '\r', '\u{2028}', '\u{2029}'])).then_some((key, value))
}

/// Body-set keys found at column 0 in raw frontmatter text, first-seen order, each once (TS
/// `legacyFrontmatterKeys`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::legacy_frontmatter_keys;
/// assert_eq!(legacy_frontmatter_keys("title: X\nfunction: twoSum\n"), ["function"]);
/// ```
pub fn legacy_frontmatter_keys(fm_raw: &str) -> Vec<String> {
    let mut found: Vec<String> = vec![];
    for key in js_lines(fm_raw).filter_map(top_level_key) {
        if BODY_SET_KEYS.contains(&key) && !found.iter().any(|f| f == key) {
            found.push(key.to_owned());
        }
    }
    found
}

/// Raw frontmatter with every body-set key's line and indented continuation lines removed (TS
/// `withoutBodySetKeys`). Splits on a bare `\n` so CRLF survives; a blank line ends a block.
pub(crate) fn without_body_set_keys(fm_raw: &str) -> String {
    let mut kept: Vec<&str> = vec![];
    let mut dropping = false;
    for line in fm_raw.split('\n') {
        let bare = line.strip_suffix('\r').unwrap_or(line);
        if let Some(key) = top_level_key(bare) {
            dropping = BODY_SET_KEYS.contains(&key);
        } else if dropping && !starts_with_ws(bare) {
            dropping = false;
        }
        if !dropping {
            kept.push(line);
        }
    }
    kept.join("\n")
}

/// `(fm_raw, body)` of a decoded `.md` (TS `splitFrontmatter`): `fm_raw` has no dashes; with no
/// block, or one over 64 KiB (the whole block, both `---` lines included), `fm_raw` is `""` (the
/// block is skipped, the body follows it).
pub(crate) fn split_frontmatter(content: &str) -> (&str, &str) {
    let end = frontmatter_end(content);
    let body = content.get(end..).unwrap_or(content);
    match body_range(content).and_then(|r| content.get(r)) {
        Some(fm) if end <= MAX_FRONTMATTER_BYTES => (fm, body),
        _ => ("", body),
    }
}

/// Count of leading whitespace chars (TS `indentOf`).
pub(crate) fn indent_of(line: &str) -> usize {
    line.chars().take_while(|&c| is_ws(c)).count()
}

/// Lines indented deeper than the header at `start`, up to the first that is not; blank lines
/// are skipped without ending the block (TS `blockLines`).
pub(crate) fn block_lines<'a>(lines: &[&'a str], start: usize) -> Vec<&'a str> {
    let Some(header) = lines.get(start) else {
        return vec![];
    };
    let base = indent_of(header);
    let mut out = vec![];
    for line in lines.iter().skip(start + 1) {
        if js_trim(line).is_empty() {
            continue;
        }
        if indent_of(line) <= base {
            break;
        }
        out.push(*line);
    }
    out
}

/// Trim, then strip one matching pair of quotes (TS config-blocks `unquote`). Not migrate's
/// `unquote`, which strips each end alone. `.` refuses a line terminator inside the pair.
pub(crate) fn unquote_pair(value: &str) -> &str {
    let t = js_trim(value);
    ['"', '\'']
        .iter()
        .find_map(|&q| t.strip_prefix(q)?.strip_suffix(q))
        .filter(|inner| !inner.contains(['\n', '\r', '\u{2028}', '\u{2029}']))
        .unwrap_or(t)
}
