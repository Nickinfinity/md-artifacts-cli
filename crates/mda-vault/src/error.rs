//! The vault crate's one error type. The `#[error]` texts are for logs only: clients see the codes
//! `mda-ops` maps these variants to.

use std::path::PathBuf;

/// Why a vault operation was refused or failed.
///
/// # Examples
///
/// ```
/// use mda_vault::VaultError;
/// let e = VaultError::NotFound { path: "missing".into() };
/// assert!(matches!(e, VaultError::NotFound { .. }));
/// ```
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// The path resolves outside the root, or cannot be proven inside it.
    #[error("path outside root: {path:?}")]
    OutsideRoot { path: PathBuf },
    /// The path does not exist.
    #[error("path not found: {path:?}")]
    NotFound { path: PathBuf },
    /// The file is larger than the caller's limit; refused before reading.
    #[error("file too large: {path:?} ({size} > {max})")]
    TooLarge { path: PathBuf, size: u64, max: u64 },
    /// Not a regular file (FIFO, device, directory); refused before opening.
    #[error("not a regular file: {path:?}")]
    NotRegular { path: PathBuf },
    /// The file's content hash is not the one the client last read (optimistic concurrency).
    #[error("conflict on {path:?}: expected {expected}, actual {actual}")]
    Conflict {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    /// A create found a file already at the target; it is never overwritten.
    #[error("file exists: {path:?}")]
    Exists { path: PathBuf },
    /// Any other I/O failure.
    #[error("io failed on {path:?}: {kind}")]
    Io {
        path: PathBuf,
        kind: std::io::ErrorKind,
    },
}
