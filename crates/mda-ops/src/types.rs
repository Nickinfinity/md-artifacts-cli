//! `types.list` / `types.set_enabled`: the artifact types of the session vault, enabled per
//! `<vault>/.mda/config.toml` else by directory existence (V-12). Enabling creates the directory;
//! disabling never deletes it (V-11). Ports the settings panel's type toggles minus the delete.

use mda_core::registry::TypeInfo;
use mda_vault::config::{read_vault_config, write_vault_config};
use mda_vault::{DirState, create_type_dir, dir_state};

use crate::error::vault_error;
use crate::location::resolve;
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
    TYPES.iter().map(|t| row(ctx, t)).collect()
}

/// `enabled` is the type's own vault file's say (D-6), else whether its directory exists (V-12);
/// a root location always exists. One `resolve` per type.
fn row(ctx: &Ctx, t: &TypeInfo) -> Result<TypeRow, OpError> {
    let place = resolve(ctx, t.artifact_type)?;
    let root = &place.root;
    let cfg = read_vault_config(root).map_err(|e| vault_error(e, root))?;
    let dir_exists = place
        .base
        .as_deref()
        .is_none_or(|b| dir_state(root, b) == DirState::Present);
    Ok(TypeRow {
        artifact_type: t.artifact_type,
        dir: t.dir,
        enabled: cfg.enabled(t.artifact_type).unwrap_or(dir_exists),
        dir_exists,
    })
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
    let place = resolve(ctx, req.artifact_type)?;
    let root = &place.root;
    let err = |e| vault_error(e, root);
    // Read first: a corrupt file refuses before any directory is made.
    let mut cfg = read_vault_config(root).map_err(err)?;
    // Create before writing: a refused mkdir must not leave an enabled type with no directory.
    // Disable is a config write only; nothing here removes a path (V-11).
    let created = match (&place.base, req.enabled) {
        (Some(b), true) => create_type_dir(root, b).map_err(err)?,
        _ => false,
    };
    cfg.set_enabled(req.artifact_type, req.enabled);
    write_vault_config(root, &cfg).map_err(err)?;
    Ok(SetEnabledResponse {
        artifact_type: req.artifact_type,
        enabled: req.enabled,
        created,
    })
}
