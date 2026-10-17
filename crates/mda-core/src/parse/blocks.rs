//! `##` block splitting and per-block payloads (`parser.service.ts:360-460`).

use std::ops::Range;

use super::fence::{self, is_js_terminator};
use super::text::js_trim;
use super::{frontmatter, overlay, tokens};
use crate::error::VarsError;
use crate::model::{ParsedBlock, ParsedVar};
use crate::vks;

/// Every `## ` section of `content` (frontmatter stripped) that has a heading and a code fence.
pub(super) fn parse_blocks(content: &str) -> Vec<ParsedBlock> {
    sections(frontmatter::strip(content))
        .filter_map(|(_, s)| parse_section(s))
        .collect()
}

/// `(heading, absolute byte range of the code)` of every block `parse_blocks` yields, in order:
/// the same sections, the same filter, spans taken from the parser's fence regex.
pub(crate) fn block_code_ranges(content: &str) -> Vec<(String, Range<usize>)> {
    let body = frontmatter::strip(content);
    let base = content.len() - body.len(); // `strip` returns a suffix of `content`
    sections(body)
        .filter_map(|(off, sec)| {
            let heading = js_trim(heading_raw(sec)?).to_owned();
            let r = fence::code_range(sec)?;
            Some((heading, base + off + r.start..base + off + r.end))
        })
        .collect()
}

/// Absolute byte ranges of the first `vks` fence body of every section `block_code_ranges` lists:
/// the bodies the parser reads (a leading `vks` code fence or the trailing defaults).
pub(crate) fn block_vks_ranges(content: &str) -> Vec<Range<usize>> {
    let body = frontmatter::strip(content);
    let base = content.len() - body.len();
    sections(body)
        .filter_map(|(off, sec)| {
            heading_raw(sec)?;
            fence::code_range(sec)?;
            let r = fence::vks_range(sec)?;
            Some(base + off + r.start..base + off + r.end)
        })
        .collect()
}

/// Index (into the parsed blocks) of the block whose `vks` body is the file-level one — the first
/// `vks` fence of the unstripped content (`fence.rs` `parse_vars`). Its `varsError` is the file's,
/// so a report shows it once (spec §11.3). `None` when no block reads that fence.
pub(crate) fn file_fence_block(content: &str) -> Option<usize> {
    let file = fence::vks_range(content)?;
    let body = frontmatter::strip(content);
    let base = content.len() - body.len();
    sections(body)
        .filter(|(_, sec)| heading_raw(sec).is_some() && fence::code_range(sec).is_some())
        .position(|(off, sec)| {
            fence::vks_range(sec).is_some_and(|r| {
                base + off + r.start == file.start && base + off + r.end == file.end
            })
        })
}

/// Slices starting at each `## ` that sits at a JS line start (offset 0 or after `\n \r U+2028
/// U+2029`; a `(?m)` regex breaks only at `\n`). Text before the first one is dropped.
fn sections(body: &str) -> impl Iterator<Item = (usize, &str)> {
    let at_line_start = |&i: &usize| {
        i == 0
            || body
                .get(..i)
                .and_then(|p| p.chars().next_back())
                .is_some_and(is_js_terminator)
    };
    let starts: Vec<usize> = body
        .match_indices("## ")
        .map(|(i, _)| i)
        .filter(at_line_start)
        .collect();
    let ends: Vec<usize> = starts
        .iter()
        .skip(1)
        .copied()
        .chain(std::iter::once(body.len()))
        .collect();
    starts
        .into_iter()
        .zip(ends)
        .filter_map(move |(a, b)| body.get(a..b).map(|s| (a, s)))
}

/// The heading line after `## `, untrimmed. `.+` must be non-empty before the trim: `## \n` is
/// dropped, `##  \n` is a heading of "".
fn heading_raw(section: &str) -> Option<&str> {
    let raw = section
        .strip_prefix("## ")?
        .split(is_js_terminator)
        .next()?;
    (!raw.is_empty()).then_some(raw)
}

fn parse_section(section: &str) -> Option<ParsedBlock> {
    let raw = heading_raw(section)?;
    let (code, fence_lang) = fence::code_block(section)?;
    // JS `slice(b < a)` is ""; `get` is `None` there.
    let start = section.find('\n').map_or(0, |i| i + 1);
    let end = section.find("```").unwrap_or(0);
    let description = section.get(start..end).map_or("", js_trim).to_owned();

    // A leading `vks` fence is a pure sub-set (an error empties it); otherwise the section's first
    // `vks` fence, if any, overlays defaults on the tokens detected in the code.
    let (vars, vars_error) = if fence_lang.as_deref() == Some("vks") {
        vks::read_fence(&code).map_or_else(|e| (vec![], Some(e)), |v| (v, None))
    } else {
        overlay(tokens::detect_vars(&code), trailing_defaults(section))
    };
    Some(ParsedBlock {
        heading: js_trim(raw).to_owned(),
        description,
        code,
        fence_lang,
        vars,
        vars_error,
    })
}

fn trailing_defaults(section: &str) -> Result<Vec<ParsedVar>, VarsError> {
    fence::vks_fence(section).map_or(Ok(vec![]), vks::read_fence)
}

/// Headings (`js_trim`med) of `## ` sections with no code fence, over the frontmatter-stripped body
/// (#54: the parser drops them, so every rewrite does). A heading-less `## ` line is skipped.
pub(crate) fn fenceless_headings(content: &str) -> Vec<String> {
    sections(frontmatter::strip(content))
        .filter(|(_, s)| fence::code_block(s).is_none())
        .filter_map(|(_, s)| heading_raw(s).map(|h| js_trim(h).to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_fence_block_is_positional() {
        // Block 0 holds the file-level fence; a later identical block is a different fence.
        let sets = "## A\n```vks\nVK-a=x\n```\n## B\n```vks\nVK-a=x\n```\n";
        assert_eq!(file_fence_block(sets), Some(0));
        // A leading untitled fence is the file-level one: no block reads it.
        let mixed = "```vks\nVK-t: 1\n```\n## A\n```vks\nVK-a: 1\n```\n";
        assert_eq!(file_fence_block(mixed), None);
        // Block 0 has no vks fence: the file-level fence is block 1's.
        let later = "## A\n```sh\nx\n```\n## B\n```vks\nVK-b: 2\n```\n";
        assert_eq!(file_fence_block(later), Some(1));
    }

    #[test]
    fn fenceless_headings_list() {
        let c = "## Empty\n\ntext\n## Full\n```vks\nVK-a: x\n```\n";
        assert_eq!(fenceless_headings(c), ["Empty"]);
        assert!(fenceless_headings("## \n").is_empty());
        assert_eq!(fenceless_headings("##  \n"), [""]);
    }
}
