//! Bounded reads: type and size are checked on metadata before the file is opened.

use std::io::Read;
use std::path::Path;

use crate::contain::io_err;
use crate::{Root, VaultError, contain};

/// Read a contained file, refusing anything larger than `max_bytes` before reading it.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, read_bounded};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// let bytes = read_bounded(&root, std::path::Path::new("Snippets/a.md"), 1 << 20).unwrap();
/// # let _ = bytes;
/// ```
pub fn read_bounded(root: &Root, rel: &Path, max_bytes: u64) -> Result<Vec<u8>, VaultError> {
    let path = contain(root, rel)?;
    log::debug!("read {}", path.display());
    // ponytail: TOCTOU contain→open; upgrade: open O_NOFOLLOW|O_NONBLOCK + fstat the handle
    let meta = std::fs::metadata(&path).map_err(|e| io_err(&path, &e))?;
    // Before open: opening a FIFO blocks forever.
    if !meta.is_file() {
        return Err(VaultError::NotRegular { path });
    }
    if meta.len() > max_bytes {
        return Err(VaultError::TooLarge {
            path,
            size: meta.len(),
            max: max_bytes,
        });
    }
    let file = std::fs::File::open(&path).map_err(|e| io_err(&path, &e))?;
    let mut buf = Vec::new();
    // Metadata can lie (growing file): cap the read itself at one byte over.
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| io_err(&path, &e))?;
    let size = buf.len() as u64;
    if size > max_bytes {
        return Err(VaultError::TooLarge {
            path,
            size,
            max: max_bytes,
        });
    }
    Ok(buf)
}
