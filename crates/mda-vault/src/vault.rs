//! Vault validation and the type directories: is a folder an Obsidian vault, which type dirs exist,
//! create one (create-only — no code path deletes a directory, V-11). Ports `vault.service.ts`
//! (`validateObsidianVault`, `detectVaultDirs`, `createVaultDirectory`); spec §12.1.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::contain::io_err;
use crate::{Root, VaultError, contain};

/// The state of one type directory inside a vault.
///
/// # Examples
///
/// ```
/// use mda_vault::DirState;
/// assert_ne!(DirState::Present, DirState::Blocked);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DirState {
    /// A directory (a symlink to a directory inside the vault counts).
    Present,
    /// Nothing at that name.
    Missing,
    /// Something else: a file, a FIFO, a symlink escaping the vault or dangling.
    Blocked,
}

/// Validate `path` as an Obsidian vault (spec §12.1): exists (`NotFound`), is a directory
/// (`NotDir`), holds a **directory** `.obsidian` (a symlink to one is accepted) else `NotObsidian`.
///
/// # Examples
///
/// ```
/// let e = mda_vault::validate_vault(std::path::Path::new("/nonexistent-mda-doc")).unwrap_err();
/// assert!(matches!(e, mda_vault::VaultError::NotFound { .. }));
/// ```
pub fn validate_vault(path: &Path) -> Result<Root, VaultError> {
    let root = Root::new(path)?;
    // `metadata` follows symlinks (a linked `.obsidian` is allowed); a file or dangling link is not a dir.
    if fs::metadata(root.path().join(".obsidian")).is_ok_and(|m| m.is_dir()) {
        Ok(root)
    } else {
        Err(VaultError::NotObsidian {
            path: path.to_path_buf(),
        })
    }
}

/// The state of type directory `dir` (a registry `dir`, one level) inside `root`.
///
/// # Examples
///
/// ```
/// use mda_vault::{Root, DirState, dir_state};
/// let root = Root::new(&std::env::temp_dir()).unwrap();
/// let _state: DirState = dir_state(&root, "Snippets");
/// ```
pub fn dir_state(root: &Root, dir: &str) -> DirState {
    let Ok(full) = contain(root, Path::new(dir)) else {
        return DirState::Blocked;
    };
    // `metadata` stats only: a FIFO is classified, never opened.
    match fs::metadata(full) {
        Ok(m) if m.is_dir() => DirState::Present,
        Err(e) if e.kind() == ErrorKind::NotFound => DirState::Missing,
        _ => DirState::Blocked,
    }
}

/// Create type directory `dir` (one level, contained); `Ok(true)` when created, `Ok(false)` when
/// it already was a directory. A file in its place is `NotDir`; an escaping symlink `OutsideRoot`.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, create_type_dir};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// let created = create_type_dir(&root, "Commands").unwrap();
/// # let _ = created;
/// ```
pub fn create_type_dir(root: &Root, dir: &str) -> Result<bool, VaultError> {
    // One level, contained right before the I/O; create-only, nothing here ever removes a path.
    let full = contain(root, Path::new(dir))?;
    log::debug!("mkdir {}", full.display());
    match fs::create_dir(&full) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            if fs::metadata(&full).is_ok_and(|m| m.is_dir()) {
                Ok(false)
            } else {
                Err(VaultError::NotDir { path: full })
            }
        }
        Err(e) => Err(io_err(&full, &e)),
    }
}
