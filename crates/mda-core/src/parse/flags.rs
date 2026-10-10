//! Flagged plain-markdown payload regions (spec §7; `flags.service.ts:36-280`).
//! The only module that spells the marker; `tests/flags.rs` guards that.

use std::sync::LazyLock;

use regex::Regex;

use super::text::{compile, js_lines, js_trim};

// Line-local, so `\A…\z` (JS `^…$` without `m`). `[A-Za-z0-9_]`-style classes are not needed here.
static START_FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\A[ \t]*%%[ \t]*oa:start(?:[ \t]([^%\n]*))?%%[ \t]*\z"));
static END_FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\A[ \t]*%%[ \t]*oa:end[ \t]*%%[ \t]*\z"));
static FENCE_RE: LazyLock<Regex> = LazyLock::new(|| compile(r"\A {0,3}(`{3,}|~{3,})"));

/// One flagged region: its name (`""` when unnamed) and its trimmed content.
///
/// # Examples
///
/// ```
/// use mda_core::parse::flags::FlaggedRegion;
/// let r = FlaggedRegion { name: String::new(), content: "x".into() };
/// assert_eq!(r.content, "x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlaggedRegion {
    pub name: String,
    pub content: String,
}

/// `***` visual rule: 3+ asterisks and nothing else but surrounding whitespace.
fn is_visual_rule(line: &str) -> bool {
    let t = js_trim(line);
    t.len() >= 3 && t.bytes().all(|b| b == b'*')
}

/// The fence run a line opens, if any.
fn fence_opener(line: &str) -> Option<&str> {
    FENCE_RE.captures(line)?.get(1).map(|m| m.as_str())
}

/// Same char, run at least as long; info after the run is tolerated.
fn closes_fence(marker: &str, line: &str) -> bool {
    let run = fence_opener(line).unwrap_or("");
    run.chars().next() == marker.chars().next() && !run.is_empty() && run.len() >= marker.len()
}

/// Blank lines trimmed at both ends only (indentation of the first line survives).
fn region_content(lines: &[&str]) -> String {
    let blank = |l: &&str| js_trim(l).is_empty();
    let start = lines.iter().position(|l| !blank(l)).unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !blank(l))
        .map_or(start, |i| i + 1);
    lines.get(start..end).unwrap_or(&[]).join("\n")
}

/// Every flagged region of `body` (frontmatter already stripped), in document order.
///
/// Fences are tracked inside and outside regions, so a documented flag inside a sample fence is
/// payload. An unterminated region runs to EOF; a second start inside an open region is content.
///
/// # Examples
///
/// ```
/// use mda_core::parse::flags::extract_flagged_regions;
/// let r = extract_flagged_regions("%%oa:start Dev%%\nrun it\n%%oa:end%%");
/// assert_eq!((r[0].name.as_str(), r[0].content.as_str()), ("Dev", "run it"));
/// ```
pub fn extract_flagged_regions(body: &str) -> Vec<FlaggedRegion> {
    let mut regions = Vec::new();
    let mut fence: Option<&str> = None;
    // `Some` ⇔ a region is open.
    let mut open: Option<(String, Vec<&str>)> = None;
    let close = |open: &mut Option<(String, Vec<&str>)>, regions: &mut Vec<FlaggedRegion>| {
        if let Some((name, lines)) = open.take() {
            regions.push(FlaggedRegion {
                name,
                content: region_content(&lines),
            });
        }
    };
    for line in js_lines(body) {
        if let Some(m) = fence {
            if closes_fence(m, line) {
                fence = None;
            }
        } else if let Some(m) = fence_opener(line) {
            fence = Some(m);
        } else {
            let start = START_FLAG_RE.captures(line);
            if let (Some(c), None) = (&start, &open) {
                let name = c.get(1).map_or("", |m| js_trim(m.as_str()));
                open = Some((name.to_owned(), Vec::new()));
                continue;
            }
            if open.is_some() && END_FLAG_RE.is_match(line) {
                close(&mut open, &mut regions);
                continue;
            }
            // `***` is chrome only inside a region; fenced lines never reach here.
            if open.is_some() && is_visual_rule(line) {
                continue;
            }
        }
        if let Some((_, lines)) = open.as_mut() {
            lines.push(line);
        }
    }
    close(&mut open, &mut regions);
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statics_compile() {
        LazyLock::force(&START_FLAG_RE);
        LazyLock::force(&END_FLAG_RE);
        LazyLock::force(&FENCE_RE);
    }
}
