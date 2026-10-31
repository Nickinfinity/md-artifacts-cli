//! THE engine configuration files (spec §12): the machine file `<config dir>/config.toml` (known
//! vaults, the default) and the vault file `<vault>/.mda/config.toml` (enabled types). Both are
//! untrusted: size-capped before parsing, corrupt files refused and never overwritten, unknown keys
//! preserved on write, writes through `write.rs` with a hash check.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use mda_core::registry::ArtifactType;

use crate::contain::MAX_PATH_BYTES;
use crate::{Root, VaultError, contain, content_hash, create_new, read_bounded, replace};

/// Largest config file read; checked on metadata before reading.
pub const MAX_CONFIG_BYTES: u64 = 65_536;
/// Most known vaults in the machine file.
pub const MAX_VAULTS: usize = 64;
/// The machine file's name inside the config dir.
pub const MACHINE_FILE: &str = "config.toml";
/// The vault file, vault-relative.
pub const VAULT_FILE: &str = ".mda/config.toml";

/// The machine file: `default`, `vaults`, every other key kept in `rest`, and the hash of the bytes
/// read (`None` when there was no file).
///
/// # Examples
///
/// ```
/// let c = mda_vault::config::MachineConfig::default();
/// assert!(c.default.is_none() && c.vaults.is_empty() && c.hash.is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MachineConfig {
    pub default: Option<String>,
    pub vaults: Vec<String>,
    pub rest: toml::Table,
    pub hash: Option<String>,
}

/// The vault file: the whole table (unknown keys kept) and the hash of the bytes read.
///
/// # Examples
///
/// ```
/// let c = mda_vault::config::VaultConfig::default();
/// assert!(c.table.is_empty() && c.hash.is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VaultConfig {
    pub table: toml::Table,
    pub hash: Option<String>,
}

impl VaultConfig {
    /// `types.<T>.enabled`, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_vault::config::VaultConfig;
    /// let c = VaultConfig::default();
    /// let _ = c.enabled(mda_core::registry::ArtifactType::Snippet);
    /// ```
    pub fn enabled(&self, t: ArtifactType) -> Option<bool> {
        self.table
            .get("types")?
            .get(t.as_str())?
            .get("enabled")?
            .as_bool()
    }

    /// Set `types.<T>.enabled`, creating the tables as needed and keeping sibling keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_vault::config::VaultConfig;
    /// let mut c = VaultConfig::default();
    /// c.set_enabled(mda_core::registry::ArtifactType::Command, true);
    /// ```
    pub fn set_enabled(&mut self, t: ArtifactType, on: bool) {
        // Shape was checked on read; a hand-built table that breaks it is replaced.
        let mut types = match self.table.remove("types") {
            Some(toml::Value::Table(x)) => x,
            _ => toml::Table::new(),
        };
        let mut entry = match types.remove(t.as_str()) {
            Some(toml::Value::Table(x)) => x,
            _ => toml::Table::new(),
        };
        entry.insert("enabled".into(), on.into());
        types.insert(t.as_str().into(), entry.into());
        self.table.insert("types".into(), types.into());
    }
}

/// The machine config dir: absolute `MDA_CONFIG_DIR` (relative → `None`, never falls through),
/// else per `os` (`std::env::consts::OS`): macOS `$HOME/Library/Application Support/mda`, Windows
/// `%APPDATA%\mda`, otherwise absolute `$XDG_CONFIG_HOME/mda` or `$HOME/.config/mda`. Empty values
/// count as unset; relative ones are skipped. `env` is the lookup (tests pass a map).
///
/// # Examples
///
/// ```
/// use mda_vault::config::config_dir;
/// let cfg = std::env::temp_dir(); // absolute on every OS
/// let d = config_dir("linux", |k| (k == "MDA_CONFIG_DIR").then(|| cfg.clone().into_os_string()));
/// assert_eq!(d, Some(cfg));
/// ```
pub fn config_dir(os: &str, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let get = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(v) = get("MDA_CONFIG_DIR") {
        // A relative override is a mistake to surface, never to paper over with the OS dir.
        return Some(PathBuf::from(v)).filter(|p| p.is_absolute());
    }
    let abs = |k: &str| get(k).map(PathBuf::from).filter(|p| p.is_absolute());
    match os {
        "macos" => abs("HOME").map(|h| h.join("Library/Application Support/mda")),
        "windows" => abs("APPDATA").map(|a| a.join("mda")),
        _ => abs("XDG_CONFIG_HOME")
            .map(|x| x.join("mda"))
            .or_else(|| abs("HOME").map(|h| h.join(".config/mda"))),
    }
}

