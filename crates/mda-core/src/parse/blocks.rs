//! `##` block splitting and per-block payloads (`parser.service.ts:360-460`).

use super::fence::{self, is_js_terminator};
use super::text::js_trim;
use super::{frontmatter, overlay, tokens};
use crate::error::VarsError;
use crate::model::{ParsedBlock, ParsedVar};
use crate::vks;

/// Every `## ` section of `content` (frontmatter stripped) that has a heading and a code fence.
pub(super) fn parse_blocks(content: &str) -> Vec<ParsedBlock> {
    sections(frontmatter::strip(content))
        .filter_map(parse_section)
        .collect()
}

/// Slices starting at each `## ` that sits at a JS line start (offset 0 or after `\n \r U+2028
/// U+2029`; a `(?m)` regex breaks only at `\n`). Text before the first one is dropped.
fn sections(body: &str) -> impl Iterator<Item = &str> {
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
        .filter_map(move |(a, b)| body.get(a..b))
}

fn parse_section(section: &str) -> Option<ParsedBlock> {
    // `.+` must be non-empty before the trim: `## \n` is dropped, `##  \n` is a heading of "".
    let raw = section
        .strip_prefix("## ")?
        .split(is_js_terminator)
        .next()?;
    if raw.is_empty() {
        return None;
    }
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
