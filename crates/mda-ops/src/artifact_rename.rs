//! `artifact.rename` (ER-6): move an artifact file to a new name in its type directory — bytes
//! verbatim, or rewritten from a model in the same op — never overwriting, and report the template
//! indexes that link the old path (spec §14.2, §14.3).

use std::path::Path;

use mda_core::model::ArtifactModel;
use mda_vault::{contain, content_hash, rename_new};

use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, bad_path, root_of};
use crate::artifact_rewrite::prepare;
use crate::error::vault_error;
use crate::rename_links::index_links_to;
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
}

/// `artifact.rename` response: the new path, the hash of its bytes, and the `rename.*` warnings.
///
/// # Examples
///
/// ```
/// let r = mda_ops::artifact_rename::RenameResponse {
///     path: "Snippets/b.md".into(), hash: "h".into(), warnings: vec![] };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"path":"Snippets/b.md","hash":"h","warnings":[]}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct RenameResponse {
    pub path: String,
    pub hash: String,
    pub warnings: Vec<OpError>,
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
    let root = &root_of(ctx)?;
    let t = artifact_path(&req.path)?;
    let new = req.new_path.as_str();
    artifact_path(new)?;
    // Same type dir (`artifact_path` guarantees the first segment is one).
    if new.split('/').next() != req.path.split('/').next() {
        return Err(bad_path(new));
    }
    let stem = new.rsplit('/').next().and_then(|f| f.strip_suffix(".md"));
    mda_core::naming::validate_stem(stem.unwrap_or_default())?;
    if new == req.path {
        return Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "same_path"));
    }
    // Type-dir pin: canonical, so a symlinked type dir works, an in-vault symlink to another type
    // dir does not; a symlink out of the vault is `contain`'s own `path.outside_root`.
    let dir = req.path.split('/').next().unwrap_or_default();
    let dir_abs = contain(root, Path::new(dir)).map_err(|e| vault_error(e, root))?;
    let new_abs = contain(root, Path::new(new)).map_err(|e| vault_error(e, root))?;
    if !new_abs.starts_with(&dir_abs) {
        return Err(bad_path(new));
    }
    let hash = match req.model {
        None => {
            // Verbatim: the file is never parsed; `rename_new` does the one hash check.
            rename_new(
                root,
                Path::new(&req.path),
                Path::new(new),
                &req.expected_hash,
                None,
                MAX_ARTIFACT_BYTES,
            )
            .map_err(|e| exists_actual(root, Path::new(new), e))?;
            req.expected_hash
        }
        Some(model) => {
            // Same guards as `artifact.update`; type mismatch names the old path, as there.
            let text = prepare(root, &req.path, new, &req.expected_hash, |_| {
                if t == model.artifact_type {
                    Ok(model)
                } else {
                    Err(bad_path(&req.path))
                }
            })?;
            rename_new(
                root,
                Path::new(&req.path),
                Path::new(new),
                &req.expected_hash,
                Some(text.as_bytes()),
                MAX_ARTIFACT_BYTES,
            )
            .map_err(|e| exists_actual(root, Path::new(new), e))?;
            content_hash(text.as_bytes())
        }
    };
    Ok(RenameResponse {
        path: req.new_path,
        hash,
        warnings: index_links_to(root, &req.path),
    })
}
