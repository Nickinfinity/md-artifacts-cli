//! One level of a vault directory: sub-directory names and `.md` file names, sorted (Q-F).

use std::fs;
use std::path::Path;

use crate::contain::io_err;
use crate::{Root, VaultError, contain};

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
    sort_names(&mut out.dirs);
    sort_names(&mut out.files);
    Ok(out)
}

/// Case-insensitive order with the exact name as tiebreak (Q-F).
fn sort_names(v: &mut [String]) {
    v.sort_by_cached_key(|n| (n.to_lowercase(), n.clone()));
}

#[cfg(test)]
mod tests {
    use super::sort_names;

    #[test]
    fn sort_is_lowercase_then_exact() {
        let mut v = vec!["b".to_string(), "a".into(), "A".into(), "B".into()];
        sort_names(&mut v);
        assert_eq!(v, ["A", "a", "B", "b"]);
    }
}
