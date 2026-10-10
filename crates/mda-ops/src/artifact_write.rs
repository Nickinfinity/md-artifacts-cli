//! The write ops: `artifact.create`, `artifact.update`, `artifact.patch`, `artifact.delete`. Every
//! write except create carries the content hash the client last read (`file.conflict` on a
//! mismatch); every write is contained, atomic and re-parsed before it lands (W-8…W-13).

use std::path::Path;

use mda_core::error::Refusal;
use mda_core::model::ArtifactModel;
use mda_core::parse::{decode, is_flagged, parse_from_content};
use mda_core::patch::{CodeTarget, FmField, PatchError, patch_code, patch_field};
use mda_core::serialize::{SerializeError, serialize};
use mda_vault::{Root, content_hash, create_new, delete as vault_delete, read_bounded, replace};

use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, bad_path, root_of};
use crate::error::vault_error;
use crate::write_file::exists_actual;
use crate::{Ctx, OpError, error};

/// `artifact.create` params: the new file's vault-relative path and its model.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact_write::CreateRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "model": {"artifactType": "Template"}})).unwrap();
/// assert_eq!(r.path, "Templates/a.md");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRequest {
    pub path: String,
    pub model: ArtifactModel,
}

/// `artifact.update` params: rewrite the file at `path` from `model`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact_write::UpdateRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "expectedHash": "h", "model": {"artifactType": "Template"}})).unwrap();
/// assert_eq!(r.expected_hash, "h");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateRequest {
    pub path: String,
    pub expected_hash: String,
    pub model: ArtifactModel,
}

/// `artifact.patch` params: one in-place edit of the file at `path`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact_write::PatchRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "expectedHash": "h", "edit": {"field": "title", "value": "T"}})).unwrap();
/// assert!(matches!(r.edit, mda_ops::artifact_write::PatchEdit::Title { .. }));
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchRequest {
    pub path: String,
    pub expected_hash: String,
    pub edit: PatchEdit,
}

/// The edit a patch makes, tagged by `field`. `code` with no `block` targets the single body;
/// with `block`, `heading` must name that block (W-5).
///
/// # Examples
///
/// ```
/// use mda_ops::artifact_write::PatchEdit;
/// let e: PatchEdit = serde_json::from_value(serde_json::json!(
///     {"field": "code", "block": 1, "heading": "Two", "code": "x"})).unwrap();
/// assert!(matches!(e, PatchEdit::Code { block: Some(1), .. }));
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "field", rename_all = "camelCase", deny_unknown_fields)]
pub enum PatchEdit {
    Title {
        value: String,
    },
    Description {
        value: String,
    },
    Code {
        #[serde(default)]
        block: Option<usize>,
        #[serde(default)]
        heading: Option<String>,
        code: String,
    },
}

/// `artifact.delete` params.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact_write::DeleteRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "expectedHash": "h"})).unwrap();
/// assert_eq!(r.path, "Templates/a.md");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteRequest {
    pub path: String,
    pub expected_hash: String,
}

/// A successful create/update/patch: the path and the hash of the bytes written.
///
/// # Examples
///
/// ```
/// let r = mda_ops::artifact_write::WriteResponse { path: "Templates/a.md".into(), hash: "h".into() };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"path":"Templates/a.md","hash":"h"}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct WriteResponse {
    pub path: String,
    pub hash: String,
}

/// A successful delete.
///
/// # Examples
///
/// ```
/// let r = mda_ops::artifact_write::DeleteResponse { path: "Templates/a.md".into() };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"path":"Templates/a.md"}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct DeleteResponse {
    pub path: String,
}

/// `file.too_large` when `size` exceeds the read limit: the engine never writes a file its own
/// `read_bounded` would refuse.
fn too_large(path: &str, size: usize) -> Result<(), OpError> {
    if size > max_bytes() {
        return Err(OpError::new(error::FILE_TOO_LARGE)
            .with("path", path)
            .with("size", size.to_string())
            .with("max", MAX_ARTIFACT_BYTES.to_string()));
    }
    Ok(())
}

