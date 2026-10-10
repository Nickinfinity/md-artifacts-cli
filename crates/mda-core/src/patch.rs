//! Byte-surgical edits of an artifact file: one frontmatter field, or one block's code. Every other
//! byte (vks bodies included, W-3) is left as it is; each result is re-parsed and checked before it
//! is returned. Ports `artifact-patcher.service.ts:35-210` (extension @ `b368c20`) without
//! `patchVarDefaults` (D-10).

use std::ops::Range;

use crate::error::Refusal;
use crate::parse::text::js_trim_end;
use crate::parse::{
    block_code_ranges, body_range, is_flagged, key_of, parse_from_content, top_code_range,
};
use crate::registry::ArtifactType;
use crate::serialize::{FRONTMATTER_KEY_ORDER, single_line};

/// The frontmatter fields a patch may set.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::patch::FmField::Title.as_str(), "title");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmField {
    Title,
    Description,
}

impl FmField {
    /// The frontmatter key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Description => "description",
        }
    }
}

/// Which code a [`patch_code`] replaces: the single body, or block `index` whose heading must match.
///
/// # Examples
///
/// ```
/// use mda_core::patch::CodeTarget;
/// let t = CodeTarget::Block { index: 1, heading: "Two".into() };
/// assert_ne!(t, CodeTarget::Single);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeTarget {
    Single,
    Block { index: usize, heading: String },
}

/// Why a patch was refused.
///
/// # Examples
///
/// ```
/// use mda_core::{error::Refusal, patch::PatchError};
/// assert_eq!(PatchError::Refused(Refusal::Flagged), PatchError::Refused(Refusal::Flagged));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    /// The file's shape forbids this edit (`artifact.not_serializable`).
    Refused(Refusal),
    /// No block at that index with that heading (`None`: a single-body edit on a multi-block file).
    BlockNotFound(Option<usize>),
    /// The edited file would not re-parse as intended (`artifact.unrepresentable`); the field name.
    Unrepresentable(&'static str),
}

