//! Rename a vault file without clobbering (spec §14.2): every refusal before any directory is
//! created, the target published by hard link, the source removed only afterwards.

use std::io;
use std::path::{Component, Path};

use crate::contain::io_err;
use crate::write::{check_hash, split, sync_dir, with_temp};
use crate::{Root, VaultError, contain, content_hash, read_bounded, replace};

/// Refuse `..`, absolute and prefix parts before any I/O can probe outside the root.
fn normal_only(rel: &Path) -> Result<(), VaultError> {
    if rel.components().all(|c| matches!(c, Component::Normal(_))) {
        Ok(())
    } else {
        Err(VaultError::OutsideRoot {
            path: rel.to_path_buf(),
        })
    }
}

/// Move `from` to `to` (both vault-relative) when `from`'s content hash is `expected`: verbatim when
/// `bytes` is `None`, else `to` holds `bytes`. Never overwrites (`Exists`); a crash leaves `from`, or
/// both, never neither.
///
/// # Examples
///
/// ```no_run
/// let root = mda_vault::Root::new(std::path::Path::new("/vault")).unwrap();
/// let r = mda_vault::rename_new(&root, "a.md".as_ref(), "b.md".as_ref(), "h", None, 1 << 20);
/// assert!(r.is_err());
/// ```
pub fn rename_new(
    root: &Root,
    from: &Path,
    to: &Path,
    expected: &str,
    bytes: Option<&[u8]>,
    max_bytes: u64,
) -> Result<(), VaultError> {
    normal_only(from)?;
    normal_only(to)?;
    // ponytail: lstat→contain TOCTOU (as `delete`); upgrade: O_NOFOLLOW handles
    // lstat the UNRESOLVED `from`: `contain` resolves a final symlink, so acting on its result
    // would move the link's target.
    let from_joined = root.path().join(from);
    let meta = std::fs::symlink_metadata(&from_joined).map_err(|e| io_err(&from_joined, &e))?;
    if meta.file_type().is_symlink() {
        return Err(VaultError::NotRegular { path: from_joined });
    }
    let src = contain(root, from)?;
    let current = read_bounded(root, from, max_bytes)?;
    check_hash(&src, &current, expected)?;
    // `to`: a final symlink is refused (never published over); `contain` checks the nearest
    // existing ancestor, so a symlinked dir leading out is `OutsideRoot` before anything is created.
    let to_joined = root.path().join(to);
    let to_exists = match std::fs::symlink_metadata(&to_joined) {
        Ok(m) if m.file_type().is_symlink() => {
            return Err(VaultError::NotRegular { path: to_joined });
        }
        Ok(_) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(io_err(&to_joined, &e)),
    };
    let dst = contain(root, to)?;
    // Alias check BEFORE the exists check: on a case-folding FS the old name lstat-succeeds as `to`.
    // Canonical paths, never inodes: two hard-linked names would make `rename` a silent no-op.
    if dst == src {
        return case_alias(root, &src, to, &current, bytes, max_bytes);
    }
    if to_exists {
        return Err(VaultError::Exists { path: dst });
    }
    let (parent, _) = split(&dst, to)?;
    std::fs::create_dir_all(&parent).map_err(|e| io_err(&parent, &e))?;
    // Re-contain both right before the link/unlink: directories now exist, the canonical form is final.
    let dst = contain(root, to)?;
    let src = contain(root, from)?;
    let (parent, name) = split(&dst, to)?;
    log::debug!("rename {} -> {}", src.display(), dst.display());
    // hard_link, never rename: fails with AlreadyExists instead of clobbering a racing file, and
    // `from` stays until `to` is complete.
    let linked = match bytes {
        None => std::fs::hard_link(&src, &dst),
        Some(b) => with_temp(&parent, &name, b, |t| std::fs::hard_link(t, &dst)),
    };
    match linked {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(VaultError::Exists { path: dst });
        }
        Err(e) => return Err(io_err(&dst, &e)),
    }
    if let Err(e) = std::fs::remove_file(&src) {
        // Unlink can fail (Windows: file held open). `src` still holds the old bytes, so undo the
        // publish (best effort) rather than leave two files and an `Exists` on retry.
        let _ = std::fs::remove_file(&dst);
        return Err(io_err(&src, &e));
    }
    sync_dir(&parent);
    if let Some(p) = src.parent() {
        sync_dir(p);
    }
    Ok(())
}

/// `to` is `from` under another case/normalization: `fs::rename` changes the name in place (each
/// step atomic, nothing unlinked), then `replace` swaps in the new bytes.
fn case_alias(
    root: &Root,
    src: &Path,
    to: &Path,
    current: &[u8],
    bytes: Option<&[u8]>,
    max_bytes: u64,
) -> Result<(), VaultError> {
    // ponytail: only the file name changes; a directory-only case change (`Snippets/sub/a.md` →
    // `Snippets/SUB/a.md`) is a silent Ok no-op (D-7, directories are not renamed) — add a dir rename if asked.
    let (parent, _) = split(src, to)?;
    let name = to.file_name().ok_or_else(|| VaultError::OutsideRoot {
        path: to.to_path_buf(),
    })?;
    let target = parent.join(name);
    std::fs::rename(src, &target).map_err(|e| io_err(&target, &e))?;
    sync_dir(&parent);
    if let Some(b) = bytes {
        replace(root, to, b, &content_hash(current), max_bytes)?;
    }
    Ok(())
}
