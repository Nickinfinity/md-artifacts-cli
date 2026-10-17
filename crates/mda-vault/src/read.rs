//! Bounded reads: type and size are checked on metadata before the file is opened.

use std::io::Read;
use std::path::Path;

use crate::contain::io_err;
use crate::{Root, VaultError, contain};

/// Size of a contained regular file from metadata, without opening it: the apply budget is checked
/// with it before any read (spec §11). Contained first, so an outside target's size never leaks.
///
/// # Examples
///
/// ```no_run
/// use mda_vault::{Root, file_len};
/// let root = Root::new(std::path::Path::new("/vault")).unwrap();
/// let _ = file_len(&root, std::path::Path::new("Snippets/a.md"));
/// ```
pub fn file_len(root: &Root, rel: &Path) -> Result<u64, VaultError> {
    let path = contain(root, rel)?;
    let meta = std::fs::metadata(&path).map_err(|e| io_err(&path, &e))?;
    if !meta.is_file() {
        return Err(VaultError::NotRegular { path });
    }
    Ok(meta.len())
}

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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mda-filelen-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("vault/Snippets")).unwrap();
        std::fs::create_dir_all(d.join("outside")).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn regular_len_and_missing() {
        let d = tmp("reg");
        std::fs::write(d.join("vault/Snippets/a.md"), b"hello").unwrap();
        let root = Root::new(&d.join("vault")).unwrap();
        assert_eq!(file_len(&root, Path::new("Snippets/a.md")).unwrap(), 5);
        assert!(matches!(
            file_len(&root, Path::new("Snippets/none.md")),
            Err(VaultError::NotFound { .. })
        ));
    }

    #[test]
    fn dir_is_not_regular() {
        let d = tmp("dir");
        let root = Root::new(&d.join("vault")).unwrap();
        assert!(matches!(
            file_len(&root, Path::new("Snippets")),
            Err(VaultError::NotRegular { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn fifo_is_not_regular_and_never_opened() {
        let d = tmp("fifo");
        let p = d.join("vault/Snippets/f.md");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&p)
                .status()
                .unwrap()
                .success()
        );
        let root = Root::new(&d.join("vault")).unwrap();
        let h = std::thread::spawn(move || file_len(&root, Path::new("Snippets/f.md")));
        let t = std::time::Instant::now();
        while !h.is_finished() {
            assert!(t.elapsed().as_secs() < 5, "file_len blocked on a FIFO");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(
            h.join().unwrap(),
            Err(VaultError::NotRegular { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn link_out_never_reports_outside_size() {
        let d = tmp("link");
        std::fs::write(d.join("outside/big.md"), vec![b'x'; 2 << 20]).unwrap();
        std::os::unix::fs::symlink(d.join("outside/big.md"), d.join("vault/Snippets/l.md"))
            .unwrap();
        let root = Root::new(&d.join("vault")).unwrap();
        assert!(matches!(
            file_len(&root, Path::new("Snippets/l.md")),
            Err(VaultError::OutsideRoot { .. })
        ));
    }
}