/// Set (or, with `""`, remove) one frontmatter field of `content`; `path` is vault-relative.
///
/// # Examples
///
/// ```
/// use mda_core::patch::{patch_field, FmField};
/// let out = patch_field("---\ntitle: a\n---\n", "Snippets/a.md", FmField::Title, "b").unwrap();
/// assert_eq!(out, "---\ntitle: b\n---\n");
/// ```
pub fn patch_field(
    content: &str,
    path: &str,
    field: FmField,
    value: &str,
) -> Result<String, PatchError> {
    if is_flagged(content) {
        return Err(PatchError::Refused(Refusal::Flagged));
    }
    let bad = PatchError::Unrepresentable(field.as_str());
    let range = body_range(content).ok_or(PatchError::Refused(Refusal::NoFrontmatter))?;
    let v = single_line(value);
    let eol = if content.starts_with("---\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let body = content.get(range.clone()).ok_or(bad.clone())?;
    let out =
        splice(content, &range, &edit_lines(body, field.as_str(), &v, eol)).ok_or(bad.clone())?;

    // Guard: only the field may differ, in the frontmatter and nowhere else.
    let mut want = parse_from_content(content, path);
    let slot = match field {
        FmField::Title => &mut want.frontmatter.title,
        FmField::Description => &mut want.frontmatter.description,
    };
    *slot = (!v.is_empty()).then_some(v);
    if parse_from_content(&out, path) == want {
        Ok(out)
    } else {
        Err(bad)
    }
}

/// `content` with `r` replaced by `with`; `None` when `r` is not on char boundaries.
fn splice(content: &str, r: &Range<usize>, with: &str) -> Option<String> {
    Some(format!(
        "{}{with}{}",
        content.get(..r.start)?,
        content.get(r.end..)?
    ))
}

/// The text of a segment from `split_inclusive('\n')`, terminator removed.
fn text_of(seg: &str) -> &str {
    seg.trim_end_matches(['\n', '\r'])
}

/// The frontmatter `body` with `key` set to `v` (non-empty) or removed (empty), line by line so
/// every other byte survives. The parser reads the LAST duplicate, so that one is replaced.
fn edit_lines(body: &str, key: &str, v: &str, eol: &str) -> String {
    let mut lines: Vec<String> = body.split_inclusive('\n').map(str::to_owned).collect();
    let is_key = |l: &String| key_of(text_of(l)) == Some(key);
    if v.is_empty() {
        let last_hit = lines.last().is_some_and(is_key);
        lines.retain(|l| !is_key(l));
        let mut out = lines.concat();
        if last_hit && out.ends_with('\n') {
            // The closing dashes' own terminator follows the body: don't leave a blank line.
            out.pop();
            if out.ends_with('\r') {
                out.pop();
            }
        }
        return out;
    }
    if let Some(i) = lines.iter().rposition(is_key) {
        if let Some(l) = lines.get_mut(i) {
            let term = l.get(text_of(l).len()..).unwrap_or("").to_owned();
            *l = format!("{key}: {v}{term}");
        }
        return lines.concat();
    }
    let rank = |k: &str| FRONTMATTER_KEY_ORDER.iter().position(|o| *o == k);
    let mine = rank(key);
    let at = lines
        .iter()
        .position(|l| {
            key_of(text_of(l)).and_then(rank) > mine && key_of(text_of(l)).and_then(rank).is_some()
        })
        .unwrap_or(lines.len());
    if at == lines.len() {
        if let Some(last) = lines.last_mut().filter(|l| !l.ends_with('\n')) {
            last.push_str(eol);
        }
        lines.push(format!("{key}: {v}"));
    } else {
        lines.insert(at, format!("{key}: {v}{eol}"));
    }
    lines.concat()
}

/// True when the fence whose code starts at `r.start` has the info string `vks` (variables, not code).
fn is_vks_fence(content: &str, r: &Range<usize>) -> bool {
    content
        .get(..r.start)
        .is_some_and(|h| h.trim_end_matches(['\r', '\n']).ends_with("```vks"))
}

/// Replace the code of `target` in `content`; `path` is vault-relative.
///
/// # Examples
///
/// ```
/// use mda_core::patch::{patch_code, CodeTarget};
/// let out = patch_code("```js\nx\n```\n", "Snippets/a.md", &CodeTarget::Single, "y").unwrap();
/// assert_eq!(out, "```js\ny\n```\n");
/// ```
pub fn patch_code(
    content: &str,
    path: &str,
    target: &CodeTarget,
    code: &str,
) -> Result<String, PatchError> {
    if is_flagged(content) {
        return Err(PatchError::Refused(Refusal::Flagged));
    }
    let before = parse_from_content(content, path);
    if before.frontmatter.artifact_type == ArtifactType::Variables {
        return Err(PatchError::Refused(Refusal::Variables));
    }
    let bad = PatchError::Unrepresentable("code");
    let (range, index) = match target {
        CodeTarget::Single => {
            if !before.blocks.is_empty() {
                return Err(PatchError::BlockNotFound(None));
            }
            (top_code_range(content), None)
        }
        CodeTarget::Block { index, heading } => {
            let hit = block_code_ranges(content).into_iter().nth(*index);
            let hit = hit.filter(|(h, _)| h == heading);
            (
                Some(hit.ok_or(PatchError::BlockNotFound(Some(*index)))?.1),
                Some(*index),
            )
        }
    };
    let range = range
        .filter(|r| !is_vks_fence(content, r))
        .ok_or(PatchError::Refused(Refusal::NoFence))?;
    let new_code = js_trim_end(code);
    let out = splice(content, &range, &format!("{new_code}\n")).ok_or(bad.clone())?;

    let after = parse_from_content(&out, path);
    let ok = match index {
        None => {
            let mut want = before;
            want.code = new_code.to_owned();
            after == want
        }
        Some(i) => block_guard(&before, &after, i, new_code),
    };
    if ok { Ok(out) } else { Err(bad) }
}

/// Block patch guard: frontmatter, file-level vars and every other block unchanged; the target keeps
/// its heading, description and fence language and gets exactly the new code (its detected vars
/// legitimately follow the code).
fn block_guard(
    before: &crate::model::ParsedArtifact,
    after: &crate::model::ParsedArtifact,
    i: usize,
    code: &str,
) -> bool {
    let same_shell = before.frontmatter == after.frontmatter
        && before.vars == after.vars
        && before.vars_error == after.vars_error
        && before.blocks.len() == after.blocks.len();
    let (Some(b), Some(a)) = (before.blocks.get(i), after.blocks.get(i)) else {
        return false;
    };
    let others = before
        .blocks
        .iter()
        .zip(&after.blocks)
        .enumerate()
        .all(|(n, (x, y))| n == i || x == y);
    same_shell
        && others
        && (
            a.heading.as_str(),
            a.description.as_str(),
            &a.fence_lang,
            a.code.as_str(),
        ) == (
            b.heading.as_str(),
            b.description.as_str(),
            &b.fence_lang,
            code,
        )
}
