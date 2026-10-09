//! THE containment rule: canonicalize (symlinks resolved), then compare path **components**.
//! Ported from the extension's `isPathWithin`; checked immediately before the I/O it protects.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::VaultError;

/// Longest joined path `contain` will look at; checked before any filesystem work.
pub const MAX_PATH_BYTES: usize = 4096;

/// A canonicalized directory that writes and reads are contained to.
///
/// # Examples
///
/// ```
/// use mda_vault::Root;
/// let root = Root::new(&std::env::temp_dir()).unwrap();
/// assert!(!root.path().as_os_str().is_empty());
/// ```
#[derive(Debug)]
pub struct Root(PathBuf);

impl Root {
    /// Canonicalize `path` into a root; a missing directory is `NotFound`.
    pub fn new(path: &Path) -> Result<Self, VaultError> {
        path.canonicalize().map(Self).map_err(|e| io_err(path, &e))
    }

    /// The canonical root path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Resolve `candidate` (relative to the root, or absolute) and prove it stays inside.
///
/// # Examples
///
/// ```
/// use mda_vault::{Root, contain};
/// let root = Root::new(&std::env::temp_dir()).unwrap();
/// assert!(contain(&root, std::path::Path::new("a.md")).is_ok());
/// ```
pub fn contain(root: &Root, candidate: &Path) -> Result<PathBuf, VaultError> {
    let outside = || VaultError::OutsideRoot {
        path: candidate.to_path_buf(),
    };
    let joined = root.path().join(candidate);
    // Limit before work: no legitimate vault path is this long.
    if joined.as_os_str().len() > MAX_PATH_BYTES {
        return Err(outside());
    }
    // Nearest existing ancestor. `symlink_metadata` (not `exists`) so a dangling link counts as
    // existing and is then refused by `canonicalize` below.
    let anchor = joined
        .ancestors()
        .find(|a| std::fs::symlink_metadata(a).is_ok())
        .ok_or_else(outside)?;
    // A dangling symlink cannot be proven contained: reject.
    let mut resolved = anchor.canonicalize().map_err(|_| outside())?;
    for c in joined
        .strip_prefix(anchor)
        .map_err(|_| outside())?
        .components()
    {
        match c {
            Component::Normal(n) => resolved.push(n),
            _ => return Err(outside()),
        }
    }
    if resolved.starts_with(root.path()) {
        Ok(resolved)
    } else {
        Err(outside())
    }
}

/// Map an I/O failure on `path` to the vault error shape.
pub(crate) fn io_err(path: &Path, e: &io::Error) -> VaultError {
    if e.kind() == io::ErrorKind::NotFound {
        VaultError::NotFound {
            path: path.to_path_buf(),
        }
    } else {
        VaultError::Io {
            path: path.to_path_buf(),
            kind: e.kind(),
        }
    }
}
