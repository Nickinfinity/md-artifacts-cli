//! Template indexes (spec §8): `safe_rel_path` — the one rejection authority for vault-authored
//! relative paths — and index link extraction, link plans and destination candidates.

use crate::error::{
    PATH_ABSOLUTE, PATH_BACKSLASH, PATH_CONTROL_CHAR, PATH_DRIVE_OR_SCHEME, PATH_EMPTY,
    PATH_PARENT_SEGMENT,
};
use crate::model::Frontmatter;
use crate::parse::text::{compile, js_trim, js_trim_end};
use regex::Regex;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::LazyLock;

/// Wikilink `[[t]]` or markdown link `[x](t)`; targets exclude brackets/parens/newlines so the scan
/// stays linear (spec §8.2). The `!` embed/image guard is done by hand: `regex` has no lookbehind.
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\[\[([^\[\]\n]+)\]\]|\[[^\[\]\n]*\]\(([^()\n]+)\)"));

/// Why `safe_rel_path` rejected a vault-authored path (spec §8.3, checked in this order).
///
/// # Examples
///
/// ```
/// use mda_core::multi_index::RelPathError as E;
/// assert_eq!(E::Empty.code(), "path.empty");
/// assert_eq!(E::ControlChar.code(), "path.control_char");
/// assert_eq!(E::DriveOrScheme.code(), "path.drive_or_scheme");
/// assert_eq!(E::Backslash.code(), "path.backslash");
/// assert_eq!(E::Absolute.code(), "path.absolute");
/// assert_eq!(E::ParentSegment.code(), "path.parent_segment");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelPathError {
    Empty,
    ControlChar,
    DriveOrScheme,
    Backslash,
    Absolute,
    ParentSegment,
}

impl RelPathError {
    /// The `path.*` code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Empty => PATH_EMPTY,
            Self::ControlChar => PATH_CONTROL_CHAR,
            Self::DriveOrScheme => PATH_DRIVE_OR_SCHEME,
            Self::Backslash => PATH_BACKSLASH,
            Self::Absolute => PATH_ABSOLUTE,
            Self::ParentSegment => PATH_PARENT_SEGMENT,
        }
    }
}

/// One accepted link of an index: the raw target, its normalised `.md` path and that path's directory.
///
/// # Examples
///
/// ```
/// use mda_core::multi_index::LinkStep;
/// let s = LinkStep { raw: "a".into(), rel_path: "a.md".into(), rel_dir: String::new() };
/// assert!(s.rel_dir.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStep {
    pub raw: String,
    pub rel_path: String,
    pub rel_dir: String,
}

/// An index body's links: accepted steps in document order, and rejected raw targets with why.
///
/// # Examples
///
/// ```
/// assert!(mda_core::multi_index::LinkPlan::default().steps.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkPlan {
    pub steps: Vec<LinkStep>,
    pub rejected: Vec<(String, RelPathError)>,
}

/// Validate and normalise a vault-authored relative path (reject, never sanitise).
///
/// # Examples
///
/// ```
/// let _ = mda_core::multi_index::safe_rel_path("a/b.md");
/// ```
pub fn safe_rel_path(raw: &str) -> Result<String, RelPathError> {
    // Order is the contract (R-20). ponytail: no decoding or Unicode folding; the containment
    // re-check at the read/write decides.
    if raw.is_empty() {
        return Err(RelPathError::Empty);
    }
    // C0 and C1 controls, as TS CONTROL_CHAR_RE.
    if raw
        .chars()
        .any(|c| matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}'))
    {
        return Err(RelPathError::ControlChar);
    }
    if raw.contains(':') {
        return Err(RelPathError::DriveOrScheme);
    }
    if raw.contains('\\') {
        return Err(RelPathError::Backslash);
    }
    if raw.starts_with('/') {
        return Err(RelPathError::Absolute);
    }
    let segs: Vec<&str> = raw
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if segs.contains(&"..") {
        return Err(RelPathError::ParentSegment);
    }
    if segs.is_empty() {
        return Err(RelPathError::Empty);
    }
    Ok(segs.join("/"))
}

/// `safe_rel_path`, then `.md` appended unless already there (ASCII case-insensitive).
///
/// # Examples
///
/// ```
/// let _ = mda_core::multi_index::resolve_link("a");
/// ```
pub fn resolve_link(raw: &str) -> Result<String, RelPathError> {
    let mut p = safe_rel_path(raw)?;
    if strip_md(&p).len() == p.len() {
        p.push_str(".md");
    }
    Ok(p)
}

/// Every link target in an index body, in document order (wikilinks and markdown links; embeds skipped).
///
/// # Examples
///
/// ```
/// let _ = mda_core::multi_index::extract_links("[[a]]");
/// ```
pub fn extract_links(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk_links(body, |raw, _| out.push(raw.to_owned()));
    out
}

/// The one link scanner: calls `f(target, byte span of the target in body)` per link, in order.
/// The span is what [`rewrite_links`] replaces; for a wikilink it excludes anchor and alias.
fn walk_links(body: &str, mut f: impl FnMut(&str, Range<usize>)) {
    let mut at = 0;
    while let Some(c) = LINK_RE.captures_at(body, at) {
        let Some(m) = c.get(0) else { break };
        // JS `(?<!!)`: skip an embed/image but rescan from the next char, so `![T]([[x]])` keeps `[[x]]`.
        if body.as_bytes().get(m.start().wrapping_sub(1)) == Some(&b'!') && m.start() > 0 {
            at = m.start() + 1;
            continue;
        }
        at = m.end();
        let (g, wiki) = match (c.get(1), c.get(2)) {
            (Some(w), _) => (w, true),
            (None, Some(l)) => (l, false),
            _ => continue,
        };
        let s = g.as_str();
        let cut = if wiki { strip_wiki_raw(s) } else { s };
        let lead = js_trim_end(cut).len() - js_trim(cut).len();
        let start = g.start() + lead;
        f(js_trim(cut), start..start + js_trim(cut).len());
    }
}

