//! Frontmatter (`parser.service.ts:14-46, 71-73, 126-195`).

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use super::text::{compile, js_lines, js_trim};
use crate::model::Frontmatter;
use crate::registry::ArtifactType;

// `---x` closes too (no line-end anchor after the closing dashes) and `---\n---` is not a block:
// both are TS behaviour, pinned by tests.
static BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| compile(r"\A---\r?\n((?s:.)*?)\r?\n---"));
static STRIP_RE: LazyLock<Regex> = LazyLock::new(|| compile(r"\A---\r?\n(?s:.)*?\r?\n---\r?\n?"));

/// `content` without its leading frontmatter block (itself when there is none).
pub(super) fn strip(content: &str) -> &str {
    match STRIP_RE.find(content) {
        Some(m) => content.get(m.end()..).unwrap_or(""),
        None => content,
    }
}

/// Frontmatter of `content`; `default` is the directory-derived type for files that name none.
pub(super) fn parse(content: &str, default: ArtifactType) -> Frontmatter {
    let mut fm = Frontmatter::new(default);
    let Some(block) = BLOCK_RE.captures(content).and_then(|c| c.get(1)) else {
        return fm;
    };
    for line in js_lines(block.as_str()) {
        if let Some((key, raw)) = split_kv(line) {
            apply(&mut fm, key, raw);
        }
    }
    fm
}

/// `key: raw` split at the first `:`, both sides `js_trim`med: the parser's line rule.
fn split_kv(line: &str) -> Option<(&str, &str)> {
    line.split_once(':').map(|(k, r)| (js_trim(k), js_trim(r)))
}

/// The key a frontmatter `line` sets, by the parser's own rule (the patcher finds lines with it).
pub(crate) fn key_of(line: &str) -> Option<&str> {
    split_kv(line).map(|(k, _)| k)
}

/// Byte range of the frontmatter text between the dashes (BLOCK_RE group 1); `None` without a block.
pub(crate) fn body_range(content: &str) -> Option<Range<usize>> {
    BLOCK_RE.captures(content)?.get(1).map(|m| m.range())
}

fn apply(fm: &mut Frontmatter, key: &str, raw: &str) {
    let text = || Some(raw.to_owned());
    match key {
        // Exact match only: `snippet` (wrong case) falls back to the directory type.
        "artifactType" => {
            if let Some(t) = ArtifactType::from_name(raw) {
                fm.artifact_type = t;
            }
        }
        "tags" => fm.tags = Some(inline_array(raw)),
        "paths" => fm.paths = Some(inline_array(raw)),
        "index" => fm.index = Some(raw == "true"),
        "title" => fm.title = text(),
        "description" => fm.description = text(),
        "language" => fm.language = text(),
        "env" => fm.env = text(),
        "target" => fm.target = text(),
        "extension" => fm.extension = text(),
        "provider" => fm.provider = text(),
        "model" => fm.model = text(),
        "version" => fm.version = text(),
        _ => {}
    }
}

/// `[a, b]` or `a, b`: one optional bracket each side stripped, split on `,`, trimmed, empties dropped.
fn inline_array(raw: &str) -> Vec<String> {
    let inner = raw.strip_prefix('[').unwrap_or(raw);
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    inner
        .split(',')
        .map(js_trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statics_compile() {
        LazyLock::force(&BLOCK_RE);
        LazyLock::force(&STRIP_RE);
    }
}
