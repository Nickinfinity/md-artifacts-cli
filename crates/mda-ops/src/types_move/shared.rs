//! What `plan` and `apply` share: the one source walk and the virtual → target mapping (D-9, D-19).

use std::path::Path;

use mda_core::registry::ArtifactType;
use mda_vault::{contain, walk_md_move};

use crate::OpError;
use crate::artifact::{MAX_ARTIFACT_BYTES, no_dot_below_root};
use crate::error::{
    ARTIFACT_BAD_PATH, MOVE_BAD_PATH, MOVE_LEFT_BEHIND, vault_error, vault_error_at,
};
use crate::location::{Place, map};
use crate::vars::to_virtual;

/// One source file, retained whole: the walk is the only read of it (D-9).
pub(super) struct Src {
    /// Physical vault-relative path in the source place.
    pub rel: String,
    /// The virtual path the client sees.
    pub virt: String,
    pub bytes: Vec<u8>,
}

/// Walk the source place once (`walk_md_move`), sorted by virtual path. Any per-file error refuses
/// the call; the count of entries a move leaves behind is returned.
pub(super) fn walk_source(t: ArtifactType, place: &Place) -> Result<(Vec<Src>, usize), OpError> {
    let base = place.base.as_deref();
    let mut out = Vec::new();
    let mut first_err: Option<OpError> = None;
    let left = walk_md_move(
        &[(&place.root, base.unwrap_or(""))],
        MAX_ARTIFACT_BYTES,
        |_, rel, res| {
            let virt = to_virtual(t, base, rel);
            match res {
                Ok(bytes) => out.push(Src {
                    rel: rel.to_owned(),
                    virt,
                    bytes,
                }),
                Err(e) => {
                    first_err.get_or_insert(vault_error_at(e, &place.root, &virt));
                }
            }
        },
    )
    .map_err(|e| vault_error(e, &place.root))?;
    if let Some(e) = first_err {
        return Err(e);
    }
    out.sort_by(|a, b| a.virt.cmp(&b.virt));
    Ok((out, left))
}

/// The `move.left_behind` warning, when something stays.
pub(super) fn left_warning(left: usize) -> Option<OpError> {
    (left > 0).then(|| OpError::new(MOVE_LEFT_BEHIND).with("count", left.to_string()))
}

/// The physical path of `virt` in the target `place`. The mapping's `artifact.bad_path` (a
/// `.`-segment headed for a root location, string-level or through a symlink) is `move.bad_path`.
pub(super) fn target_rel(t: ArtifactType, place: &Place, virt: &str) -> Result<String, OpError> {
    let remap = |e: OpError| {
        if e.code == ARTIFACT_BAD_PATH {
            OpError::new(MOVE_BAD_PATH).with("path", virt)
        } else {
            e
        }
    };
    let rel = map(t, place, virt).map_err(remap)?;
    no_dot_below_root(place, &rel, virt).map_err(remap)?;
    Ok(rel)
}

/// Does anything (a dangling symlink included) already sit at `rel` in `place`?
pub(super) fn occupied(place: &Place, rel: &str, virt: &str) -> Result<bool, OpError> {
    let full =
        contain(&place.root, Path::new(rel)).map_err(|e| vault_error_at(e, &place.root, virt))?;
    Ok(full.symlink_metadata().is_ok())
}