/// Read the machine file in `dir`: missing dir or file → empty config, `hash: None`.
///
/// # Examples
///
/// ```
/// let c = mda_vault::config::read_machine(std::path::Path::new("/nonexistent-mda-doc")).unwrap();
/// assert!(c.vaults.is_empty());
/// ```
pub fn read_machine(dir: &Path) -> Result<MachineConfig, VaultError> {
    let path = dir.join(MACHINE_FILE);
    let Some(root) = machine_root(dir)? else {
        return Ok(MachineConfig::default());
    };
    match read_table(&root, Path::new(MACHINE_FILE), &path)? {
        None => Ok(MachineConfig::default()),
        Some((t, hash)) => machine_from(t, hash, &path),
    }
}

/// Write the machine file in `dir` (created if missing): `create_new` when `cfg.hash` is `None`,
/// else a hash-checked `replace`; a lost race is `Conflict`.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::config::{MachineConfig, write_machine};
/// write_machine(std::path::Path::new("/tmp/mda-cfg"), &MachineConfig::default()).unwrap();
/// ```
pub fn write_machine(dir: &Path, cfg: &MachineConfig) -> Result<(), VaultError> {
    let mut table = cfg.rest.clone();
    if let Some(d) = &cfg.default {
        table.insert("default".into(), d.clone().into());
    }
    table.insert("vaults".into(), cfg.vaults.clone().into());
    // Same-directory-or-create: the dir must exist before it can be a Root.
    std::fs::create_dir_all(dir).map_err(|e| crate::contain::io_err(dir, &e))?;
    let root = Root::new(dir)?;
    write_table(
        &root,
        Path::new(MACHINE_FILE),
        &dir.join(MACHINE_FILE),
        &table,
        cfg.hash.as_deref(),
    )
}

/// Read `<root>/.mda/config.toml`: missing `.mda` or file → empty config.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, config::read_vault_config};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// let c = read_vault_config(&root).unwrap();
/// # let _ = c;
/// ```
pub fn read_vault_config(root: &Root) -> Result<VaultConfig, VaultError> {
    let path = root.path().join(VAULT_FILE);
    if !mda_dir_exists(root)? {
        return Ok(VaultConfig::default());
    }
    match read_table(root, Path::new(VAULT_FILE), &path)? {
        None => Ok(VaultConfig::default()),
        Some((table, hash)) => {
            check_types(&table, &path)?;
            Ok(VaultConfig {
                table,
                hash: Some(hash),
            })
        }
    }
}

/// Write `<root>/.mda/config.toml` (`.mda/` created on first write), hash-checked like the machine
/// file.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, config::{VaultConfig, write_vault_config}};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// write_vault_config(&root, &VaultConfig::default()).unwrap();
/// ```
pub fn write_vault_config(root: &Root, cfg: &VaultConfig) -> Result<(), VaultError> {
    mda_dir_exists(root)?; // `.mda` a file / escaping link is refused before any write
    write_table(
        root,
        Path::new(VAULT_FILE),
        &root.path().join(VAULT_FILE),
        &cfg.table,
        cfg.hash.as_deref(),
    )
}

