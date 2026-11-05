//! `artifact.rename` (ER-6): move an artifact file to a new name in its type directory — bytes
//! verbatim, or rewritten from a model in the same op — never overwriting, and report the template
//! indexes that link the old path (spec §14.2, §14.3).

use std::path::Path;

use mda_core::model::ArtifactModel;
use mda_vault::{Root, VaultError, contain, content_hash, rename_new};

use crate::artifact::{
    MAX_ARTIFACT_BYTES, artifact_path, bad_path, check_dirs_at, generic_path, locate, mid_dirs,
    no_dot_below_root, segments,
};
use crate::artifact_rewrite::prepare_at;
use crate::error::{vault_error, vault_error_at};
use crate::location::{Resolved, map};
use crate::rename_links::{index_links_to, update_index_links};
use crate::write_file::exists_actual;
use crate::{Ctx, OpError, error};

/// `artifact.rename` params: the file at `path` (hash `expectedHash`) moves to `newPath` in the same
/// type directory; with `model`, the moved file's content is that model, serialized as `artifact.update`
/// would.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact_rename::RenameRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Snippets/a.md", "expectedHash": "h", "newPath": "Snippets/b.md"})).unwrap();
/// assert!(r.model.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameRequest {
    pub path: String,
    pub expected_hash: String,
    pub new_path: String,
    #[serde(default)]
    pub model: Option<ArtifactModel>,
    #[serde(default)]
    pub update_links: bool,
}

/// One index rewritten by `updateLinks`: its vault path and the hash of its new bytes.
///
/// # Examples
///
/// ```
/// let r = mda_ops::artifact_rename::UpdatedIndex { path: "Templates/i.md".into(), hash: "h".into() };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"path":"Templates/i.md","hash":"h"}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedIndex {
    pub path: String,
    pub hash: String,
}

/// `artifact.rename` response: the new path, the hash of its bytes, the `rename.*` warnings, and with
/// `updateLinks` the indexes rewritten.
///
/// # Examples
///
/// ```
/// let r = mda_ops::artifact_rename::RenameResponse {
///     path: "Snippets/b.md".into(), hash: "h".into(), warnings: vec![], updated_indexes: None };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"path":"Snippets/b.md","hash":"h","warnings":[]}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameResponse {
    pub path: String,
    pub hash: String,
    pub warnings: Vec<OpError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_indexes: Option<Vec<UpdatedIndex>>,
}

/// `artifact.rename`: refuse before any write, then move without clobbering.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact_rename::{rename, RenameRequest}};
/// let req: RenameRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Snippets/a.md", "expectedHash": "h", "newPath": "Snippets/b.md"})).unwrap();
/// assert!(rename(&Ctx::new(None), req).is_err());
/// ```
pub fn rename(ctx: &Ctx, req: RenameRequest) -> Result<RenameResponse, OpError> {
    if req.model.is_some() {
        generic_path(&req.path)?;
    }
    let r = locate(ctx, &req.path)?;
    let (root, t) = (&r.place.root, r.ty);
    let new = req.new_path.as_str();
    // Same type dir; `newPath` maps against the SAME place (resolved once) and passes the same
    // guards: `.md` path rule, dot rule at a root location, canonical dot check.
    if artifact_path(new)? != t || new.split('/').next() != req.path.split('/').next() {
        return Err(bad_path(new));
    }
    let new_rel = map(t, &r.place, new)?;
    no_dot_below_root(&r.place, &new_rel, new)?;
    let new_segs = segments(new).map_err(|()| bad_path(new))?;
    check_dirs_at(&r.place, mid_dirs(&new_segs))?;
    let stem = new.rsplit('/').next().and_then(|f| f.strip_suffix(".md"));
    mda_core::naming::validate_stem(stem.unwrap_or_default())?;
    if new == req.path {
        return Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "same_path"));
    }
    // Type-dir pin: canonical, so a symlinked type dir works, an in-vault symlink to another type
    // dir does not; a symlink out of the vault is `contain`'s own `path.outside_root`.
    let dir_abs = contain(root, Path::new(r.place.base.as_deref().unwrap_or_default()))
        .map_err(|e| vault_error_at(e, root, new))?;
    let new_abs = contain(root, Path::new(&new_rel)).map_err(|e| vault_error_at(e, root, new))?;
    if !new_abs.starts_with(&dir_abs) {
        return Err(bad_path(new));
    }
    let hash = match req.model {
        None => {
            // Verbatim: the file is never parsed; `rename_new` does the one hash check.
            rename_new(
                root,
                Path::new(&r.rel),
                Path::new(&new_rel),
                &req.expected_hash,
                None,
                MAX_ARTIFACT_BYTES,
            )
            .map_err(|e| rename_err(root, &r, &new_rel, new, e))?;
            req.expected_hash
        }
        Some(model) => {
            // Same guards as `artifact.update`; type mismatch names the old path, as there.
            let text = prepare_at(&r, new, &req.expected_hash, |_| {
                if t == model.artifact_type {
                    Ok(model)
                } else {
                    Err(bad_path(&req.path))
                }
            })?;
            rename_new(
                root,
                Path::new(&r.rel),
                Path::new(&new_rel),
                &req.expected_hash,
                Some(text.as_bytes()),
                MAX_ARTIFACT_BYTES,
            )
            .map_err(|e| rename_err(root, &r, &new_rel, new, e))?;
            content_hash(text.as_bytes())
        }
    };
    let (warnings, updated) = if req.update_links {
        update_index_links(&r.place, &req.path, new)
    } else {
        (index_links_to(&r.place, &req.path), Vec::new())
    };
    Ok(RenameResponse {
        path: req.new_path,
        hash,
        warnings,
        updated_indexes: req.update_links.then_some(updated),
    })
}

/// A `rename_new` error with virtual paths (D-1): `Exists` names the target (plus `actual`); any
/// other error names the target when it carries the target's physical path, else the source.
fn rename_err(root: &Root, r: &Resolved, new_rel: &str, new: &str, e: VaultError) -> OpError {
    if matches!(e, VaultError::Exists { .. }) {
        return exists_actual(root, Path::new(new_rel), new, e);
    }
    let op = vault_error(e, root);
    let virt = if op.params.get("path").is_some_and(|p| p == new_rel) {
        new
    } else {
        &r.virt
    };
    let mut op = op;
    if let Some(p) = op.params.get_mut("path") {
        virt.clone_into(p);
    }
    op
}
