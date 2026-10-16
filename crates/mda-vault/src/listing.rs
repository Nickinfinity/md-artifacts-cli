//! One level of a vault directory: sub-directory names and `.md` file names, sorted (Q-F).

use std::fs;
use std::path::Path;

use crate::contain::io_err;
use crate::{Root, VaultError, WalkLimit, contain, read_bounded};

/// The names directly under one directory.
///
/// # Examples
///
/// ```
/// let l = mda_vault::DirListing::default();
/// assert!(l.dirs.is_empty() && l.files.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirListing {
    pub dirs: Vec<String>,
    pub files: Vec<String>,
}

/// List `rel` (relative to `root`): contained first, then one `read_dir`. Entries are classified
/// with `fs::metadata` (follows links; never opens, so a FIFO cannot block). A symlinked directory
/// shows up in `dirs`; `contain` refuses it on the next call if it leaves the root.
///
/// # Examples
///
/// ```
/// use mda_vault::{Root, list_dir};
/// let root = Root::new(&std::env::temp_dir()).unwrap();
/// let _ = list_dir(&root, std::path::Path::new("."));
/// ```
pub fn list_dir(root: &Root, rel: &Path) -> Result<DirListing, VaultError> {
    let abs = contain(root, rel)?;
    log::debug!("list {}", abs.display());
    let meta = fs::metadata(&abs).map_err(|e| io_err(&abs, &e))?;
    if !meta.is_dir() {
        return Err(VaultError::NotFound { path: abs });
    }
    let mut out = DirListing::default();
    for entry in fs::read_dir(&abs).map_err(|e| io_err(&abs, &e))? {
        let entry = entry.map_err(|e| io_err(&abs, &e))?;
        // ponytail: names travel as strings; non-UTF-8 skipped — carry bytes if a client needs them
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        // Error (dangling link) counts as not-a-dir, so a `.md` link still lists as a file.
        if fs::metadata(entry.path()).is_ok_and(|m| m.is_dir()) {
            out.dirs.push(name);
        } else if name.ends_with(".md") {
            out.files.push(name);
        }
    }
    sort_names(&mut out.dirs, |n| n);
    sort_names(&mut out.files, |n| n);
    Ok(out)
}

/// Case-insensitive order with the exact name as tiebreak (Q-F).
fn sort_names<T>(v: &mut [T], name: impl Fn(&T) -> &str) {
    v.sort_by_cached_key(|t| (name(t).to_lowercase(), name(t).to_owned()));
}

/// Deepest directory level entered below the start (the start is level 0).
pub const MAX_WALK_DEPTH: usize = 32;
/// Work items per walk: every `.md` file met and every directory entered (the start excluded).
pub const MAX_WALK_FILES: usize = 10_000;
/// Bytes read per walk: each file's size is added (and checked) before it is read.
pub const MAX_WALK_BYTES: u64 = 64 << 20;

/// Every `.md` under `start` (vault-relative POSIX), depth-first; `visit(rel, bytes-or-error)` once per
/// file. Symlinked directories are never descended; limits refuse the whole call (spec §13.2).
///
/// # Examples
///
/// ```
/// let root = mda_vault::Root::new(&std::env::temp_dir()).unwrap();
/// let r = mda_vault::walk_md(&root, "mda-no-such-dir-walk", 1024, |_, _| {});
/// assert!(r.is_ok());
/// ```
pub fn walk_md<F: FnMut(&str, Result<Vec<u8>, VaultError>)>(
    root: &Root,
    start: &str,
    max_file: u64,
    visit: F,
) -> Result<(), VaultError> {
    let abs = contain(root, Path::new(start))?;
    // A missing start, or one that is not a directory, is an empty walk (TS `catch → return`).
    if !fs::metadata(&abs).is_ok_and(|m| m.is_dir()) {
        return Ok(());
    }
    let mut st = Walk {
        root,
        max_file,
        files: 0,
        bytes: 0,
        visit,
    };
    walk_dir(&mut st, start, 0)
}

/// Walk state: the work counters are shared by the whole call.
struct Walk<'a, F> {
    root: &'a Root,
    max_file: u64,
    files: usize,
    bytes: u64,
    visit: F,
}

/// Count one work item; refuse before the work when the cap is passed.
fn count<F>(st: &mut Walk<'_, F>) -> Result<(), VaultError> {
    st.files += 1;
    if st.files > MAX_WALK_FILES {
        return Err(VaultError::Limit {
            limit: WalkLimit::Files,
            max: MAX_WALK_FILES as u64,
        });
    }
    Ok(())
}

