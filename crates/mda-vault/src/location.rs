//! A type's storage location in the machine file (spec §15): `[types.<Type>] vault = "/abs"`,
//! `dir = "<segment>"` or `root = true`. The file is untrusted: every shape is checked before any
//! value reaches a path.

use std::path::Path;

use mda_core::naming::validate_dir_segment;
use mda_core::registry::ArtifactType;

use crate::VaultError;
use crate::config::MachineConfig;

/// Where inside its vault a type's files live.
///
/// # Examples
///
/// ```
/// use mda_vault::Where;
/// assert_ne!(Where::Default, Where::Root);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    /// No `dir`/`root` key: the registry dir, not pinned (D-5 b).
    Default,
    /// An explicit `dir` key: that directory, pinned.
    Dir(String),
    /// `root = true`: the vault root; the type has the vault to itself (D-3).
    Root,
}

/// One type's configured location. `vault: None` = the session vault (D-4).
///
/// # Examples
///
/// ```
/// use mda_vault::{Location, Where};
/// let l = Location { vault: None, place: Where::Root };
/// assert!(l.vault.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub vault: Option<String>,
    pub place: Where,
}

/// The location `cfg` configures for `t`, if any; a malformed table is `LocationInvalid`.
///
/// # Examples
///
/// ```
/// use mda_vault::{config::MachineConfig, location};
/// let cfg = MachineConfig::default();
/// assert_eq!(location(&cfg, mda_core::registry::ArtifactType::Snippet).unwrap(), None);
/// ```
pub fn location(cfg: &MachineConfig, t: ArtifactType) -> Result<Option<Location>, VaultError> {
    let bad = |reason| VaultError::LocationInvalid {
        ty: t.as_str().into(),
        reason,
    };
    let Some(types) = cfg.rest.get("types") else {
        return Ok(None);
    };
    let Some(entry) = types
        .as_table()
        .ok_or_else(|| bad("shape"))?
        .get(t.as_str())
    else {
        return Ok(None);
    };
    parse(entry.as_table().ok_or_else(|| bad("shape"))?).map_err(bad)
}

/// The keys of one `[types.<T>]` table as a location; `Err` is the reason code.
fn parse(e: &toml::Table) -> Result<Option<Location>, &'static str> {
    let vault = match e.get("vault") {
        None => None,
        Some(v) => Some(check_vault(v.as_str().ok_or("vault")?)?.to_owned()),
    };
    let dir = match e.get("dir") {
        None => None,
        Some(d) => Some(check_dir(d.as_str().ok_or("dir")?)?.to_owned()),
    };
    let root = match e.get("root") {
        None => None,
        Some(r) => Some(r.as_bool().ok_or("shape")?),
    };
    let place = match (dir, root) {
        (Some(_), Some(true)) => return Err("both"),
        (Some(d), _) => Where::Dir(d),
        (None, Some(true)) => Where::Root,
        (None, Some(false)) => return Err("neither"),
        (None, None) if vault.is_some() => Where::Default,
        (None, None) => return Ok(None),
    };
    Ok(Some(Location { vault, place }))
}

fn check_vault(v: &str) -> Result<&str, &'static str> {
    if Path::new(v).is_absolute() {
        Ok(v)
    } else {
        Err("vault")
    }
}

/// A leading dot is refused (D-14): `.obsidian` / `.mda` must never be a type dir.
fn check_dir(d: &str) -> Result<&str, &'static str> {
    if d.starts_with('.') || d.contains(['/', '\\']) || validate_dir_segment(d).is_err() {
        return Err("dir");
    }
    Ok(d)
}

/// Set (`Some`) or clear (`None`) `t`'s location in `cfg`, keeping every other key.
///
/// # Examples
///
/// ```
/// use mda_vault::{config::MachineConfig, set_location};
/// let mut cfg = MachineConfig::default();
/// assert!(set_location(&mut cfg, mda_core::registry::ArtifactType::Snippet, None).is_ok());
/// ```
pub fn set_location(
    cfg: &mut MachineConfig,
    t: ArtifactType,
    loc: Option<Location>,
) -> Result<(), VaultError> {
    let bad = |reason| VaultError::LocationInvalid {
        ty: t.as_str().into(),
        reason,
    };
    // Validate everything before the first mutation: a refusal leaves `cfg` byte-equal.
    if let Some(l) = &loc {
        if let Some(v) = &l.vault {
            check_vault(v).map_err(bad)?;
        }
        if let Where::Dir(d) = &l.place {
            check_dir(d).map_err(bad)?;
        }
    }
    let mut types = match cfg.rest.get("types") {
        None => toml::Table::new(),
        Some(v) => v.as_table().ok_or_else(|| bad("shape"))?.clone(),
    };
    let mut entry = match types.get(t.as_str()) {
        None => toml::Table::new(),
        Some(v) => v.as_table().ok_or_else(|| bad("shape"))?.clone(),
    };
    for k in ["vault", "dir", "root"] {
        entry.remove(k);
    }
    if let Some(l) = loc {
        if let Some(v) = l.vault {
            entry.insert("vault".into(), v.into());
        }
        match l.place {
            Where::Default => {}
            Where::Dir(d) => {
                entry.insert("dir".into(), d.into());
            }
            Where::Root => {
                entry.insert("root".into(), true.into());
            }
        }
    }
    if entry.is_empty() {
        types.remove(t.as_str());
    } else {
        types.insert(t.as_str().into(), entry.into());
    }
    if types.is_empty() {
        cfg.rest.remove("types");
    } else {
        cfg.rest.insert("types".into(), types.into());
    }
    Ok(())
}
