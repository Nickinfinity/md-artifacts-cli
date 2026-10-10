//! Vault writes: atomic (same-directory temp file → fsync → rename, W-2/§14.A), contained, and
//! guarded by the SHA-256 content hash the client last read (P-7). The only hash function.

use std::ffi::OsStr;
use std::io::{self, Write as _};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicUsize, Ordering};

use sha2::{Digest, Sha256};

use crate::contain::io_err;
use crate::{Root, VaultError, contain, read_bounded};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Write `bytes` to a hidden same-directory temp file, fsync it, hand it to `publish`, and always
/// remove it afterwards (success or failure). The name never ends in `.md`, so listings skip it.
fn with_temp(
    parent: &Path,
    name: &OsStr,
    bytes: &[u8],
    publish: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    tmp_name.push(format!(".mda-tmp-{}-{n}", std::process::id()));
    let tmp = parent.join(tmp_name);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let res = f
        .write_all(bytes)
        .and_then(|()| f.sync_all())
        .and_then(|()| publish(&tmp));
    drop(f);
    // After a rename the temp is already gone (NotFound, fine); after a hard link or any error it
    // is still there and must not linger.
    let _ = std::fs::remove_file(&tmp);
    res
}

/// Make the rename/link durable. No-op off unix (directories cannot be opened there).
fn sync_dir(parent: &Path) {
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(parent) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = parent;
}

/// Split a contained target into (parent, file name); a root-level or empty path is refused.
fn split(path: &Path, rel: &Path) -> Result<(std::path::PathBuf, std::ffi::OsString), VaultError> {
    match (path.parent(), path.file_name()) {
        (Some(p), Some(n)) => Ok((p.to_path_buf(), n.to_os_string())),
        _ => Err(VaultError::OutsideRoot {
            path: rel.to_path_buf(),
        }),
    }
}

/// SHA-256 of `bytes`, lower-case hex: the content hash every write is checked against.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     mda_vault::content_hash(b""),
///     "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
/// );
/// ```
pub fn content_hash(bytes: &[u8]) -> String {
    use std::fmt::Write;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}"); // writing to a String cannot fail
            s
        })
}

/// Create `rel` with `bytes`, creating missing directories; never overwrites (`Exists`).
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, create_new};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// create_new(&root, std::path::Path::new("Templates/a.md"), b"x").unwrap();
/// ```
pub fn create_new(root: &Root, rel: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let target = contain(root, rel)?;
    let (parent, _) = split(&target, rel)?;
    std::fs::create_dir_all(&parent).map_err(|e| io_err(&parent, &e))?;
    // Re-contain after creating directories: the canonical parent now exists.
    let target = contain(root, rel)?;
    let (parent, name) = split(&target, rel)?;
    log::debug!("write {}", target.display());
    // hard_link, never rename: it fails with AlreadyExists instead of clobbering a file that
    // appeared since the check.
    let res = with_temp(&parent, &name, bytes, |t| std::fs::hard_link(t, &target));
    match res {
        Ok(()) => {
            sync_dir(&parent);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            Err(VaultError::Exists { path: target })
        }
        Err(e) => Err(io_err(&target, &e)),
    }
}

/// Replace `rel` with `bytes` when its current content hashes to `expected` (`Conflict`
/// otherwise); the current file is read bounded by `max_bytes`.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, replace};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// replace(&root, std::path::Path::new("Templates/a.md"), b"y", "e3b0…", 1 << 20).unwrap();
/// ```
pub fn replace(
    root: &Root,
    rel: &Path,
    bytes: &[u8],
    expected: &str,
    max_bytes: u64,
) -> Result<(), VaultError> {
    let target = contain(root, rel)?;
    let current = read_bounded(root, rel, max_bytes)?; // regular file, size-bounded
    check_hash(&target, &current, expected)?;
    let (parent, name) = split(&target, rel)?;
    log::debug!("write {}", target.display());
    // ponytail: hash check → rename TOCTOU; upgrade: lock file or O_EXCL swap
    with_temp(&parent, &name, bytes, |t| std::fs::rename(t, &target))
        .map_err(|e| io_err(&target, &e))?;
    sync_dir(&parent);
    Ok(())
}

fn check_hash(path: &Path, current: &[u8], expected: &str) -> Result<(), VaultError> {
    let actual = content_hash(current);
    if actual == expected {
        return Ok(());
    }
    Err(VaultError::Conflict {
        path: path.to_path_buf(),
        expected: expected.to_owned(),
        actual,
    })
}

/// Delete `rel` when its content hashes to `expected`; a symlink is refused (`NotRegular`) and its
/// target never touched (W-1).
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, delete};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// delete(&root, std::path::Path::new("Templates/a.md"), "e3b0…", 1 << 20).unwrap();
/// ```
pub fn delete(root: &Root, rel: &Path, expected: &str, max_bytes: u64) -> Result<(), VaultError> {
    // `..`/absolute parts would let this lstat probe outside the root before `contain` runs.
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(VaultError::OutsideRoot {
            path: rel.to_path_buf(),
        });
    }
    // ponytail: lstat→contain TOCTOU gap (a swap between them); upgrade: fstat an O_NOFOLLOW handle
    // lstat the UNRESOLVED path first: `contain` resolves a final symlink, so unlinking its result
    // would act on the link's target.
    let joined = root.path().join(rel);
    let meta = std::fs::symlink_metadata(&joined).map_err(|e| io_err(&joined, &e))?;
    if meta.file_type().is_symlink() {
        return Err(VaultError::NotRegular { path: joined });
    }
    let target = contain(root, rel)?;
    let current = read_bounded(root, rel, max_bytes)?;
    check_hash(&target, &current, expected)?;
    log::debug!("delete {}", target.display());
    std::fs::remove_file(&target).map_err(|e| io_err(&target, &e))?;
    if let Some(p) = target.parent() {
        sync_dir(p);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("mda-wt-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn leftovers(d: &Path) -> usize {
        std::fs::read_dir(d)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".mda-tmp-")
            })
            .count()
    }

    #[test]
    fn failed_publish_removes_temp() {
        let d = dir("fail");
        let r = with_temp(&d, OsStr::new("a.md"), b"x", |_| Err(io::Error::other("x")));
        assert!(r.is_err());
        assert_eq!(leftovers(&d), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn temp_name_is_hidden_and_not_md() {
        let d = dir("name");
        with_temp(&d, OsStr::new("a.md"), b"x", |t| {
            let n = t.file_name().unwrap().to_string_lossy().into_owned();
            assert!(n.starts_with('.') && !n.ends_with(".md"), "{n}");
            Ok(())
        })
        .unwrap();
        assert_eq!(leftovers(&d), 0);
        let _ = std::fs::remove_dir_all(&d);
    }
}
