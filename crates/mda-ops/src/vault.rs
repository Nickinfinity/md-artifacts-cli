//! `vault.inspect` / `vault.select` / `vault.list` / `vault.use`, `config.get`, and the session
//! resolver every front-end starts from (spec §12.1, plan §3.7 V-2…V-4, V-9, V-10). Ports
//! `vault.service.ts` (validate, detect dirs) and the settings panel's folder selection.

use std::path::{Path, PathBuf};

use mda_core::registry::TYPES;
use mda_vault::config::{MACHINE_FILE, MAX_VAULTS, MachineConfig, read_machine, write_machine};
use mda_vault::{DirState, VaultError, create_type_dir, dir_state, validate_vault};

use crate::error::vault_error;
use crate::{ArtifactType, Ctx, OpError, Root, Source, error};

pub use mda_vault::config::config_dir;

/// `vault.inspect` request: an absolute folder path.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InspectRequest {
    pub path: String,
}

/// One type directory of an inspected folder.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirRow {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub dir: &'static str,
    pub state: DirState,
}

/// `vault.inspect` response: the canonical path, whether it is a vault, every type dir's state,
/// and the default-enabled types whose dir is missing.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectResponse {
    pub path: String,
    pub is_obsidian_vault: bool,
    pub dirs: Vec<DirRow>,
    pub missing_defaults: Vec<ArtifactType>,
}

fn yes() -> bool {
    true
}

/// `vault.select` request: `createDefaults` (default `true`) creates missing default type dirs.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SelectRequest {
    pub path: String,
    #[serde(default = "yes")]
    pub create_defaults: bool,
}

/// `vault.select` response: the canonical vault path and the type dirs created.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectResponse {
    pub path: String,
    pub created: Vec<String>,
}

/// `vault.list` request (no fields).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ListRequest {}

/// One known vault and whether it still validates.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownVault {
    pub path: String,
    pub available: bool,
}

/// `vault.list` response.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub vaults: Vec<KnownVault>,
}

/// `vault.use` request: a known vault's path.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UseRequest {
    pub path: String,
}

/// `vault.use` response: the canonical path now in use.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UseResponse {
    pub path: String,
}

/// `config.get` request (no fields).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConfigGetRequest {}

/// `config.get` response: session state only (no I/O).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGetResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault: Option<String>,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_error: Option<OpError>,
}

/// Build the session context: `--vault` (fatal if invalid; machine file not read) > a valid stored
/// default > none. A stale or corrupt default never fails: it becomes `default_error` (V-3).
/// `config_dir` is always kept.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::vault::resolve(None, None).unwrap();
/// assert!(ctx.root.borrow().is_none());
/// ```
pub fn resolve(flag: Option<&Path>, config_dir: Option<PathBuf>) -> Result<Ctx, OpError> {
    let ctx = Ctx::new(flag.map(open_vault).transpose()?);
    let ctx = match config_dir {
        Some(d) => ctx.with_config_dir(d),
        None => ctx,
    };
    if flag.is_none() {
        load_default(&ctx);
    }
    Ok(ctx)
}

/// Adopt the stored default, or record why not (V-3): never fails the process, never writes.
fn load_default(ctx: &Ctx) {
    let Some(dir) = &ctx.config_dir else { return };
    let cfg = match read_machine(dir) {
        Ok(c) => c,
        Err(e) => {
            ctx.default_error.replace(Some(e.into()));
            return;
        }
    };
    let Some(d) = cfg.default else { return };
    match open_vault(Path::new(&d)) {
        Ok(root) => switch(ctx, root),
        Err(_) => {
            ctx.default_error
                .replace(Some(OpError::new(error::VAULT_UNAVAILABLE).with("path", d)));
        }
    }
}

fn not_absolute() -> OpError {
    OpError::new(error::OP_BAD_REQUEST).with("reason", "path must be absolute")
}

fn canon(r: &Root) -> String {
    // ponytail: lossy on non-UTF-8 paths (a mangled stored path then fails to open as
    // `path.not_found`); refuse such vaults with `op.bad_request` if one ever appears.
    r.path().display().to_string()
}

/// The machine config dir and its parsed file; no dir at all is `config.invalid` with `path: ""`.
fn machine(ctx: &Ctx) -> Result<(PathBuf, MachineConfig), OpError> {
    let dir = ctx
        .config_dir
        .clone()
        .ok_or_else(|| OpError::new(error::CONFIG_INVALID).with("path", ""))?;
    let cfg = read_machine(&dir)?;
    Ok((dir, cfg))
}

/// Make `canon` the default (adding it when new) and write the machine file: the only config
/// write in this module, hash-checked by `write_machine`.
fn persist_default(dir: &Path, mut cfg: MachineConfig, canon: String) -> Result<(), OpError> {
    if !cfg.vaults.contains(&canon) {
        cfg.vaults.push(canon.clone());
    }
    cfg.default = Some(canon);
    Ok(write_machine(dir, &cfg)?)
}

/// Switch the session to `root`. The old root is swapped out in one statement: no `borrow()` is
/// alive here (a double borrow would panic the second time a session selects).
fn switch(ctx: &Ctx, root: Root) {
    ctx.root.replace(Some(root));
    ctx.source.set(Source::Default);
    ctx.default_error.take();
}

