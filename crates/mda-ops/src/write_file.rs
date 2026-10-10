//! `artifact.write_file`: render a whole-file artifact (Template, AIAgentsConfig) and write the
//! result into a folder of the client's workspace — contained to the root the client passes and
//! written through the same atomic path as vault writes.

use std::path::Path;

use mda_core::model::ParsedArtifact;
use mda_core::naming::output_name;
use mda_core::render::{Values, Warning, check_values, render_artifact};
use mda_vault::{Root, VaultError, content_hash, create_new, read_bounded, replace};

use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, load, root_of, segments};
use crate::error::vault_error;
use crate::render::render_error;
use crate::{Ctx, OpError, error};

/// `artifact.write_file` params. `workspaceRoot` is absolute; `destDir` is workspace-relative POSIX
/// (`""` = the root). No `expectedHash` → create (never clobbers); a hash → replace that file.
///
/// # Examples
///
/// ```
/// let r: mda_ops::write_file::WriteFileRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "workspaceRoot": "/w", "destDir": ""})).unwrap();
/// assert!(r.expected_hash.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteFileRequest {
    pub path: String,
    #[serde(default)]
    pub block: Option<usize>,
    #[serde(default)]
    pub values: Values,
    pub workspace_root: String,
    pub dest_dir: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub expected_hash: Option<String>,
}

/// What `artifact.write_file` wrote: the workspace-relative path, the written bytes' hash, the
/// render warnings and whether the bytes hold an ESC character.
///
/// # Examples
///
/// ```
/// let r = mda_ops::write_file::WriteFileResponse { path: "a.ts".into(), hash: "h".into(), warnings: vec![], contains_escape: false };
/// assert!(serde_json::to_string(&r).unwrap().contains(r#""containsEscape":false"#));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileResponse {
    pub path: String,
    pub hash: String,
    pub warnings: Vec<Warning>,
    pub contains_escape: bool,
}

/// `artifact.write_file`: render a whole-file artifact into the client's workspace.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, write_file};
/// let req: write_file::WriteFileRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/a.md", "workspaceRoot": "/w", "destDir": ""})).unwrap();
/// assert_eq!(write_file::write_file(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn write_file(ctx: &Ctx, req: WriteFileRequest) -> Result<WriteFileResponse, OpError> {
    let root = root_of(ctx)?;
    artifact_path(&req.path)?;
    if !Path::new(&req.workspace_root).is_absolute() {
        return Err(
            OpError::new(error::OP_BAD_REQUEST).with("reason", "workspaceRoot must be absolute")
        );
    }
    check_dest(&req.dest_dir)?;
    check_values(&req.values)?;
    let ws = Root::new(Path::new(&req.workspace_root)).map_err(OpError::from)?;
    let p = load(root, &req.path)?.artifact;
    emit_whole_file(
        &ws,
        &req.dest_dir,
        &p,
        req.block,
        req.file_name.as_deref(),
        &req.values,
        req.expected_hash.as_deref(),
    )
}

/// The reusable core: type checks, output name, render, then an atomic write inside `ws`. Also the
/// per-step writer of `index.run`, so the `destDir` rule lives here and cannot be bypassed.
pub(crate) fn emit_whole_file(
    ws: &Root,
    dest_dir: &str,
    p: &ParsedArtifact,
    block: Option<usize>,
    file_name: Option<&str>,
    values: &Values,
    expected: Option<&str>,
) -> Result<WriteFileResponse, OpError> {
    // The parsed type decides, not the directory (TS `writesWholeFile(frontmatter.artifactType)`).
    if !p.frontmatter.artifact_type.info().writes_file {
        return Err(OpError::new(error::ARTIFACT_NOT_WHOLE_FILE)
            .with("type", p.frontmatter.artifact_type.as_str()));
    }
    if p.frontmatter.index == Some(true) {
        return Err(OpError::new(error::ARTIFACT_IS_INDEX));
    }
    if p.blocks.len() > 1 {
        return Err(
            OpError::new(error::ARTIFACT_MULTI_BLOCK).with("count", p.blocks.len().to_string())
        );
    }
    let name = output_name(p, file_name)?;
    let rel = dest_rel(dest_dir, &name)?;
    let out = render_artifact(p, block, None, values).map_err(render_error)?;
    let bytes = out.output.as_bytes();
    let path = Path::new(&rel);
    match expected {
        None => create_new(ws, path, bytes).map_err(|e| exists_actual(ws, path, e))?,
        Some(h) => {
            replace(ws, path, bytes, h, MAX_ARTIFACT_BYTES).map_err(|e| vault_error(e, ws))?;
        }
    }
    Ok(WriteFileResponse {
        path: rel,
        hash: content_hash(bytes),
        warnings: out.warnings,
        contains_escape: out.contains_escape,
    })
}

fn check_dest(dest_dir: &str) -> Result<(), OpError> {
    if dest_dir.is_empty() || segments(dest_dir).is_ok() {
        return Ok(());
    }
    Err(OpError::new(error::PATH_OUTSIDE_ROOT).with("path", dest_dir))
}

/// Workspace-relative POSIX path of the output: `name` alone for `destDir` `""`, else
/// `destDir/name`. A `destDir` that is not plain segments is `path.outside_root` (reject, never
/// sanitise); the writer's containment re-checks the real path.
pub(crate) fn dest_rel(dest_dir: &str, name: &str) -> Result<String, OpError> {
    check_dest(dest_dir)?;
    Ok(if dest_dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dest_dir}/{name}")
    })
}

/// Map a create-time vault error: `file.exists` also carries `actual`, the hash of the file in the
/// way, so a client can retry as a replace. Every other error maps through `vault_error(root)`.
pub(crate) fn exists_actual(root: &Root, rel: &Path, e: VaultError) -> OpError {
    let exists = matches!(e, VaultError::Exists { .. });
    let op = vault_error(e, root);
    if !exists {
        return op;
    }
    match read_bounded(root, rel, MAX_ARTIFACT_BYTES) {
        Ok(b) => op.with("actual", content_hash(&b)),
        Err(_) => op,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_file_request_requires_workspace_root() {
        let r = serde_json::from_value::<WriteFileRequest>(
            serde_json::json!({"path": "Templates/a.md", "destDir": ""}),
        );
        assert!(r.is_err());
    }
}
