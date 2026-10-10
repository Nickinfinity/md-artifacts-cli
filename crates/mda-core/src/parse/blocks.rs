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
