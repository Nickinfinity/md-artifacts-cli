//! vks migration: legacy `KEY=value` bodies the parser reads → §9.5 YAML, whole file or nothing
//! (spec §11.2). Rust-only (seed D-6).

use std::ops::Range;

use super::Skip;
use crate::error::Limit;
use crate::model::ParsedArtifact;
use crate::parse::text::{js_lines, js_trim};
use crate::parse::{
    block_code_ranges, is_flagged, parse_from_content, top_code_range, unfenced_vars,
    vks_body_ranges_of,
};
use crate::vks::{Dialect, MAX_BODY_BYTES, check_vars, classify, emit_body, read_fence};

/// One planned rewrite: migrated fence count, variables over them, legacy lines dropped, new content.
///
/// # Examples
///
/// ```
/// use mda_core::migrate::vks::VksRewrite;
/// let r = VksRewrite { fences: 1, vars: 2, dropped_lines: 0, content: String::new() };
/// assert_eq!(r.fences, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VksRewrite {
    pub fences: usize,
    pub vars: usize,
    pub dropped_lines: usize,
    pub content: String,
}

/// Legacy bodies the parser reads → §9.5 YAML; whole file or nothing. `path` = vault-relative
/// (decides the default type).
///
/// # Examples
///
/// ```
/// let c = "```yaml VKs\nVK-a=1\n```\n";
/// let r = mda_core::migrate::vks::rewrite(c, "Variables/a.md").unwrap().unwrap();
/// assert_eq!(r.content, "```yaml VKs\nVK-a: '1'\n```\n");
/// assert!(mda_core::migrate::vks::rewrite("", "Variables/a.md").unwrap().is_none());
/// ```
pub fn rewrite(content: &str, path: &str) -> Result<Option<VksRewrite>, Skip> {
    let before = Side::new(content, path);
    let ranges = &before.ranges;
    if ranges.is_empty() {
        return match unfenced_vars(content) {
            Some(Ok(v)) if !v.is_empty() => Err(Skip::UnfencedVars),
            _ => Ok(None),
        };
    }
    let mut out = String::with_capacity(content.len());
    let (mut prev, mut r) = (
        0,
        VksRewrite {
            fences: 0,
            vars: 0,
            dropped_lines: 0,
            content: String::new(),
        },
    );
    for range in ranges {
        let Some(body) = content.get(range.clone()) else {
            continue;
        };
        let Some((yaml, vars, nonblank)) = migrate_body(content, range, body)? else {
            continue;
        };
        out.push_str(content.get(prev..range.start).unwrap_or_default());
        out.push_str(&yaml);
        prev = range.end;
        r.fences += 1;
        r.vars += vars;
        r.dropped_lines += nonblank.saturating_sub(vars);
    }
    if r.fences == 0 {
        return Ok(None);
    }
    out.push_str(content.get(prev..).unwrap_or_default());
    guard(before, &out, path)?;
    r.content = out;
    Ok(Some(r))
}

/// One legacy body → `(yaml, var count, non-blank legacy lines)`; `None` when it is not a
/// candidate (YAML, or no variables: classify says Legacy for empty / comment-only bodies, and
/// rewriting those to `""` would re-list them forever).
fn migrate_body(
    content: &str,
    range: &Range<usize>,
    body: &str,
) -> Result<Option<(String, usize, usize)>, Skip> {
    if classify(body) != Dialect::Legacy {
        return Ok(None);
    }
    let vars = read_fence(body).map_err(Skip::Vars)?;
    if vars.is_empty() {
        return Ok(None);
    }
    check_vars(&vars).map_err(Skip::Vars)?;
    let mut yaml = emit_body(&vars).map_err(Skip::Unrepresentable)?;
    if crlf_opening(content, range.start) {
        yaml = yaml.replace('\n', "\r\n");
    }
    // After the CRLF swap: the reader bounds what is on disk.
    if yaml.len() > MAX_BODY_BYTES {
        return Err(Skip::Vars(crate::vks::limit_error(
            Limit::BodyBytes,
            MAX_BODY_BYTES,
            0,
        )));
    }
    let nonblank = js_lines(body).filter(|l| !js_trim(l).is_empty()).count();
    Ok(Some((yaml, vars.len(), nonblank)))
}

/// True when the fence's opening line ends `\r\n` (the two bytes before the body).
fn crlf_opening(content: &str, start: usize) -> bool {
    start
        .checked_sub(2)
        .and_then(|a| content.get(a..start))
        .is_some_and(|s| s == "\r\n")
}

/// One side of the guard: parsed once, with the ranges and the flag derived from that parse.
struct Side<'a> {
    content: &'a str,
    parsed: ParsedArtifact,
    flagged: bool,
    ranges: Vec<Range<usize>>,
}

impl<'a> Side<'a> {
    fn new(content: &'a str, path: &str) -> Self {
        let parsed = parse_from_content(content, path);
        let flagged = is_flagged(content);
        let ranges = vks_body_ranges_of(content, &parsed, flagged);
        Self {
            content,
            parsed,
            flagged,
            ranges,
        }
    }

    /// The parse with the code of migrated bodies cleared. Flagged files compare unmasked: a
    /// fence inside a flag region is payload (§7.3).
    fn masked(mut self) -> ParsedArtifact {
        if self.flagged {
            return self.parsed;
        }
        let (c, ranges) = (self.content, &self.ranges);
        if top_code_range(c).is_some_and(|r| has(ranges, &r)) {
            self.parsed.code.clear();
        }
        for (b, (_, r)) in self.parsed.blocks.iter_mut().zip(block_code_ranges(c)) {
            if has(ranges, &r) {
                b.code.clear();
            }
        }
        self.parsed
    }
}

/// The rewrite must parse to the same artifact, bar the migrated bodies themselves (RISK 4).
fn guard(before: Side, out: &str, path: &str) -> Result<(), Skip> {
    if before.masked() == Side::new(out, path).masked() {
        Ok(())
    } else {
        Err(Skip::RoundTrip { field: "vars" })
    }
}

/// `ranges` (sorted by `vks_body_ranges`) holds `r`; a binary search keeps the mask linear-ish.
fn has(ranges: &[Range<usize>], r: &Range<usize>) -> bool {
    ranges
        .binary_search_by_key(&(r.start, r.end), |x| (x.start, x.end))
        .is_ok()
}
