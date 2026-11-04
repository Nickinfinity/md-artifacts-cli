//! `types.list` / `types.set_enabled`: the artifact types of the session vault, enabled per
//! `<vault>/.mda/config.toml` else by directory existence (V-12). Enabling creates the directory;
//! disabling never deletes it (V-11). Ports the settings panel's type toggles minus the delete.

use mda_core::registry::TypeInfo;
use mda_vault::config::{read_vault_config, write_vault_config};
use mda_vault::{DirState, create_type_dir, dir_state};

use crate::artifact::TreeError;
use crate::error::vault_error;
use crate::error::{ARTIFACT_BAD_PATH, CONFIG_LOCATION_CONFLICT, CONFIG_LOCATION_INVALID};
use crate::location::{
    LocationSpec, LocationView, check_location, persist_location, place_for, resolve,
};
use crate::vault::machine;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_exists: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationView>,
    /// Set instead of the three fields above when this type's own location is unusable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TreeError>,
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
    // A broken location of one type is that row's error; everything else is op-level.
    let place = match resolve(ctx, t.artifact_type) {
        Ok(p) => p,
        Err(e)
            if [
                CONFIG_LOCATION_CONFLICT,
                CONFIG_LOCATION_INVALID,
                ARTIFACT_BAD_PATH,
            ]
            .contains(&e.code) =>
        {
            return Ok(TypeRow {
                artifact_type: t.artifact_type,
                dir: t.dir,
                enabled: None,
                dir_exists: None,
                location: None,
                error: Some(TreeError { code: e.code }),
            });
        }
        Err(e) => return Err(e),
    };
    let root = &place.root;
    let cfg = read_vault_config(root).map_err(|e| vault_error(e, root))?;
    let dir_exists = place
        .base
        .as_deref()
        .is_none_or(|b| dir_state(root, b) == DirState::Present);
    Ok(TypeRow {
        artifact_type: t.artifact_type,
        dir: t.dir,
        enabled: Some(cfg.enabled(t.artifact_type).unwrap_or(dir_exists)),
        dir_exists: Some(dir_exists),
        location: Some(LocationView::of(&place)),
        error: None,
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

/// `types.set_location` request: the type and the fields of a `LocationSpec` (spelled out: serde
/// cannot combine `flatten` with `deny_unknown_fields`). No location field resets to the default.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetLocationRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub vault: Option<String>,
    pub dir: Option<String>,
    pub root: Option<bool>,
}

/// `types.set_location` response: where the type now lives.
#[derive(Debug, serde::Serialize)]
pub struct SetLocationResponse {
    pub location: LocationView,
}

/// `types.set_location`: store (or reset) a type's location without moving files. Order: shape,
/// D-3 and vault checks, dir pin, create-only dir (only when the target vault enables the type,
/// D-17), then the one machine-file write; any refusal leaves the file untouched.
///
/// # Examples
///
/// ```
/// use mda_ops::{ArtifactType, Ctx, types::{set_location, SetLocationRequest}};
/// let req = SetLocationRequest { artifact_type: ArtifactType::Snippet, vault: None, dir: None, root: None };
/// assert_eq!(set_location(&Ctx::new(None), req).unwrap_err().code, "config.invalid");
/// ```
pub fn set_location(ctx: &Ctx, req: SetLocationRequest) -> Result<SetLocationResponse, OpError> {
    let t = req.artifact_type;
    let spec = LocationSpec {
        vault: req.vault,
        dir: req.dir,
        root: req.root,
    };
    let loc = spec.to_location(t)?;
    let (dir, cfg) = machine(ctx)?;
    check_location(ctx, &cfg, t, loc.as_ref())?;
    let place = place_for(ctx, t, loc.as_ref())?;
    if let Some(b) = &place.base {
        let root = &place.root;
        let enabled = read_vault_config(root)
            .map_err(|e| vault_error(e, root))?
            .enabled(t);
        if enabled == Some(true) {
            create_type_dir(root, b).map_err(|e| vault_error(e, root))?;
        }
    }
    persist_location(&dir, cfg, t, loc)?;
    Ok(SetLocationResponse {
        location: LocationView::of(&place),
    })
}