/// One directory: its `.md` files first, then its subdirectories, each in `sort_names` order.
/// Only the start (`level == 0`) returns I/O failures; below it they become a `visit` entry.
fn walk_dir<F: FnMut(&str, Result<Vec<u8>, VaultError>)>(
    st: &mut Walk<'_, F>,
    rel: &str,
    level: usize,
) -> Result<(), VaultError> {
    let mut files: Vec<(String, bool, fs::DirEntry)> = Vec::new();
    let mut dirs: Vec<String> = Vec::new();
    // Contain again right before `read_dir`: a dir swapped for a symlink since classification.
    let listed = contain(st.root, Path::new(rel)).and_then(|abs| {
        for entry in fs::read_dir(&abs).map_err(|e| io_err(&abs, &e))? {
            let entry = entry.map_err(|e| io_err(&abs, &e))?;
            // `file_type` comes from the dirent and does not follow links.
            let ft = entry.file_type().map_err(|e| io_err(&abs, &e))?;
            // ponytail: non-UTF-8 names skipped, as in `list_dir`
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if ft.is_dir() {
                dirs.push(name);
            } else if name.ends_with(".md") {
                files.push((name, ft.is_symlink(), entry));
            } else {
                continue; // other names, symlinks included, cost nothing
            }
            // Per-dir memory is bounded before sorting.
            if st.files + files.len() + dirs.len() > MAX_WALK_FILES {
                return Err(VaultError::Limit {
                    limit: WalkLimit::Files,
                    max: MAX_WALK_FILES as u64,
                });
            }
        }
        Ok(())
    });
    match listed {
        Err(e @ VaultError::Limit { .. }) => return Err(e),
        Err(e) if level == 0 => return Err(e),
        Err(e) => {
            (st.visit)(rel, Err(e));
            return Ok(());
        }
        Ok(()) => {}
    }
    sort_names(&mut files, |f| &f.0);
    sort_names(&mut dirs, |d| d);
    for (name, is_link, entry) in files {
        count(st)?;
        walk_file(st, &format!("{rel}/{name}"), is_link, &entry)?;
    }
    for name in dirs {
        count(st)?;
        if level + 1 > MAX_WALK_DEPTH {
            return Err(VaultError::Limit {
                limit: WalkLimit::Depth,
                max: MAX_WALK_DEPTH as u64,
            });
        }
        walk_dir(st, &format!("{rel}/{name}"), level + 1)?;
    }
    Ok(())
}

/// One `.md` entry: sized (never following an uncontained link) before it is read.
fn walk_file<F: FnMut(&str, Result<Vec<u8>, VaultError>)>(
    st: &mut Walk<'_, F>,
    rel: &str,
    is_link: bool,
    entry: &fs::DirEntry,
) -> Result<(), VaultError> {
    let abs = st.root.path().join(rel);
    let meta = if is_link {
        // A link out must not leak its target's size or spend the byte budget.
        match contain(st.root, Path::new(rel)) {
            Ok(real) => fs::metadata(&real),
            Err(e) => {
                (st.visit)(rel, Err(e));
                return Ok(());
            }
        }
    } else {
        // `DirEntry::metadata` does not follow and is taken relative to the open directory, so a
        // parent swapped for an outside link after `read_dir` cannot redirect the stat.
        entry.metadata()
    };
    let meta = match meta {
        Ok(m) if m.is_dir() => return Ok(()), // a link to a directory is never descended
        Ok(m) => m,
        Err(e) => {
            (st.visit)(rel, Err(io_err(&abs, &e)));
            return Ok(());
        }
    };
    let len = meta.len();
    if len > st.max_file {
        let e = VaultError::TooLarge {
            path: abs,
            size: len,
            max: st.max_file,
        };
        (st.visit)(rel, Err(e));
        return Ok(());
    }
    let over = || VaultError::Limit {
        limit: WalkLimit::Bytes,
        max: MAX_WALK_BYTES,
    };
    if st.bytes.saturating_add(len) > MAX_WALK_BYTES {
        return Err(over());
    }
    let res = read_bounded(st.root, Path::new(rel), st.max_file);
    if let Ok(buf) = &res {
        st.bytes = st.bytes.saturating_add(buf.len() as u64);
        if st.bytes > MAX_WALK_BYTES {
            return Err(over());
        }
    }
    (st.visit)(rel, res);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sort_names;

    #[test]
    fn sort_is_lowercase_then_exact() {
        let mut v = vec!["b".to_string(), "a".into(), "A".into(), "B".into()];
        sort_names(&mut v, |n| n);
        assert_eq!(v, ["A", "a", "B", "b"]);
    }
}
