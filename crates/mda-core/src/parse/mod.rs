//! `.md` parsing: frontmatter, code fences, `##` blocks, `<VK-…>` detection, flags, payload
//! selection. Ports `parser.service.ts` (extension @ `b368c20`).

mod blocks;
mod fence;
pub mod flags;
mod frontmatter;
pub mod text;
pub mod tokens;

use std::collections::{HashMap, HashSet};

pub use fence::{DEFAULTS_INFO, untagged_defaults};
pub use text::decode;

pub(crate) use blocks::{block_code_ranges, block_vks_ranges, fenceless_headings};
pub(crate) use frontmatter::{body_range, key_of};

use crate::error::{Limit, VKS_LIMIT, VarsError};
use crate::model::{Layout, ParsedArtifact, ParsedBlock, ParsedVar, PayloadKind};
use crate::registry::{ArtifactType, type_for_dir};
use crate::vks::{MAX_BLOCK_DEFAULTS, VksValue};
use flags::FlagScan;
use text::js_trim;

/// Fence language reported for a flag-delimited or bare-body payload (the note's own markdown).
pub(crate) const PAYLOAD_LANG: &str = "markdown";

/// True when the frontmatter-stripped body holds at least one flagged region (spec §7). The
/// engine never rewrites such a file: text outside the flags would be lost (W-7).
///
/// # Examples
///
/// ```
/// assert!(!mda_core::parse::is_flagged("---\nartifactType: Snippet\n---\n```js\nx\n```\n"));
/// ```
pub fn is_flagged(content: &str) -> bool {
    !flags::extract_flagged_regions(frontmatter::strip(content)).is_empty()
}

/// The frontmatter alone (no payload: `language` is not filled from a fence) — the migration guard
/// compares it (spec §11.1).
pub(crate) fn frontmatter_of(content: &str) -> crate::model::Frontmatter {
    frontmatter::parse(content, ArtifactType::Snippet)
}

/// Byte ranges (sorted, deduped) of every vks body the parser reads: the first `vks` fence of the
/// unstripped content, plus — only when the file is unflagged and parses to ≥ 1 block — the first
/// `vks` fence of each parsed `##` section (spec §11.2).
#[cfg(test)] // reason: production callers hold a parse already and use `vks_body_ranges_of`
pub(crate) fn vks_body_ranges(content: &str, path: &str) -> Vec<std::ops::Range<usize>> {
    let p = parse_from_content(content, path);
    vks_body_ranges_of(content, &p, is_flagged(content))
}

/// `vks_body_ranges` (test-only) for a caller that already parsed `content` and knows whether it is flagged
/// (the migration guard reuses both: a parse of a 1 MiB file is the cost it bounds).
pub(crate) fn vks_body_ranges_of(
    content: &str,
    parsed: &ParsedArtifact,
    flagged: bool,
) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<_> = fence::vks_range(content).into_iter().collect();
    if !flagged && !parsed.blocks.is_empty() {
        out.extend(block_vks_ranges(content));
    }
    out.sort_by_key(|r| (r.start, r.end));
    out.dedup();
    out
}

/// Index of the parsed block whose `vks` fence is also the file-level fence (its `varsError` is the
/// file's): `vault.validate` reports that error once, other blocks' errors always (spec §11.3).
///
/// # Examples
///
/// ```
/// let c = "## A\n```yaml VKs\nVK-a: 1\n```\n## B\n```yaml VKs\nVK-b: 2\n```\n";
/// assert_eq!(mda_core::parse::file_fence_block(c), Some(0));
/// assert_eq!(mda_core::parse::file_fence_block("no fence"), None);
/// ```
pub fn file_fence_block(content: &str) -> Option<usize> {
    blocks::file_fence_block(content)
}

/// The unfenced `vars:` section `parse_vars` reads: `None` when a `vks` fence exists or no section
/// matches.
pub(crate) fn unfenced_vars(content: &str) -> Option<Result<Vec<ParsedVar>, VarsError>> {
    fence::unfenced_vars(content)
}

/// Absolute byte range of the code of the file's first fence (the single-body payload).
pub(crate) fn top_code_range(content: &str) -> Option<std::ops::Range<usize>> {
    let body = frontmatter::strip(content);
    let base = content.len() - body.len(); // `strip` returns a suffix of `content`
    let r = fence::code_range(body)?;
    Some(base + r.start..base + r.end)
}

/// The payload fields a file's body yields, however it is delimited.
struct Payload {
    code: String,
    fence_lang: Option<String>,
    vars: Vec<ParsedVar>,
    blocks: Vec<ParsedBlock>,
    vars_error: Option<VarsError>,
    layout: Layout,
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
    let scan = flags::scan_flags(frontmatter::strip(content));
    let payload = if scan.regions.is_empty() {
        unflagged(content, fm.artifact_type)
    } else {
        flagged(scan, content)
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
        layout: payload.layout,
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
        layout: Layout::default(),
    }
}

