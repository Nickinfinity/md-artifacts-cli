//! The template-index links that point at a vault path (spec §14.3). `artifact.rename` reports them as
//! `rename.index_link` warnings; with `updateLinks` it rewrites each linking index's link targets
//! instead (D-34…D-40), file by file, after the move and after the whole scan.

use std::collections::HashSet;
use std::path::Path;

use mda_core::multi_index::{is_index, plan_links, rewrite_links};
use mda_core::parse::{decode, parse_from_content};
use mda_core::patch::patch_index_body;
use mda_core::registry::type_for_dir;
use mda_vault::{Root, VaultError, contain, content_hash, replace, walk_md};

use crate::OpError;
use crate::artifact::MAX_ARTIFACT_BYTES;
use crate::artifact_rename::UpdatedIndex;
use crate::error::{
    RENAME_INDEX_LINK, RENAME_LINK_UPDATE_FAILED, RENAME_LINKS_UNCHECKED, vault_error,
};

/// One linking index found by the scan: its bytes as read (the write's hash check), the distinct raw
/// links to the old path, and how many link steps match (undeduped: it sizes the rewrite).
struct Hit {
    rel: String,
    bytes: Vec<u8>,
    links: Vec<String>,
    matches: usize,
}

/// What the walk found; `unchecked` is the `rename.links_unchecked` warning of a walk that stopped early.
struct Scan {
    hits: Vec<Hit>,
    unchecked: Option<OpError>,
}

/// One `rename.index_link` per (index, link) under `old`'s type directory that resolves to `old`;
/// a walk that stops early appends one `rename.links_unchecked`.
pub(crate) fn index_links_to(root: &Root, old: &str) -> Vec<OpError> {
    let Some((dir, _)) = old.split_once('/') else {
        return vec![];
    };
    warnings(collect(root, old, dir, |visit| {
        walk_md(root, &[dir], MAX_ARTIFACT_BYTES, visit)
    }))
}

/// With `updateLinks`: rewrite the links to `old` in every linking index (spec §14.3). Returns the warnings
/// and the indexes updated. The rename has already happened; nothing here undoes it. The scan finishes
/// before the first write.
pub(crate) fn update_index_links(
    root: &Root,
    old: &str,
    new: &str,
) -> (Vec<OpError>, Vec<UpdatedIndex>) {
    let Some((dir, _)) = old.split_once('/') else {
        return (vec![], vec![]);
    };
    let scan = collect(root, old, dir, |visit| {
        walk_md(root, &[dir], MAX_ARTIFACT_BYTES, visit)
    });
    apply(root, old, new, scan)
}

/// A hit's `rename.index_link` warnings.
fn hit_warnings(hit: &Hit) -> impl Iterator<Item = OpError> + '_ {
    hit.links.iter().map(|raw| {
        OpError::new(RENAME_INDEX_LINK)
            .with("index", &hit.rel)
            .with("link", raw)
    })
}

fn warnings(scan: Scan) -> Vec<OpError> {
    let mut out: Vec<_> = scan.hits.iter().flat_map(hit_warnings).collect();
    out.extend(scan.unchecked);
    out
}

/// Rewrite every hit; a failed one stays byte-identical and warns `rename.link_update_failed` then its
/// `rename.index_link` rows. `rename.links_unchecked` stays last (D-35).
fn apply(root: &Root, old: &str, new: &str, scan: Scan) -> (Vec<OpError>, Vec<UpdatedIndex>) {
    let (mut out, mut done) = (Vec::new(), Vec::new());
    // The moved file is skipped by canonical identity (right on case-insensitive and -sensitive
    // filesystems alike). A `contain` error means "not the moved file": `rewrite_one`'s own pin
    // then refuses a path it cannot contain, and a vanished `new` cannot match anything.
    let moved = contain(root, Path::new(new)).ok();
    for hit in &scan.hits {
        if moved.is_some() && contain(root, Path::new(&hit.rel)).ok() == moved {
            continue;
        }
        match rewrite_one(root, hit, old, new) {
            Ok(row) => done.push(row),
            Err(reason) => {
                out.push(
                    OpError::new(RENAME_LINK_UPDATE_FAILED)
                        .with("index", &hit.rel)
                        .with("reason", reason),
                );
                out.extend(hit_warnings(hit));
            }
        }
    }
    out.extend(scan.unchecked);
    (out, done)
}