/// The config dir as a Root; a missing dir is `None` (no machine config yet).
fn machine_root(dir: &Path) -> Result<Option<Root>, VaultError> {
    match Root::new(dir) {
        Ok(r) => Ok(Some(r)),
        Err(VaultError::NotFound { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Does `<root>/.mda` exist as a directory? Missing → false; a file → `NotDir`; an escaping
/// symlink → `OutsideRoot`. Checked before any vault-file I/O.
fn mda_dir_exists(root: &Root) -> Result<bool, VaultError> {
    let dir = contain(root, Path::new(".mda"))?;
    match std::fs::metadata(&dir) {
        Ok(m) if m.is_dir() => Ok(true),
        Ok(_) => Err(VaultError::NotDir { path: dir }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(crate::contain::io_err(&dir, &e)),
    }
}

/// Read, cap, decode and parse one config file; `None` = absent. `err_path` names the file in
/// `ConfigInvalid`. The hash is of the bytes parsed (never re-read).
fn read_table(
    root: &Root,
    rel: &Path,
    err_path: &Path,
) -> Result<Option<(toml::Table, String)>, VaultError> {
    let invalid = || VaultError::ConfigInvalid {
        path: err_path.to_path_buf(),
    };
    let bytes = match read_bounded(root, rel, MAX_CONFIG_BYTES) {
        Ok(b) => b,
        Err(VaultError::NotFound { .. }) => return Ok(None),
        Err(VaultError::TooLarge { .. }) => return Err(invalid()),
        Err(e) => return Err(e),
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    let table = text.parse::<toml::Table>().map_err(|_| invalid())?;
    Ok(Some((table, content_hash(&bytes))))
}

/// Serialize and write: `create_new` when there was no file, else hash-checked `replace`. A lost
/// create race (or a file where none was read) is `Conflict`, never an overwrite.
fn write_table(
    root: &Root,
    rel: &Path,
    err_path: &Path,
    table: &toml::Table,
    hash: Option<&str>,
) -> Result<(), VaultError> {
    let text = toml::to_string(table).map_err(|_| VaultError::ConfigInvalid {
        path: err_path.to_path_buf(),
    })?;
    match hash {
        Some(h) => replace(root, rel, text.as_bytes(), h, MAX_CONFIG_BYTES),
        None => create_new(root, rel, text.as_bytes()).map_err(|e| match e {
            VaultError::Exists { path } => {
                let actual = read_bounded(root, rel, MAX_CONFIG_BYTES)
                    .map(|b| content_hash(&b))
                    .unwrap_or_default();
                VaultError::Conflict {
                    path,
                    expected: String::new(),
                    actual,
                }
            }
            other => other,
        }),
    }
}

/// Validate the machine file's shape (spec §12.1).
fn machine_from(
    mut t: toml::Table,
    hash: String,
    path: &Path,
) -> Result<MachineConfig, VaultError> {
    let invalid = || VaultError::ConfigInvalid {
        path: path.to_path_buf(),
    };
    let default = match t.remove("default") {
        None => None,
        Some(toml::Value::String(s)) => Some(s),
        Some(_) => return Err(invalid()),
    };
    let vaults = match t.remove("vaults") {
        None => Vec::new(),
        Some(toml::Value::Array(a)) => a
            .into_iter()
            .map(|v| match v {
                toml::Value::String(s) => Ok(s),
                _ => Err(invalid()),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err(invalid()),
    };
    let entries_ok = vaults
        .iter()
        .all(|v| v.len() <= MAX_PATH_BYTES && Path::new(v).is_absolute());
    let default_ok = default.as_ref().is_none_or(|d| vaults.contains(d));
    if vaults.len() > MAX_VAULTS || !entries_ok || !default_ok {
        return Err(invalid());
    }
    Ok(MachineConfig {
        default,
        vaults,
        rest: t,
        hash: Some(hash),
    })
}

/// Validate the vault file's shape: `types` a table of tables whose `enabled` is a bool.
fn check_types(t: &toml::Table, path: &Path) -> Result<(), VaultError> {
    let invalid = || VaultError::ConfigInvalid {
        path: path.to_path_buf(),
    };
    let Some(types) = t.get("types") else {
        return Ok(());
    };
    let types = types.as_table().ok_or_else(invalid)?;
    for entry in types.values() {
        let entry = entry.as_table().ok_or_else(invalid)?;
        if entry.get("enabled").is_some_and(|e| !e.is_bool()) {
            return Err(invalid());
        }
    }
    Ok(())
}