fn max_bytes() -> usize {
    usize::try_from(MAX_ARTIFACT_BYTES).unwrap_or(usize::MAX)
}

fn map_serialize(e: SerializeError, path: &str) -> OpError {
    match e {
        SerializeError::Vars(v) => OpError {
            code: v.code,
            params: v.params,
        },
        SerializeError::Vks(u) => OpError::new(error::VKS_UNREPRESENTABLE)
            .with("var", u.var)
            .with("reason", u.reason.as_str()),
        SerializeError::Field { field, reason } => OpError::new(error::ARTIFACT_UNREPRESENTABLE)
            .with("field", field)
            .with("reason", reason.as_str()),
        SerializeError::TooLarge { size } => OpError::new(error::FILE_TOO_LARGE)
            .with("path", path)
            .with("size", size.to_string())
            .with("max", MAX_ARTIFACT_BYTES.to_string()),
    }
}

fn map_patch(e: PatchError) -> OpError {
    match e {
        PatchError::Refused(r) => not_serializable(r),
        PatchError::BlockNotFound(i) => OpError::new(error::ARTIFACT_BLOCK_NOT_FOUND)
            .with("block", i.map_or_else(String::new, |i| i.to_string())),
        PatchError::Unrepresentable(f) => OpError::new(error::ARTIFACT_UNREPRESENTABLE)
            .with("field", f)
            .with("reason", "round_trip"),
    }
}

fn not_serializable(r: Refusal) -> OpError {
    OpError::new(error::ARTIFACT_NOT_SERIALIZABLE).with("reason", r.as_str())
}

/// The file's bytes, read bounded, whose hash must be `expected` — checked before anything is
/// parsed, so a file another client changed reports `file.conflict`, not a refusal.
fn read_checked(root: &Root, path: &str, expected: &str) -> Result<Vec<u8>, OpError> {
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

fn written(path: String, text: &str) -> WriteResponse {
    WriteResponse {
        hash: content_hash(text.as_bytes()),
        path,
    }
}

/// `artifact.create`: write a new file from a model; never overwrites (`file.exists`).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact_write::{create, CreateRequest}};
/// let req: CreateRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "model": {"artifactType": "Template"}})).unwrap();
/// assert_eq!(create(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn create(ctx: &Ctx, req: CreateRequest) -> Result<WriteResponse, OpError> {
    let root = root_of(ctx)?;
    if artifact_path(&req.path)? != req.model.artifact_type {
        return Err(bad_path(&req.path));
    }
    let text = serialize(&req.model, max_bytes()).map_err(|e| map_serialize(e, &req.path))?;
    let rel = Path::new(&req.path);
    create_new(root, rel, text.as_bytes()).map_err(|e| exists_actual(root, rel, e))?;
    Ok(written(req.path, &text))
}

/// `artifact.update`: rewrite a file from a model, when its hash is `expectedHash`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact_write::{update, UpdateRequest}};
/// let req: UpdateRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "expectedHash": "h", "model": {"artifactType": "Template"}})).unwrap();
/// assert_eq!(update(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn update(ctx: &Ctx, req: UpdateRequest) -> Result<WriteResponse, OpError> {
    let root = root_of(ctx)?;
    let t = artifact_path(&req.path)?;
    let bytes = read_checked(root, &req.path, &req.expected_hash)?;
    let content = decode(&bytes);
    if is_flagged(&content) {
        return Err(not_serializable(Refusal::Flagged));
    }
    let cur = parse_from_content(&content, &req.path);
    if cur.frontmatter.index == Some(true) {
        return Err(not_serializable(Refusal::Index));
    }
    if cur.vars_error.is_some() || cur.blocks.iter().any(|b| b.vars_error.is_some()) {
        return Err(OpError::new(error::ARTIFACT_VARS_INVALID).with("path", req.path));
    }
    if t != req.model.artifact_type {
        return Err(bad_path(&req.path));
    }
    let text = serialize(&req.model, max_bytes()).map_err(|e| map_serialize(e, &req.path))?;
    replace(
        root,
        Path::new(&req.path),
        text.as_bytes(),
        &req.expected_hash,
        MAX_ARTIFACT_BYTES,
    )
    .map_err(|e| vault_error(e, root))?;
    Ok(written(req.path, &text))
}