/// `s` without the ASCII-case-insensitive prefix `p` (`.get()`: never panics on a char boundary).
/// Only `old` is matched this way, as the scan does; `new` is compared exactly.
fn strip_prefix_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    let head = s.get(..p.len())?;
    head.eq_ignore_ascii_case(p)
        .then(|| s.get(p.len()..))
        .flatten()
}

/// Rewrite one index in place; the `Err` is the D-36 `reason`.
fn rewrite_one(root: &Root, hit: &Hit, old: &str, new: &str) -> Result<UpdatedIndex, &'static str> {
    let prefix = format!("{}/", hit.rel.rsplit_once('/').map_or("", |(d, _)| d));
    let (Some(from_rel), Some(to_rel)) = (strip_prefix_ci(old, &prefix), new.strip_prefix(&prefix))
    else {
        return Err("unreachable");
    };
    // Raw bytes, never the lossy decode: a replacement char would be written back.
    let text = std::str::from_utf8(&hit.bytes).map_err(|_| "not_patchable")?;
    // `replace` bounds the current file, not the new text. ponytail: overestimates (the removed
    // spans are ignored), so a file just under the cap can be refused — exact sizing if it ever bites.
    let grown = hit
        .bytes
        .len()
        .saturating_add(hit.matches.saturating_mul(to_rel.len()));
    if grown as u64 > MAX_ARTIFACT_BYTES {
        return Err("not_patchable");
    }
    let (bom, body) = match text.strip_prefix('\u{feff}') {
        Some(rest) => ("\u{feff}", rest),
        None => ("", text),
    };
    let code = parse_from_content(body, &hit.rel).code;
    let new_code = rewrite_links(&code, from_rel, to_rel).map_err(|_| "unlinkable")?;
    let patched = patch_index_body(body, &hit.rel, &new_code).map_err(|_| "not_patchable")?;
    let out = format!("{bom}{patched}");
    // Pin: an in-vault symlink to another type dir must not be written through.
    let dir = hit.rel.split('/').next().unwrap_or_default();
    match (
        contain(root, Path::new(dir)),
        contain(root, Path::new(&hit.rel)),
    ) {
        (Ok(d), Ok(f)) if f.starts_with(&d) => {}
        _ => return Err("not_patchable"),
    }
    replace(
        root,
        Path::new(&hit.rel),
        out.as_bytes(),
        &content_hash(&hit.bytes),
        MAX_ARTIFACT_BYTES,
    )
    .map_err(|e| match e {
        VaultError::Conflict { .. } | VaultError::NotFound { .. } => "conflict",
        _ => "write_failed",
    })?;
    Ok(UpdatedIndex {
        path: hit.rel.clone(),
        hash: content_hash(out.as_bytes()),
    })
}

/// The scan over any walker (the seam lets a test inject a walk error after some files).
fn collect(
    root: &Root,
    old: &str,
    dir: &str,
    walker: impl FnOnce(&mut dyn FnMut(&str, Result<Vec<u8>, VaultError>)) -> Result<(), VaultError>,
) -> Scan {
    let mut scan = Scan {
        hits: Vec::new(),
        unchecked: None,
    };
    // Only whole-file types can be indexes.
    if !type_for_dir(dir).is_some_and(|t| t.info().writes_file) {
        return scan;
    }
    let mut visit = |rel: &str, res: Result<Vec<u8>, VaultError>| {
        // Unreadable or oversize files are skipped; the renamed file is never its own warning.
        let Ok(bytes) = res else { return };
        if rel.eq_ignore_ascii_case(old) {
            return;
        }
        let file = parse_from_content(&decode(&bytes), rel);
        if !is_index(&file.frontmatter) {
            return;
        }
        let idx_dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
        let (mut seen, mut links, mut matches) = (HashSet::new(), Vec::new(), 0);
        for step in plan_links(&file.code).steps {
            // The joined vault path, not `rel_path` alone: links resolve from the index's dir.
            let target = format!("{idx_dir}/{}", step.rel_path);
            if target.eq_ignore_ascii_case(old) {
                matches += 1;
                if seen.insert(step.raw.clone()) {
                    links.push(step.raw);
                }
            }
        }
        if matches > 0 {
            scan.hits.push(Hit {
                rel: rel.to_owned(),
                bytes,
                links,
                matches,
            });
        }
    };
    if let Err(e) = walker(&mut visit) {
        let mut w = OpError::new(RENAME_LINKS_UNCHECKED);
        if matches!(e, VaultError::Limit { .. }) {
            // Same params as `walk.limit`: one mapping only.
            w.params = vault_error(e, root).params;
        }
        scan.unchecked = Some(w);
    }
    scan
}

#[cfg(test)]
mod tests;
