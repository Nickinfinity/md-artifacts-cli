//! Marker matching for `each`/`end` (iterative, depth-capped), block mode, and `join` (T3.3).

use super::budget::{Budget, Warnings};
use super::resolve::Item;
use super::scan::{Kind, Tok};
use super::{MAX_LOOP_DEPTH, Warning};
use crate::error::{
    LimitExceeded, RENDER_SELF_NESTED, RENDER_UNMATCHED_END, RENDER_UNTERMINATED, RenderLimit,
};
use crate::parse::text::js_trim;

struct Open<'a> {
    idx: usize,
    path: &'a str,
    /// Self-nested: pairs with its own `end` but both stay literal.
    failed: bool,
}

pub(super) fn warn(w: &mut Warnings, code: &'static str, t: &Tok<'_>, path: &str) {
    w.add(Warning::new(code, t.line).with("path", format!("VK-{path}")));
}

/// Pair every `each` with its `end` before anything expands. `partner[i]` is the other marker's
/// index, `None` for a literal marker. Markers must nest: an `end` closes only the innermost open
/// `each` of the same path. A push past [`MAX_LOOP_DEPTH`] refuses, so the evaluator's recursion
/// (one level per pair) is bounded before it starts.
pub(super) fn match_markers(
    toks: &[Tok<'_>],
    warnings: &mut Warnings,
) -> Result<Vec<Option<usize>>, LimitExceeded> {
    let mut partner = vec![None; toks.len()];
    let mut stack: Vec<Open<'_>> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match &t.kind {
            Kind::Each { path, .. } => {
                if stack.len() >= MAX_LOOP_DEPTH {
                    return Err(LimitExceeded {
                        limit: RenderLimit::Depth,
                        max: MAX_LOOP_DEPTH,
                    });
                }
                let failed = stack.iter().any(|o| o.path == *path);
                if failed {
                    warn(warnings, RENDER_SELF_NESTED, t, path);
                }
                stack.push(Open {
                    idx: i,
                    path,
                    failed,
                });
            }
            Kind::End { path } => match stack.pop_if(|o| o.path == *path) {
                Some(o) if !o.failed => {
                    set(&mut partner, o.idx, i);
                    set(&mut partner, i, o.idx);
                }
                Some(_) => {}
                None => warn(warnings, RENDER_UNMATCHED_END, t, path),
            },
            _ => {}
        }
    }
    for o in stack {
        if let Some(t) = toks.get(o.idx) {
            warn(warnings, RENDER_UNTERMINATED, t, o.path);
        }
    }
    Ok(partner)
}

fn set(partner: &mut [Option<usize>], at: usize, to: usize) {
    if let Some(p) = partner.get_mut(at) {
        *p = Some(to);
    }
}

/// Where a block-mode pair sits: both marker lines are cut out whole.
#[derive(Clone, Copy)]
pub(super) struct BlockSpan<'a> {
    /// Start of the `each` line (leading blanks included).
    pub cut_from: usize,
    /// First byte after the `each` line's terminator.
    pub body_from: usize,
    /// End of the body, its final terminator excluded.
    pub body_to: usize,
    /// The body's final terminator (`\r\n`, `\n`, or `""`).
    pub nl: &'a str,
    /// First byte after the `end` line's terminator.
    pub after: usize,
}

/// Block mode iff both markers are alone on their lines. Decided once per pair.
pub(super) fn block_mode<'a>(
    code: &'a str,
    each: &Tok<'_>,
    end: &Tok<'_>,
) -> Option<BlockSpan<'a>> {
    let (cut_from, body_from) = line_bounds(code, each)?;
    let (end_line, after) = line_bounds(code, end)?;
    let body = code.get(body_from..end_line)?;
    let nl_len = if body.ends_with("\r\n") {
        2
    } else {
        usize::from(body.ends_with('\n'))
    };
    let body_to = end_line.saturating_sub(nl_len);
    Some(BlockSpan {
        cut_from,
        body_from,
        body_to,
        nl: code.get(body_to..end_line)?,
        after,
    })
}

fn blank(c: char) -> bool {
    js_trim(c.encode_utf8(&mut [0; 4])).is_empty()
}

/// The line holding `t`: (start, end after the terminator), or `None` when anything but blanks
/// shares it. Scans outward and stops at the first non-blank, so the cost is the distance walked.
fn line_bounds(code: &str, t: &Tok<'_>) -> Option<(usize, usize)> {
    let mut start = 0;
    for (i, c) in code.get(..t.start)?.char_indices().rev() {
        if c == '\n' {
            start = i + 1;
            break;
        }
        if !blank(c) {
            return None;
        }
    }
    let mut end = code.len();
    for (i, c) in code.get(t.end..)?.char_indices() {
        if c == '\n' {
            end = t.end + i + 1;
            break;
        }
        if !blank(c) {
            return None;
        }
    }
    Some((start, end))
}

/// What a `join` walk reached.
#[derive(PartialEq, Eq)]
pub(super) enum JoinShape {
    Strings,
    Record,
}

/// Collect every string reached by `segs` from `v`, flattening lists on the way. One step per
/// node visited, so a join that emits nothing is still bounded. Recursion follows the value's
/// nesting, which `vks` caps at `MAX_DEPTH`.
pub(super) fn join_values<'v>(
    v: Item<'v>,
    segs: &[&str],
    budget: &mut Budget,
    out: &mut Vec<&'v str>,
) -> Result<JoinShape, LimitExceeded> {
    budget.step()?;
    match (v, segs.split_first()) {
        (Item::Str(s), None) => out.push(s),
        (Item::Rec(_), None) => return Ok(JoinShape::Record),
        (Item::List(l), _) => {
            for k in 0..Item::list_len(l) {
                if let Some(item) = Item::list_item(l, k)
                    && join_values(item, segs, budget, out)? == JoinShape::Record
                {
                    return Ok(JoinShape::Record);
                }
            }
        }
        (item, Some((seg, rest))) => {
            if let Some(next) = item.field(seg) {
                return join_values(next, rest, budget, out);
            }
        }
    }
    Ok(JoinShape::Strings)
}
