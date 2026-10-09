//! THE error-code table. Codes are protocol: renaming one is a breaking change.

use std::collections::BTreeMap;

use mda_vault::VaultError;

/// The only error shape clients see: a stable code plus string parameters, never prose.
///
/// # Examples
///
/// ```
/// use mda_ops::{OpError, error::PATH_NOT_FOUND};
/// let e = OpError::new(PATH_NOT_FOUND).with("path", "a");
/// assert_eq!(serde_json::to_value(&e).unwrap()["code"], "path.not_found");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OpError {
    pub code: &'static str,
    pub params: BTreeMap<String, String>,
}

impl OpError {
    /// An error with no parameters.
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            params: BTreeMap::new(),
        }
    }

    /// Add one parameter.
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

/// No op is registered under the requested name.
pub const OP_UNKNOWN: &str = "op.unknown";
/// The request's params did not deserialize (unknown field, wrong type, missing field).
pub const OP_BAD_REQUEST: &str = "op.bad_request";
/// The client's protocol major version differs from the engine's.
pub const PROTOCOL_VERSION_MISMATCH: &str = "protocol.version_mismatch";
/// A path resolves outside its root, or cannot be proven inside it.
pub const PATH_OUTSIDE_ROOT: &str = "path.outside_root";
/// A path does not exist.
pub const PATH_NOT_FOUND: &str = "path.not_found";
/// A file exceeds the read limit.
pub const FILE_TOO_LARGE: &str = "file.too_large";
/// Not a regular file (FIFO, device, directory).
pub const FILE_NOT_REGULAR: &str = "file.not_regular";
/// Any other I/O failure (exit code 3).
pub const IO_FAILED: &str = "io.failed";
/// An engine defect: an op produced a response that could not be serialized. No params.
pub const OP_INTERNAL: &str = "op.internal";

/// Every code, for exhaustive checks (English table, exit codes, op cases).
pub const ALL_CODES: &[&str] = &[
    OP_UNKNOWN,
    OP_BAD_REQUEST,
    PROTOCOL_VERSION_MISMATCH,
    PATH_OUTSIDE_ROOT,
    PATH_NOT_FOUND,
    FILE_TOO_LARGE,
    FILE_NOT_REGULAR,
    IO_FAILED,
    OP_INTERNAL,
];

// ponytail: paths become params via display(), lossy on non-UTF-8 names; carry raw bytes if a
// client ever needs to round-trip such a name.
impl From<VaultError> for OpError {
    fn from(e: VaultError) -> Self {
        match e {
            VaultError::OutsideRoot { path } => {
                Self::new(PATH_OUTSIDE_ROOT).with("path", path.display().to_string())
            }
            VaultError::NotFound { path } => {
                Self::new(PATH_NOT_FOUND).with("path", path.display().to_string())
            }
            VaultError::TooLarge { path, size, max } => Self::new(FILE_TOO_LARGE)
                .with("path", path.display().to_string())
                .with("size", size.to_string())
                .with("max", max.to_string()),
            VaultError::NotRegular { path } => {
                Self::new(FILE_NOT_REGULAR).with("path", path.display().to_string())
            }
            VaultError::Io { path, kind } => Self::new(IO_FAILED)
                .with("path", path.display().to_string())
                .with("kind", kind.to_string()),
        }
    }
}
