//! `extract_config_blocks`: every ```` ```yaml leetcode ```` fence of a body .

use super::*;
use crate::leetcode::truncate_param;
use crate::render::Warning;

/// One config fence's byte range in the body it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConfigSpan {
    /// Offset of the opening fence line, inclusive.
    pub start: usize,
    /// Offset just past the closing fence line's own characters, exclusive.
    pub end: usize,
}

/// Result of walking a body for config fences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigBlocksResult {
    /// Every fence body in document order, joined with `\n`.
    pub raw: String,
    /// One span per fence, offsets into the walked body.
    pub spans: Vec<ConfigSpan>,
    /// `leetcode.config.*` warnings (W03-W07).
    pub warnings: Vec<Warning>,
}

/// Which occurrence wins when a top-level key is declared in two fences (TS `DUPLICATE_KEY_WINNER`).
/// A `match`, so `__proto__` and friends are plain misses.
fn duplicate_winner(key: &str) -> Option<&'static str> {
    match key {
        "function" | "functions" | "params" | "returns" | "test" | "practice" | "tags" => {
            Some("last")
        }
        "libs" | "checks" | "packages" => Some("first"),
        _ => None,
    }
}

fn warning(code: &'static str, params: &[(&str, String)]) -> Warning {
    Warning {
        code,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    }
}

/// `^```\s*$` on a `\r`-stripped line.
fn is_closing_fence(line: &str) -> bool {
    line.strip_prefix("```")
        .is_some_and(|rest| js_trim(rest).is_empty())
}

/// `^yaml\s+leetcode\s*$` on the text after the opening backticks.
fn is_config_marker(info: &str) -> bool {
    info.strip_prefix("yaml")
        .is_some_and(|r| starts_with_ws(r) && js_trim(r) == "leetcode")
}

fn bare(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

/// The fence body after an opening line: `(content lines, span_end)`, `None` when never closed.
fn read_fence<'a>(
    lines: &mut std::str::Split<'a, char>,
    mut offset: usize,
) -> Option<(Vec<&'a str>, usize)> {
    let mut content = vec![];
    for raw in lines {
        let line = bare(raw);
        if is_closing_fence(line) {
            return Some((content, offset + raw.len()));
        }
        offset += raw.len() + 1;
        content.push(line);
    }
    None
}

/// D5: a fence's first non-blank line cannot nest under anything, so an indent there is the defect.
fn check_indentation(content: &[&str], warnings: &mut Vec<Warning>) {
    if let Some(first) = content.iter().find(|l| !js_trim(l).is_empty())
        && starts_with_ws(first)
    {
        warnings.push(warning(
            "leetcode.config.indented",
            &[("line", truncate_param(first))],
        ));
    }
}

/// Column-0 keys of a fence that matter to the warnings (retained or duplicate-sensitive), once each.
/// Filtering here keeps the per-fence set tiny however hostile the fence is.
fn relevant_keys<'a>(content: &[&'a str]) -> Vec<&'a str> {
    let mut keys: Vec<&str> = vec![];
    for key in content.iter().filter_map(|l| top_level_key(l)) {
        let relevant = RETAINED_FM_KEYS.contains(&key) || duplicate_winner(key).is_some();
        if relevant && !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

fn warn_retained(block_keys: &[Vec<&str>], warnings: &mut Vec<Warning>) {
    let mut seen: Vec<&str> = vec![];
    for key in block_keys.iter().flatten() {
        if RETAINED_FM_KEYS.contains(key) && !seen.contains(key) {
            seen.push(key);
            warnings.push(warning(
                "leetcode.config.key_in_frontmatter",
                &[("key", (*key).to_owned())],
            ));
        }
    }
}

fn warn_duplicates(block_keys: &[Vec<&str>], warnings: &mut Vec<Warning>) {
    let mut counts: Vec<(&str, usize)> = vec![];
    for key in block_keys.iter().flatten() {
        match counts.iter_mut().find(|(k, _)| k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key, 1)),
        }
    }
    for (key, count) in counts {
        if let (true, Some(winner)) = (count >= 2, duplicate_winner(key)) {
            let params = [
                ("key", key.to_owned()),
                ("count", count.to_string()),
                ("winner", winner.to_owned()),
            ];
            warnings.push(warning("leetcode.config.duplicate_key", &params));
        }
    }
}

/// Walk `body` for every ```` ```yaml leetcode ```` fence in one linear pass (TS
/// `extractConfigBlocks`). Any other fence is one opaque region, so fence-shaped lines inside it
/// are never config; an unterminated config fence reads nothing and stops the walk.
#[allow(dead_code)] // reason: seam, used by T4.5
pub(crate) fn extract_config_blocks(body: &str) -> ConfigBlocksResult {
    let mut warnings = vec![];
    let (mut blocks, mut spans, mut block_keys): (Vec<String>, Vec<ConfigSpan>, Vec<Vec<&str>>) =
        Default::default();
    let mut lines = body.split('\n');
    let (mut offset, mut fenced) = (0, false);
    while let Some(raw) = lines.next() {
        let line = bare(raw);
        let opens = !fenced && line.strip_prefix("```").is_some_and(is_config_marker);
        if opens {
            let Some((content, span_end)) = read_fence(&mut lines, offset + raw.len() + 1) else {
                warnings.push(warning("leetcode.config.unterminated_fence", &[]));
                break;
            };
            spans.push(ConfigSpan {
                start: offset,
                end: span_end,
            });
            check_indentation(&content, &mut warnings);
            block_keys.push(relevant_keys(&content));
            blocks.push(content.join("\n"));
            offset = span_end + 1;
            continue;
        }
        if line.starts_with("```") {
            fenced = !fenced;
        }
        offset += raw.len() + 1;
    }
    if fenced {
        warnings.push(warning("leetcode.config.hidden_by_open_fence", &[]));
    }
    warn_retained(&block_keys, &mut warnings);
    warn_duplicates(&block_keys, &mut warnings);
    ConfigBlocksResult {
        raw: blocks.join("\n"),
        spans,
        warnings,
    }
}
