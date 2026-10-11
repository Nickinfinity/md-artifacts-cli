//! `types.list` / `types.set_enabled`: the artifact types of the session vault, enabled per
//! `<vault>/.mda/config.toml` else by directory existence (V-12). Enabling creates the directory;
//! disabling never deletes it (V-11). Ports the settings panel's type toggles minus the delete.

use mda_core::registry::TypeInfo;
use mda_vault::config::{VaultConfig, read_vault_config, write_vault_config};
use mda_vault::{DirState, Root, create_type_dir, dir_state};

use crate::artifact::root_of;
use crate::error::vault_error;
use crate::{ArtifactType, Ctx, OpError, TYPES};

/// `types.list` request (no fields).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ListTypesRequest {}

/// One type in the session vault.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRow {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub dir: &'static str,
    pub enabled: bool,
    pub dir_exists: bool,
}

/// `types.set_enabled` request: the exact type spelling and the new state.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SetEnabledRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub enabled: bool,
}

/// `types.set_enabled` response: `created` when enabling made the directory.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetEnabledResponse {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub enabled: bool,
    pub created: bool,
}

/// `types.list`: every type in registry order.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, types::{list, ListTypesRequest}};
/// let e = list(&Ctx::new(None), ListTypesRequest {}).unwrap_err();
/// assert_eq!(e.code, "vault.not_selected");
/// ```
pub fn list(ctx: &Ctx, req: ListTypesRequest) -> Result<Vec<TypeRow>, OpError> {
    let ListTypesRequest {} = req;
    let root = &root_of(ctx)?;
    let cfg = read_vault_config(root).map_err(|e| vault_error(e, root))?;
    Ok(TYPES.iter().map(|t| row(root, &cfg, t)).collect())
}

/// `enabled` is the vault file's say, else whether the directory exists (V-12).
fn row(root: &Root, cfg: &VaultConfig, t: &TypeInfo) -> TypeRow {
    let dir_exists = dir_state(root, t.dir) == DirState::Present;
    TypeRow {
        artifact_type: t.artifact_type,
        dir: t.dir,
        enabled: cfg.enabled(t.artifact_type).unwrap_or(dir_exists),
        dir_exists,
    }
}

/// `types.set_enabled`: write the vault file; enable creates the directory first.
///
/// # Examples
///
/// ```
/// use mda_ops::{ArtifactType, Ctx, types::{set_enabled, SetEnabledRequest}};
/// let req = SetEnabledRequest { artifact_type: ArtifactType::Command, enabled: true };
/// let e = set_enabled(&Ctx::new(None), req).unwrap_err();
/// assert_eq!(e.code, "vault.not_selected");
/// ```
pub fn set_enabled(ctx: &Ctx, req: SetEnabledRequest) -> Result<SetEnabledResponse, OpError> {
    let root = &root_of(ctx)?;
    let err = |e| vault_error(e, root);
    // Read first: a corrupt file refuses before any directory is made.
    let mut cfg = read_vault_config(root).map_err(err)?;
    // Create before writing: a refused mkdir must not leave an enabled type with no directory.
    // Disable is a config write only; nothing here removes a path (V-11).
    let created = if req.enabled {
        create_type_dir(root, req.artifact_type.info().dir).map_err(err)?
    } else {
        false
    };
    cfg.set_enabled(req.artifact_type, req.enabled);
    write_vault_config(root, &cfg).map_err(err)?;
    Ok(SetEnabledResponse {
        artifact_type: req.artifact_type,
        enabled: req.enabled,
        created,
    })
}