/// A flag-less file: the fenced payload, or, for a whole-file type with no content fence, the bare
/// body minus its `vars:` section. A `vks` fence is the defaults section, not content.
fn unflagged(content: &str, ty: ArtifactType) -> Payload {
    let fenced = fenced(content);
    let has_content_fence =
        !fenced.code.is_empty() && fenced.fence_lang.as_deref() != Some(DEFAULTS_INFO);
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
        layout: Layout {
            payload: PayloadKind::Bare,
            ..Layout::default()
        },
    }
}

/// A flag-delimited payload: region content is the artifact, verbatim. One region is a single-block
/// file; two or more become blocks. Vars are detected per region and overlaid with the file's
/// defaults; a defaults error goes on the artifact only (blocks keep `""` defaults, no error).
fn flagged(scan: FlagScan, content: &str) -> Payload {
    let regions = &scan.regions;
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
        layout: Layout {
            payload: PayloadKind::Flags,
            regions: scan.layouts,
            notes: scan.notes,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{VKS_SYNTAX, VarsError};

    fn scan(regions: &[(&str, &str)]) -> FlagScan {
        FlagScan {
            regions: regions
                .iter()
                .map(|(n, c)| flags::FlaggedRegion {
                    name: (*n).into(),
                    content: (*c).into(),
                })
                .collect(),
            ..FlagScan::default()
        }
    }

    fn spans<'a>(c: &'a str, path: &str) -> Vec<&'a str> {
        let r = vks_body_ranges(c, path);
        r.into_iter().filter_map(|r| c.get(r)).collect()
    }

    #[test]
    fn ranges_are_what_the_parser_reads() {
        let v = "Variables/v.md";
        let flat = "---\nartifactType: Variables\n---\n\n```yaml VKs\nVK-a=1\n```\n";
        assert_eq!(
            vks_body_ranges(flat, v),
            vec![top_code_range(flat).unwrap()]
        );
        let sets = "## D\n```yaml VKs\nVK-a=1\n```\n## P\n```yaml VKs\nVK-a=2\n```\n";
        assert_eq!(spans(sets, v), ["VK-a=1\n", "VK-a=2\n"]);
        let trail = "## A\n```sh\nx\n```\n```yaml VKs\nVK-a=1\n```\n```yaml VKs\nVK-b=2\n```\n";
        assert_eq!(spans(trail, "Snippets/t.md"), ["VK-a=1\n"]);
        let late = "```yaml VKs\nVK-t=1\n```\n## \n```yaml VKs\nVK-a=1\n```\n";
        assert_eq!(spans(late, v), ["VK-t=1\n"]);
        let (a, z) = (format!("%%oa:{}%%", "start"), format!("%%oa:{}%%", "end"));
        let flagged = format!(
            "{a}\nx\n{z}\n## S\n```yaml VKs\nVK-a=1\n```\n## T\n```yaml VKs\nVK-b=1\n```\n"
        );
        assert_eq!(spans(&flagged, "AIPrompts/f.md"), ["VK-a=1\n"]);
        let bare = "Intro\n```yaml VKs\nA=1\n```\n## S\n```sh\nx\n```\n";
        assert_eq!(spans(bare, "AIAgentsConf/b.md"), ["A=1\n"]);
    }

    #[test]
    fn unfenced_vars_reads_only_without_a_vks_fence() {
        assert!(unfenced_vars("```sh\nx\n```\n\nvars:\nVK-a=1\n").is_some());
        assert!(unfenced_vars("```yaml VKs\nVK-a=1\n```\nvars:\nVK-b=1\n").is_none());
        assert!(unfenced_vars("```sh\nx\n```\n").is_none());
    }

    #[test]
    fn one_flagged_region_is_a_single_block_file() {
        let p = flagged(scan(&[("r", "Hi <VK-a>")]), "");
        assert_eq!(p.code, "Hi <VK-a>");
        assert_eq!(p.fence_lang.as_deref(), Some("markdown"));
        assert!(p.blocks.is_empty());
        assert_eq!(p.vars, vec![ParsedVar::text("VK-a", "")]);
    }

    #[test]
    fn two_flagged_regions_become_blocks_with_file_defaults_overlaid() {
        let content = "```yaml VKs\nVK-a=1\nVK-b=2\n```\n";
        let p = flagged(scan(&[("one", "x <VK-a>"), ("two", "y <VK-c>")]), content);
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
