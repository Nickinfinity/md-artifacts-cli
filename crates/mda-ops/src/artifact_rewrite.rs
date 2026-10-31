//! The one rewrite path (X-12): the hash-checked bounded read and the read → check → edit →
//! serialize → replace sequence shared by `artifact.update` and every `vars.*` edit.

use std::path::Path;

use mda_core::error::Refusal;
use mda_core::model::{ArtifactModel, ParsedArtifact, PayloadKind};
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::ArtifactType;
use mda_core::serialize::serialize;
use mda_vault::{Root, content_hash, read_bounded, replace};

use crate::artifact::MAX_ARTIFACT_BYTES;
use crate::artifact_write::{WriteResponse, map_serialize, max_bytes, not_serializable, written};
use crate::error::vault_error;
use crate::{OpError, error};

/// The file's bytes, read bounded, whose hash must be `expected` — checked before anything is
/// parsed, so a file another client changed reports `file.conflict`, not a refusal.
pub(crate) fn read_checked(root: &Root, path: &str, expected: &str) -> Result<Vec<u8>, OpError> {
    let bytes = read_bounded(root, Path::new(path), MAX_ARTIFACT_BYTES)
        .map_err(|e| vault_error(e, root))?;
    let actual = content_hash(&bytes);
    if actual != expected {
        return Err(OpError::new(error::FILE_CONFLICT)
            .with("path", path)
            .with("expected", expected)
            .with("actual", actual));
    }
    Ok(bytes)
}

/// The pre-write half of the rewrite path (X-12): read bounded → hash check (on `src`) → refuse
/// varsError → `edit` → layout guards → serialize + guard; returns the text to write. `err_path`
/// names the file in serialize/size errors (differs from `src` for `artifact.rename`).
pub(crate) fn prepare(
    root: &Root,
    src: &str,
    err_path: &str,
    expected: &str,
    edit: impl FnOnce(ParsedArtifact) -> Result<ArtifactModel, OpError>,
) -> Result<String, OpError> {
    let bytes = read_checked(root, src, expected)?;
    let content = decode(&bytes);
    let cur = parse_from_content(&content, src);
    let flagged = cur.layout.payload == PayloadKind::Flags;
    let fm = &cur.frontmatter;
    let paths = fm.index == Some(true) && fm.paths.as_ref().is_some_and(|p| !p.is_empty());
    // A Variables file's content is its vks fence (spec §3): flags there cannot be written back.
    if flagged && fm.artifact_type == ArtifactType::Variables {
        return Err(not_serializable(Refusal::Flagged));
    }
    if cur.vars_error.is_some() || cur.blocks.iter().any(|b| b.vars_error.is_some()) {
        return Err(OpError::new(error::ARTIFACT_VARS_INVALID).with("path", src));
    }
    let model = edit(cur)?;
    // A 1.0 client never sends `layout` / `paths`: its model would silently drop the notes, flags
    // or `paths`, so such a rewrite stays refused, as it was before W7c (W-7).
    if flagged && model.layout.payload != PayloadKind::Flags {
        return Err(not_serializable(Refusal::Flagged));
    }
    if paths && model.paths.is_empty() {
        return Err(not_serializable(Refusal::Index));
    }
    serialize(&model, max_bytes()).map_err(|e| map_serialize(e, err_path))
}

/// The one rewrite path (X-12): [`prepare`] then replace. Shared by `artifact.update` and every
/// `vars.*` edit, so lossy UTF-8 (#35), no BOM (#36) and legacy → YAML (W-3) hold for all of them.
/// Since W7c a flagged or index file is rewritten when the model carries its layout / `paths`
/// (spec §7.6).
pub(crate) fn rewrite(
    root: &Root,
    path: &str,
    expected: &str,
    edit: impl FnOnce(ParsedArtifact) -> Result<ArtifactModel, OpError>,
) -> Result<WriteResponse, OpError> {
    let text = prepare(root, path, path, expected, edit)?;
    replace(
        root,
        Path::new(path),
        text.as_bytes(),
        expected,
        MAX_ARTIFACT_BYTES,
    )
    .map_err(|e| vault_error(e, root))?;
    Ok(written(path.to_owned(), &text))
}
