//! THE error-code table. Codes are protocol: renaming one is a breaking change.

use std::collections::BTreeMap;

use mda_vault::{Root, VaultError};

pub use mda_core::error::{
    VKS_BAD_KEY, VKS_CONTROL_CHAR, VKS_DUPLICATE_KEY, VKS_INDENT, VKS_INLINE_COMMENT, VKS_LIMIT,
    VKS_MIXED_DIALECT, VKS_MIXED_LIST, VKS_SYNTAX, VKS_UNSUPPORTED,
};

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
/// The op needs a vault and none was given (`--vault`). No params.
pub const VAULT_NOT_SELECTED: &str = "vault.not_selected";
/// Not an artifact path: not `.md`, or not under a type directory. Param `path`.
pub const ARTIFACT_BAD_PATH: &str = "artifact.bad_path";

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
    VAULT_NOT_SELECTED,
    ARTIFACT_BAD_PATH,
    VKS_SYNTAX,
    VKS_INDENT,
    VKS_BAD_KEY,
    VKS_DUPLICATE_KEY,
    VKS_UNSUPPORTED,
    VKS_MIXED_LIST,
    VKS_INLINE_COMMENT,
    VKS_CONTROL_CHAR,
    VKS_LIMIT,
    VKS_MIXED_DIALECT,
];

/// Map a vault error from an op that holds `root`: every `path` param becomes vault-relative POSIX,
/// so no machine path reaches a client. A path not under the root (an `OutsideRoot` candidate is
/// client text) is echoed as given. Every op holding a `Root` maps through this, never `From`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Root, error::vault_error};
/// use mda_vault::VaultError;
/// let root = Root::new(&std::env::temp_dir()).unwrap();
/// let e = vault_error(VaultError::NotFound { path: root.path().join("Snippets/x.md") }, &root);
/// assert_eq!(e.params["path"], "Snippets/x.md");
/// ```
pub fn vault_error(mut e: VaultError, root: &Root) -> OpError {
    let (VaultError::OutsideRoot { path }
    | VaultError::NotFound { path }
    | VaultError::TooLarge { path, .. }
    | VaultError::NotRegular { path }
    | VaultError::Io { path, .. }) = &mut e;
    if let Ok(rel) = path.strip_prefix(root.path()) {
        let posix: Vec<_> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        *path = posix.join("/").into();
    }
    OpError::from(e)
}

// For root-less errors only (`Root::new` on the user's own `--vault`): the path is the user's.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_error_relativizes_paths() {
        let root = Root::new(&std::env::temp_dir()).unwrap();
        let abs = root.path().join("Snippets/x.md");
        let e = vault_error(VaultError::NotFound { path: abs }, &root);
        assert_eq!(e.params["path"], "Snippets/x.md");
    }

    #[test]
    fn vault_error_echoes_outside_candidate() {
        let root = Root::new(&std::env::temp_dir()).unwrap();
        let e = vault_error(
            VaultError::OutsideRoot {
                path: "../etc/passwd".into(),
            },
            &root,
        );
        assert_eq!(e.code, PATH_OUTSIDE_ROOT);
        assert_eq!(e.params["path"], "../etc/passwd");
    }

    #[test]
    fn vks_codes_listed() {
        for c in mda_core::error::VKS_CODES {
            assert!(ALL_CODES.contains(c), "{c}");
        }
    }
}