/// Why [`rewrite_links`] refused: the new target cannot be spelled as a link.
///
/// # Examples
///
/// ```
/// use mda_core::multi_index::RewriteError;
/// assert_eq!(RewriteError::Unlinkable, RewriteError::Unlinkable);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteError {
    Unlinkable,
}

/// `body` with every link resolving to `from_rel` pointing at `to_rel` (both relative to the index's
/// directory); only the target text changes (spec §14.3).
///
/// # Examples
///
/// ```
/// let _ = mda_core::multi_index::rewrite_links("[[a]]", "a.md", "b.md");
/// ```
pub fn rewrite_links(body: &str, from_rel: &str, to_rel: &str) -> Result<String, RewriteError> {
    // (span, replacement) per matched link; `want` is the resolved path list the guard expects.
    let mut hits: Vec<(Range<usize>, String)> = Vec::new();
    let (mut want, mut rejected) = (Vec::new(), Vec::new());
    // The new path must be its own normal form, so the guard can compare it exactly.
    let to_ok = resolve_link(to_rel).is_ok_and(|p| p == to_rel);
    let mut bad_target = false;
    walk_links(body, |raw, span| {
        let Ok(rel) = resolve_link(raw) else {
            rejected.push(raw.to_owned());
            return;
        };
        if !rel.eq_ignore_ascii_case(from_rel) {
            want.push(rel);
            return;
        }
        bad_target |= !to_ok || to_rel.contains(['#', '|', '[', ']', '(', ')']);
        let stem = strip_md(to_rel);
        let had_md = safe_rel_path(raw).is_ok_and(|p| strip_md(&p).len() != p.len());
        let new = if had_md {
            format!("{stem}.md")
        } else {
            stem.to_owned()
        };
        want.push(to_rel.to_owned());
        hits.push((span, new));
    });
    if hits.is_empty() {
        return Ok(body.to_owned());
    }
    if bad_target {
        return Err(RewriteError::Unlinkable);
    }
    // One pass, gap by gap: linear however many links match.
    let mut out = String::with_capacity(body.len() + hits.len() * to_rel.len());
    let mut at = 0;
    for (span, new) in &hits {
        out.push_str(body.get(at..span.start).ok_or(RewriteError::Unlinkable)?);
        out.push_str(new);
        at = span.end;
    }
    out.push_str(body.get(at..).ok_or(RewriteError::Unlinkable)?);
    // Guard: the rewritten body must plan exactly the old links with the matched ones swapped.
    let plan = plan_links(&out);
    let got: Vec<&String> = plan.steps.iter().map(|s| &s.rel_path).collect();
    let rej: Vec<&String> = plan.rejected.iter().map(|(r, _)| r).collect();
    if got.iter().copied().eq(want.iter()) && rej.iter().copied().eq(rejected.iter()) {
        Ok(out)
    } else {
        Err(RewriteError::Unlinkable)
    }
}

/// `p` without a trailing `.md` (ASCII case-insensitive).
fn strip_md(p: &str) -> &str {
    match p
        .len()
        .checked_sub(3)
        .and_then(|i| p.get(i..).map(|e| (i, e)))
    {
        Some((i, e)) if e.eq_ignore_ascii_case(".md") => p.get(..i).unwrap_or(p),
        _ => p,
    }
}

/// The link plan of an index body: each target resolved, or rejected with its code.
///
/// # Examples
///
/// ```
/// let _ = mda_core::multi_index::plan_links("[[a]]");
/// ```
pub fn plan_links(body: &str) -> LinkPlan {
    let mut plan = LinkPlan::default();
    for raw in extract_links(body) {
        match resolve_link(&raw) {
            Ok(rel_path) => {
                let rel_dir = rel_dir_of(&rel_path).to_owned();
                plan.steps.push(LinkStep {
                    raw,
                    rel_path,
                    rel_dir,
                });
            }
            Err(e) => plan.rejected.push((raw, e)),
        }
    }
    plan
}

/// Destination folders offered for one step: the mirrored folder first, then accepted `paths:` entries,
/// deduplicated (first wins).
///
/// # Examples
///
/// ```
/// let _ = mda_core::multi_index::dest_candidates("", "sub", &[]);
/// ```
pub fn dest_candidates(mirror_from: &str, rel_dir: &str, paths: &[String]) -> Vec<String> {
    let mirrored = [mirror_from, rel_dir]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("/");
    let mut seen = HashSet::new();
    std::iter::once(mirrored)
        .chain(paths.iter().filter_map(|p| safe_rel_path(p).ok()))
        .filter(|c| seen.insert(c.clone()))
        .collect()
}

/// Whether frontmatter marks a template index: `index: true` on a whole-file type.
///
/// # Examples
///
/// ```
/// use mda_core::{model::Frontmatter, registry::ArtifactType};
/// let _ = mda_core::multi_index::is_index(&Frontmatter::new(ArtifactType::Template));
/// ```
pub fn is_index(fm: &Frontmatter) -> bool {
    fm.index == Some(true) && fm.artifact_type.info().writes_file
}

/// Text before the last `/`, `""` when none.
fn rel_dir_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

/// A wikilink's target text, untrimmed: cut at the first `|`, then at the first `#` before it.
fn strip_wiki_raw(raw: &str) -> &str {
    let no_alias = raw.split('|').next().unwrap_or("");
    no_alias.split('#').next().unwrap_or("")
}