/// Create every default-enabled type dir that is not already a directory (a blocked one errors).
fn create_defaults(root: &Root) -> Result<Vec<String>, OpError> {
    let mut created = Vec::new();
    for t in TYPES.iter().filter(|t| t.default_enabled) {
        if dir_state(root, t.dir) != DirState::Present
            && create_type_dir(root, t.dir).map_err(|e| vault_error(e, root))?
        {
            created.push(t.dir.to_owned());
        }
    }
    Ok(created)
}

/// Open a vault the client named: absolute (`op.bad_request`), then spec §12.1 validation.
///
/// # Examples
///
/// ```
/// let missing = std::env::temp_dir().join("mda-nonexistent-doc"); // absolute on every OS
/// let e = mda_ops::vault::open_vault(&missing).unwrap_err();
/// assert_eq!(e.code, "path.not_found");
/// ```
pub fn open_vault(path: &Path) -> Result<Root, OpError> {
    if !path.is_absolute() {
        return Err(not_absolute());
    }
    Ok(validate_vault(path)?)
}

/// `vault.inspect`: read-only report on a folder.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vault::{inspect, InspectRequest}};
/// let r = inspect(&Ctx::new(None), InspectRequest { path: "/nonexistent-mda-doc".into() });
/// assert!(r.is_err());
/// ```
pub fn inspect(_ctx: &Ctx, req: InspectRequest) -> Result<InspectResponse, OpError> {
    let path = Path::new(&req.path);
    if !path.is_absolute() {
        return Err(not_absolute());
    }
    // A folder without `.obsidian` is a report (`false`), not an error; the other refusals are.
    let (root, is_vault) = match validate_vault(path) {
        Ok(r) => (r, true),
        Err(VaultError::NotObsidian { .. }) => (Root::new(path)?, false),
        Err(e) => return Err(e.into()),
    };
    let dirs: Vec<DirRow> = TYPES
        .iter()
        .map(|t| DirRow {
            artifact_type: t.artifact_type,
            dir: t.dir,
            state: dir_state(&root, t.dir),
        })
        .collect();
    let missing_defaults = TYPES
        .iter()
        .zip(&dirs)
        .filter(|(t, d)| t.default_enabled && d.state == DirState::Missing)
        .map(|(t, _)| t.artifact_type)
        .collect();
    Ok(InspectResponse {
        path: canon(&root),
        is_obsidian_vault: is_vault,
        dirs,
        missing_defaults,
    })
}

/// `vault.select`: validate, create default dirs, remember, make default, switch the session.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vault::{select, SelectRequest}};
/// let r = select(&Ctx::new(None), SelectRequest { path: "/nonexistent-mda-doc".into(), create_defaults: true });
/// assert!(r.is_err());
/// ```
pub fn select(ctx: &Ctx, req: SelectRequest) -> Result<SelectResponse, OpError> {
    let root = open_vault(Path::new(&req.path))?;
    let (dir, cfg) = machine(ctx)?;
    let path = canon(&root);
    // Refuse before any dir is created: a full or corrupt machine file must leave the vault alone.
    if cfg.vaults.len() >= MAX_VAULTS && !cfg.vaults.contains(&path) {
        return Err(OpError::new(error::CONFIG_INVALID)
            .with("path", dir.join(MACHINE_FILE).display().to_string()));
    }
    let created = if req.create_defaults {
        create_defaults(&root)?
    } else {
        Vec::new()
    };
    persist_default(&dir, cfg, path.clone())?;
    switch(ctx, root);
    Ok(SelectResponse { path, created })
}

/// `vault.list`: known vaults and the default.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vault::{list, ListRequest}};
/// let r = list(&Ctx::new(None), ListRequest {});
/// assert!(r.is_err());
/// ```
pub fn list(ctx: &Ctx, _req: ListRequest) -> Result<ListResponse, OpError> {
    let (_, cfg) = machine(ctx)?;
    let vaults = cfg
        .vaults
        .into_iter()
        .map(|path| KnownVault {
            available: validate_vault(Path::new(&path)).is_ok(),
            path,
        })
        .collect();
    Ok(ListResponse {
        default: cfg.default,
        vaults,
    })
}

/// `vault.use`: make a known vault the default and switch the session.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vault::{use_vault, UseRequest}};
/// let r = use_vault(&Ctx::new(None), UseRequest { path: "/nonexistent-mda-doc".into() });
/// assert!(r.is_err());
/// ```
pub fn use_vault(ctx: &Ctx, req: UseRequest) -> Result<UseResponse, OpError> {
    let root = open_vault(Path::new(&req.path))?;
    let (dir, cfg) = machine(ctx)?;
    let path = canon(&root);
    if !cfg.vaults.contains(&path) {
        return Err(OpError::new(error::VAULT_UNKNOWN).with("path", req.path));
    }
    persist_default(&dir, cfg, path.clone())?;
    switch(ctx, root);
    Ok(UseResponse { path })
}

/// `config.get`: the session vault, its source, the config dir, the deferred default error.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vault::{get, ConfigGetRequest}};
/// let _ = get(&Ctx::new(None), ConfigGetRequest {});
/// ```
pub fn get(ctx: &Ctx, req: ConfigGetRequest) -> Result<ConfigGetResponse, OpError> {
    let _ = req;
    Ok(ConfigGetResponse {
        vault: ctx.root.borrow().as_ref().map(canon),
        source: ctx.source.get(),
        config_dir: ctx.config_dir.as_ref().map(|d| d.display().to_string()),
        default_error: ctx.default_error.borrow().clone(),
    })
}
