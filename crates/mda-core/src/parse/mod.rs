//! `.md` parsing: frontmatter, code fences, `##` blocks, `<VK-…>` detection, flags, payload
//! selection. Ports `parser.service.ts` (extension @ `b368c20`).

mod blocks;
mod fence;
pub mod flags;
mod frontmatter;
pub mod text;
pub mod tokens;

use std::collections::{HashMap, HashSet};

pub use text::decode;

use crate::error::{Limit, VKS_LIMIT, VarsError};
use crate::model::{ParsedArtifact, ParsedBlock, ParsedVar};
use crate::registry::{ArtifactType, type_for_dir};
use crate::vks::{MAX_BLOCK_DEFAULTS, VksValue};
use flags::FlaggedRegion;
use text::js_trim;

/// Fence language reported for a flag-delimited or bare-body payload (the note's own markdown).
const PAYLOAD_LANG: &str = "markdown";

/// The payload fields a file's body yields, however it is delimited.
struct Payload {
    code: String,
    fence_lang: Option<String>,
    vars: Vec<ParsedVar>,
    blocks: Vec<ParsedBlock>,
    vars_error: Option<VarsError>,
}

/// Parse one artifact file's decoded `content`; `path` is vault-relative POSIX
/// (`<TypeDir>/<rel>.md`) and decides the default type, `fileName` and `relativePath`.
///
/// # Examples
///
/// ```
/// let a = mda_core::parse::parse_from_content("", "Snippets/a.md");
/// assert_eq!(a.file_path, "Snippets/a.md");
/// assert_eq!(a.file_name, "a");
/// ```
pub fn parse_from_content(content: &str, path: &str) -> ParsedArtifact {
    let (dir, relative_path) = path.split_once('/').unwrap_or((path, ""));
    let default = type_for_dir(dir).unwrap_or(ArtifactType::Snippet);
    let mut fm = frontmatter::parse(content, default);

    // Flags win over fences; flag-less files take the classic path unchanged.
    let regions = flags::extract_flagged_regions(frontmatter::strip(content));
    let payload = if regions.is_empty() {
        unflagged(content, fm.artifact_type)
    } else {
        flagged(&regions, content)
    };
    if fm.language.as_deref().is_none_or(str::is_empty) && payload.fence_lang.is_some() {
        fm.language = payload.fence_lang;
    }
    ParsedArtifact {
        file_path: path.to_owned(),
        file_name: file_name(path),
        relative_path: relative_path.to_owned(),
        frontmatter: fm,
        code: payload.code,
        vars: payload.vars,
        blocks: payload.blocks,
        vars_error: payload.vars_error,
    }
}

/// Node `basename(p, '.md')`: a name that is exactly `.md` stays `.md`.
fn file_name(path: &str) -> String {
    let base = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    match base.strip_suffix(".md") {
        Some(stem) if !stem.is_empty() => stem,
        _ => base,
    }
    .to_owned()
}

/// Overlay `defaults` onto the detected vars, keeping code order: a default with a matching name
/// supplies the value (last duplicate wins); every default with no match is appended, duplicates
/// included. A rejected `vks` body leaves the detected vars at `""` and returns the error.
fn overlay(
    detected: Vec<ParsedVar>,
    defaults: Result<Vec<ParsedVar>, VarsError>,
) -> (Vec<ParsedVar>, Option<VarsError>) {
    let defaults = match defaults {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => return (detected, None),
        Err(e) => return (detected, Some(e)),
    };
    let by_name: HashMap<&str, &VksValue> = defaults
        .iter()
        .map(|d| (d.name.as_str(), &d.value))
        .collect();
    let mut merged: Vec<ParsedVar> = detected
        .iter()
        .map(|v| ParsedVar {
            name: v.name.clone(),
            value: by_name
                .get(v.name.as_str())
                .map_or_else(|| v.value.clone(), |&d| d.clone()),
        })
        .collect();
    let seen: HashSet<&str> = detected.iter().map(|v| v.name.as_str()).collect();
    let extras = defaults.iter().filter(|d| !seen.contains(d.name.as_str()));
    merged.extend(extras.cloned());
    (merged, None)
}

/// The classic fence payload: top-level vars are the file's defaults **only** (no token detection),
/// blocks are the `##` sections.
fn fenced(content: &str) -> Payload {
    let (code, fence_lang) = fence::code_block(frontmatter::strip(content)).unwrap_or_default();
    let (vars, vars_error) =
        fence::parse_vars(content).map_or_else(|e| (vec![], Some(e)), |v| (v, None));
    Payload {
        code,
        fence_lang,
        vars,
        blocks: blocks::parse_blocks(content),
        vars_error,
    }
}

