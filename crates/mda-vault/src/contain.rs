//! THE containment rule: canonicalize (symlinks resolved), then compare path **components**.
//! Ported from the extension's `isPathWithin`; checked immediately before the I/O it protects.

use std::io;
use std::path::{Component, Path, PathBuf};

use mda_core::naming::{NameError, validate_dir_segment};

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
#[derive(Debug, Clone)]
pub struct Root(PathBuf);

impl Root {
    /// Canonicalize `path` into a root; a missing directory is `NotFound`, a non-directory `NotDir`.
    pub fn new(path: &Path) -> Result<Self, VaultError> {
        let canon = path.canonicalize().map_err(|e| io_err(path, &e))?;
        if !std::fs::metadata(&canon).is_ok_and(|m| m.is_dir()) {
            return Err(VaultError::NotDir {
                path: path.to_path_buf(),
            });
        }
        Ok(Self(canon))
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

/// Strip the Windows verbatim prefix `fs::canonicalize` adds, for every path that leaves the
/// engine (responses, error params, stored config): `\\?\C:\x` -> `C:\x`, `\\?\UNC\s\x` ->
/// `\\s\x`. Anything else (every Unix path) is unchanged. Works on the string form, so it is
/// the same on every OS. The root stays canonical internally; containment never sees this.
///
/// The prefix is kept when stripping would change meaning: Win32 normalises a trailing dot or
/// space and device names away (`\\?\C:\v.` -> `C:\v`, a different directory), so a component that
/// is `edge_dot` / `edge_space` / `reserved` leaves the path verbatim. Only `\\?\X:\` (ASCII
/// letter) is a drive form; `\\?\C:foo` stays as is.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// assert_eq!(mda_vault::plain_path(Path::new("/x")), Path::new("/x"));
/// ```
pub fn plain_path(p: &Path) -> PathBuf {
    // `into_string` (not a lossy conversion): a non-UTF-8 path is returned untouched.
    let Ok(s) = p.as_os_str().to_owned().into_string() else {
        return p.to_path_buf();
    };
    let (plain, skip) = if let Some(r) = s.strip_prefix(r"\\?\UNC\") {
        (format!(r"\\{r}"), 2) // server and share are not directory names
    } else if let Some(r) = s.strip_prefix(r"\\?\")
        && let mut c = r.chars()
        && let (Some(d), Some(':'), Some('\\')) = (c.next(), c.next(), c.next())
        && d.is_ascii_alphabetic()
    {
        (r.to_owned(), 1) // the drive
    } else {
        return p.to_path_buf();
    };
    let unsafe_to_strip = plain
        .split('\\')
        .filter(|g| !g.is_empty())
        .skip(skip)
        .any(|g| {
            matches!(
                validate_dir_segment(g),
                Err(NameError::EdgeDot | NameError::EdgeSpace | NameError::Reserved)
            )
        });
    if unsafe_to_strip {
        p.to_path_buf()
    } else {
        PathBuf::from(plain)
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

#[cfg(test)]
mod plain_tests {
    use super::plain_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn strips_verbatim_prefixes_on_every_os() {
        let f = |s: &str| plain_path(Path::new(s));
        assert_eq!(f(r"\\?\C:\x"), PathBuf::from(r"C:\x"));
        assert_eq!(f(r"\\?\UNC\s\x"), PathBuf::from(r"\\s\x"));
        assert_eq!(f("/x"), PathBuf::from("/x"));
        assert_eq!(f(r"\\?\Volume{1}\x"), PathBuf::from(r"\\?\Volume{1}\x"));
        assert_eq!(f(r"\\?\C:\"), PathBuf::from(r"C:\"));
    }

    #[test]
    fn keeps_prefix_when_stripping_changes_meaning() {
        // Win32 would read these as a different directory without `\\?\`.
        for kept in [
            r"\\?\C:\v.",
            r"\\?\C:\v ",
            r"\\?\C:\CON",
            r"\\?\C:\a\nul.txt\b",
            r"\\?\UNC\s\share\v.",
            r"\\?\C:foo",
            r"\\?\1:\x",
            r"\\?\C:",
        ] {
            assert_eq!(plain_path(Path::new(kept)), PathBuf::from(kept), "{kept}");
        }
        // a dotted-but-valid segment and a server named like a device are still stripped
        assert_eq!(
            plain_path(Path::new(r"\\?\UNC\con\share\v.x")),
            PathBuf::from(r"\\con\share\v.x")
        );
    }
}
