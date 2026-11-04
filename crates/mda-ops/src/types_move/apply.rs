//! `types.move.apply`'s copy → verify → switch → delete steps (D-9, D-18). Every step before the
//! switch undoes only this call's copies; after it nothing is undone.

use std::collections::BTreeMap;
use std::path::Path;

use mda_vault::{Root, VaultError, content_hash, create_new, delete, read_bounded};

use super::shared::Src;
use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, bad_path};
use crate::error::{
    FILE_CONFLICT, MOVE_OVERLAP, MOVE_PLAN_STALE, MOVE_SOURCE_KEPT, MOVE_TARGET_EXISTS,
    OP_BAD_REQUEST, vault_error_at,
};
use crate::location::Place;
use crate::migrate::{FileRef, MAX_APPLY_FILES};
use crate::{ArtifactType, OpError};

/// A target dir inside a root source in the same vault would be walked into itself.
pub(super) fn overlap(src: &Place, target: &Place) -> Result<(), OpError> {
    match (&src.base, &target.base) {
        (None, Some(d)) if src.root.path() == target.root.path() => {
            Err(OpError::new(MOVE_OVERLAP).with("dir", d.as_str()))
        }
        _ => Ok(()),
    }
}

/// Cap, duplicates, and every path virtual and of `t`, before any I/O.
pub(super) fn bounds(t: ArtifactType, files: &[FileRef]) -> Result<(), OpError> {
    let bad = |reason: &str| OpError::new(OP_BAD_REQUEST).with("reason", reason);
    if files.len() > MAX_APPLY_FILES {
        return Err(bad("at most 10000 files"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for f in files {
        if artifact_path(&f.path)? != t {
            return Err(bad_path(&f.path));
        }
        if !seen.insert(f.path.as_str()) {
            return Err(bad("duplicate path"));
        }
    }
    Ok(())
}

/// The re-walked set must be the planned set (`move.plan_stale` names the first difference in
/// sorted order), and every buffer must hash to what the plan read (`file.conflict`).
pub(super) fn matches_plan(files: &[Src], planned: &[FileRef]) -> Result<(), OpError> {
    let want: BTreeMap<&str, &str> = planned
        .iter()
        .map(|f| (f.path.as_str(), f.hash.as_str()))
        .collect();
    let have: BTreeMap<&str, &Src> = files.iter().map(|s| (s.virt.as_str(), s)).collect();
    let stale = want
        .keys()
        .filter(|k| !have.contains_key(**k))
        .chain(have.keys().filter(|k| !want.contains_key(**k)))
        .min();
    if let Some(p) = stale {
        return Err(OpError::new(MOVE_PLAN_STALE).with("path", *p));
    }
    for s in files {
        let actual = content_hash(&s.bytes);
        match want.get(s.virt.as_str()) {
            Some(h) if *h == actual => {}
            Some(h) => return Err(conflict(&s.virt, h, &actual)),
            None => return Err(OpError::new(MOVE_PLAN_STALE).with("path", s.virt.as_str())),
        }
    }
    Ok(())
}

fn conflict(virt: &str, expected: &str, actual: &str) -> OpError {
    OpError::new(FILE_CONFLICT)
        .with("path", virt)
        .with("expected", expected)
        .with("actual", actual)
}

/// Copy every retained buffer (never overwriting), then read each copy back and compare its hash.
/// On any failure this call's copies are removed.
pub(super) fn copy_verified(
    target: &Place,
    files: &[Src],
    rels: &[String],
) -> Result<Vec<(String, String)>, OpError> {
    let mut made: Vec<(String, String)> = Vec::new();
    let res = copy_all(target, files, rels, &mut made);
    if res.is_err() {
        undo(&target.root, &made);
    }
    res.map(|()| made)
}

fn copy_all(
    target: &Place,
    files: &[Src],
    rels: &[String],
    made: &mut Vec<(String, String)>,
) -> Result<(), OpError> {
    for (s, rel) in files.iter().zip(rels) {
        match create_new(&target.root, Path::new(rel), &s.bytes) {
            Ok(()) => made.push((rel.clone(), content_hash(&s.bytes))),
            Err(VaultError::Exists { .. }) => {
                return Err(OpError::new(MOVE_TARGET_EXISTS).with("path", s.virt.as_str()));
            }
            Err(e) => return Err(vault_error_at(e, &target.root, &s.virt)),
        }
    }
    for ((rel, hash), s) in made.iter().zip(files) {
        let back = read_bounded(&target.root, Path::new(rel), MAX_ARTIFACT_BYTES)
            .map_err(|e| vault_error_at(e, &target.root, &s.virt))?;
        let actual = content_hash(&back);
        if actual != *hash {
            return Err(conflict(&s.virt, hash, &actual));
        }
    }
    Ok(())
}

/// Delete this call's copies, hash-checked. A copy that cannot be removed stays (logged).
// ponytail: a stuck copy is only logged; an error carries no warnings — add a channel if it bites
fn undo(root: &Root, made: &[(String, String)]) {
    for (rel, hash) in made {
        if let Err(e) = delete(root, Path::new(rel), hash, MAX_ARTIFACT_BYTES) {
            log::warn!("move: copy {rel} left in place: {e}");
        }
    }
}

/// After the switch: delete each source, hash-checked. A failure is a `move.source_kept` warning.
pub(super) fn delete_sources(src: &Place, files: &[Src]) -> Vec<OpError> {
    files
        .iter()
        .filter(|s| {
            delete(
                &src.root,
                Path::new(&s.rel),
                &content_hash(&s.bytes),
                MAX_ARTIFACT_BYTES,
            )
            .is_err()
        })
        .map(|s| OpError::new(MOVE_SOURCE_KEPT).with("path", s.virt.as_str()))
        .collect()
}

/// Undo hook for the caller when the machine-file write fails.
pub(super) fn rollback(target: &Place, made: &[(String, String)]) {
    undo(&target.root, made);
}