/// `artifact.patch`: one byte-surgical edit, when the file's hash is `expectedHash`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact_write::{patch, PatchRequest}};
/// let req: PatchRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "expectedHash": "h", "edit": {"field": "title", "value": "T"}})).unwrap();
/// assert_eq!(patch(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn patch(ctx: &Ctx, req: PatchRequest) -> Result<WriteResponse, OpError> {
    let root = root_of(ctx)?;
    artifact_path(&req.path)?;
    let edit = req.edit;
    let bad = |r: &str| OpError::new(error::OP_BAD_REQUEST).with("reason", r);
    let target = match &edit {
        PatchEdit::Code {
            block: Some(i),
            heading: Some(h),
            ..
        } => Some(CodeTarget::Block {
            index: *i,
            heading: h.clone(),
        }),
        PatchEdit::Code {
            block: Some(_),
            heading: None,
            ..
        } => {
            return Err(bad("heading required with block"));
        }
        PatchEdit::Code {
            block: None,
            heading: Some(_),
            ..
        } => {
            return Err(bad("block required with heading"));
        }
        PatchEdit::Code { .. } => Some(CodeTarget::Single),
        _ => None,
    };
    let bytes = read_checked(root, &req.path, &req.expected_hash)?;
    // Lossy like `update` and Obsidian's own save: invalid UTF-8 becomes U+FFFD (decision #35).
    let content = decode(&bytes);
    let added = match &edit {
        PatchEdit::Title { value } | PatchEdit::Description { value } => value.len(),
        PatchEdit::Code { code, .. } => code.len(),
    };
    // Bounds the work; the post-check below bounds what lands (an inserted key, the code's
    // newline and a restored BOM add bytes this sum cannot see).
    too_large(&req.path, content.len().saturating_add(added))?;
    let patched = match (&edit, target) {
        (PatchEdit::Code { code, .. }, Some(t)) => patch_code(&content, &req.path, &t, code),
        (PatchEdit::Title { value }, _) => patch_field(&content, &req.path, FmField::Title, value),
        (PatchEdit::Description { value }, _) => {
            patch_field(&content, &req.path, FmField::Description, value)
        }
        // Code always yields a target above.
        (PatchEdit::Code { .. }, None) => return Err(bad("code target")),
    }
    .map_err(map_patch)?;
    // Written without a BOM, as `update` and Obsidian write (decision #36).
    let out = patched;
    too_large(&req.path, out.len())?;
    replace(
        root,
        Path::new(&req.path),
        out.as_bytes(),
        &req.expected_hash,
        MAX_ARTIFACT_BYTES,
    )
    .map_err(|e| vault_error(e, root))?;
    Ok(written(req.path, &out))
}

/// `artifact.delete`: remove a file, when its hash is `expectedHash`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact_write::{delete, DeleteRequest}};
/// let req: DeleteRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "expectedHash": "h"})).unwrap();
/// assert_eq!(delete(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn delete(ctx: &Ctx, req: DeleteRequest) -> Result<DeleteResponse, OpError> {
    let root = root_of(ctx)?;
    artifact_path(&req.path)?;
    vault_delete(
        root,
        Path::new(&req.path),
        &req.expected_hash,
        MAX_ARTIFACT_BYTES,
    )
    .map_err(|e| vault_error(e, root))?;
    Ok(DeleteResponse { path: req.path })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pins deny_unknown_fields under the internal `field` tag (B-RISK 15).
    #[test]
    fn patch_edit_rejects_unknown_field() {
        let r = serde_json::from_value::<PatchEdit>(
            serde_json::json!({"field": "title", "value": "x", "x": 1}),
        );
        assert!(r.is_err());
    }
}