/// A flag-less file: the fenced payload, or, for a whole-file type with no content fence, the bare
/// body minus its `vars:` section. A `vks` fence is the defaults section, not content.
fn unflagged(content: &str, ty: ArtifactType) -> Payload {
    let fenced = fenced(content);
    let has_content_fence = !fenced.code.is_empty() && fenced.fence_lang.as_deref() != Some("vks");
    if has_content_fence || !ty.info().writes_file {
        return fenced;
    }
    let body = fence::strip_vars_section(frontmatter::strip(content));
    let body = js_trim(&body);
    if body.is_empty() {
        return fenced;
    }
    let (vars, vars_error) = overlay(tokens::detect_vars(body), fence::parse_vars(content));
    Payload {
        code: body.to_owned(),
        fence_lang: Some(PAYLOAD_LANG.into()),
        vars,
        blocks: vec![],
        vars_error,
    }
}

/// A flag-delimited payload: region content is the artifact, verbatim. One region is a single-block
/// file; two or more become blocks. Vars are detected per region and overlaid with the file's
/// defaults; a defaults error goes on the artifact only (blocks keep `""` defaults, no error).
fn flagged(regions: &[FlaggedRegion], content: &str) -> Payload {
    let mut defaults = fence::parse_vars(content);
    // Every block gets every default (TS parity), so output is regions x defaults: refuse the
    // overlay past the cap, before it is built. Same path as a failed defaults fence.
    let n = defaults.as_ref().map_or(0, Vec::len);
    if regions.len() > 1 && regions.len().saturating_mul(n) > MAX_BLOCK_DEFAULTS {
        defaults = Err(VarsError::new(VKS_LIMIT, 0)
            .with("limit", Limit::BlockDefaults.as_str())
            .with("max", MAX_BLOCK_DEFAULTS.to_string()));
    }
    let vars_for = |code: &str| overlay(tokens::detect_vars(code), defaults.clone());
    let (code, vars, vars_error) = match regions.first() {
        Some(first) => {
            let (vars, err) = vars_for(&first.content);
            (first.content.clone(), vars, err)
        }
        None => (String::new(), vec![], None),
    };
    let blocks = if regions.len() > 1 {
        regions
            .iter()
            .map(|r| ParsedBlock {
                heading: r.name.clone(),
                description: String::new(),
                code: r.content.clone(),
                fence_lang: Some(PAYLOAD_LANG.into()),
                vars: vars_for(&r.content).0,
                vars_error: None,
            })
            .collect()
    } else {
        vec![]
    };
    Payload {
        code,
        fence_lang: Some(PAYLOAD_LANG.into()),
        vars,
        blocks,
        vars_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{VKS_SYNTAX, VarsError};

    fn region(name: &str, content: &str) -> FlaggedRegion {
        FlaggedRegion {
            name: name.into(),
            content: content.into(),
        }
    }

    #[test]
    fn one_flagged_region_is_a_single_block_file() {
        let p = flagged(&[region("r", "Hi <VK-a>")], "");
        assert_eq!(p.code, "Hi <VK-a>");
        assert_eq!(p.fence_lang.as_deref(), Some("markdown"));
        assert!(p.blocks.is_empty());
        assert_eq!(p.vars, vec![ParsedVar::text("VK-a", "")]);
    }

    #[test]
    fn two_flagged_regions_become_blocks_with_file_defaults_overlaid() {
        let content = "```vks\nVK-a=1\nVK-b=2\n```\n";
        let p = flagged(
            &[region("one", "x <VK-a>"), region("two", "y <VK-c>")],
            content,
        );
        assert_eq!(p.blocks.len(), 2);
        assert_eq!(p.blocks[0].heading, "one");
        assert_eq!(p.blocks[1].fence_lang.as_deref(), Some("markdown"));
        assert_eq!(
            p.blocks[0].vars,
            vec![ParsedVar::text("VK-a", "1"), ParsedVar::text("VK-b", "2")]
        );
        assert_eq!(
            p.blocks[1].vars,
            vec![
                ParsedVar::text("VK-c", ""),
                ParsedVar::text("VK-a", "1"),
                ParsedVar::text("VK-b", "2")
            ]
        );
        assert_eq!(p.code, "x <VK-a>");
    }

    #[test]
    fn overlay_error_keeps_detected_vars_and_returns_the_error() {
        let detected = vec![ParsedVar::text("VK-a", "")];
        let err = VarsError::new(VKS_SYNTAX, 2);
        let (vars, e) = overlay(detected.clone(), Err(err.clone()));
        assert_eq!(vars, detected);
        assert_eq!(e, Some(err));
    }

    #[test]
    fn overlay_last_duplicate_wins_and_unmatched_extras_repeat() {
        let t = ParsedVar::text;
        let d = vec![
            t("VK-a", "1"),
            t("VK-a", "2"),
            t("VK-z", "9"),
            t("VK-z", "8"),
        ];
        let (vars, e) = overlay(vec![t("VK-a", "")], Ok(d));
        assert_eq!(vars, vec![t("VK-a", "2"), t("VK-z", "9"), t("VK-z", "8")]);
        assert_eq!(e, None);
    }
}
