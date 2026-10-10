//! THE error-code table. Codes are protocol: renaming one is a breaking change.

use std::collections::BTreeMap;

use mda_vault::{Root, VaultError};

pub use mda_core::error::{
    NAMING_CONTROL_CHAR, NAMING_EDGE_DOT, NAMING_EDGE_SPACE, NAMING_EMPTY, NAMING_ILLEGAL_CHAR,
    NAMING_PATH_INJECTION, NAMING_RESERVED, NAMING_TOO_LONG, RENDER_EACH_NOT_LIST,
    RENDER_JOIN_EMPTY, RENDER_JOIN_RECORD, RENDER_NOT_SCALAR, RENDER_SELF_NESTED,
    RENDER_UNKNOWN_VAR, RENDER_UNMATCHED_END, RENDER_UNTERMINATED, RENDER_WARNING_CODES,
    VKS_BAD_KEY, VKS_CONTROL_CHAR, VKS_DUPLICATE_KEY, VKS_INDENT, VKS_INLINE_COMMENT, VKS_LIMIT,
    VKS_MIXED_DIALECT, VKS_MIXED_LIST, VKS_SYNTAX, VKS_UNREPRESENTABLE, VKS_UNSUPPORTED,
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

/// The file's content hash differs from the one the client sent. Params `path`, `expected`, `actual`.
pub const FILE_CONFLICT: &str = "file.conflict";
/// A create found a file already at the path. Param `path`.
pub const FILE_EXISTS: &str = "file.exists";
/// The file carries a `varsError`; create/update will not write over it (D-6, W-6). Param `path`.
pub const ARTIFACT_VARS_INVALID: &str = "artifact.vars_invalid";
/// The model cannot be written in the file format. Params `field`, `reason` (`round_trip` | `multi_block`).
pub const ARTIFACT_UNREPRESENTABLE: &str = "artifact.unrepresentable";
/// No block with the given index and heading. Param `block` (index, or `""`).
pub const ARTIFACT_BLOCK_NOT_FOUND: &str = "artifact.block_not_found";
/// The engine will not rewrite this file. Param `reason` (`flagged` | `index` | `variables` |
/// `no_frontmatter` | `no_fence`).
pub const ARTIFACT_NOT_SERIALIZABLE: &str = "artifact.not_serializable";

/// A render expansion limit was reached; the whole render is refused. Params `limit`
/// (`output_bytes` | `iterations` | `steps` | `depth`), `max`.
pub const RENDER_LIMIT: &str = "render.limit";
/// The human CLI refuses to print rendered output holding an ESC character (`--json` returns it).
pub const RENDER_CONTAINS_ESCAPE: &str = "render.contains_escape";
/// `artifact.write_file` on a type that does not write a file. Param `type`.
pub const ARTIFACT_NOT_WHOLE_FILE: &str = "artifact.not_whole_file";
/// `artifact.write_file` on a whole-file artifact with more than one block. Param `count`.
pub const ARTIFACT_MULTI_BLOCK: &str = "artifact.multi_block";
/// `artifact.write_file` on a template index (an index is run, never written). No params.
pub const ARTIFACT_IS_INDEX: &str = "artifact.is_index";

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
    VKS_UNREPRESENTABLE,
    FILE_CONFLICT,
    FILE_EXISTS,
    ARTIFACT_VARS_INVALID,
    ARTIFACT_UNREPRESENTABLE,
    ARTIFACT_BLOCK_NOT_FOUND,
    ARTIFACT_NOT_SERIALIZABLE,
    RENDER_LIMIT,
    RENDER_CONTAINS_ESCAPE,
    NAMING_PATH_INJECTION,
    NAMING_EMPTY,
    NAMING_EDGE_SPACE,
    NAMING_EDGE_DOT,
    NAMING_ILLEGAL_CHAR,
    NAMING_CONTROL_CHAR,
    NAMING_RESERVED,
    NAMING_TOO_LONG,
    ARTIFACT_NOT_WHOLE_FILE,
    ARTIFACT_MULTI_BLOCK,
    ARTIFACT_IS_INDEX,
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
    | VaultError::Conflict { path, .. }
    | VaultError::Exists { path }
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
            VaultError::Conflict {
                path,
                expected,
                actual,
            } => Self::new(FILE_CONFLICT)
                .with("path", path.display().to_string())
                .with("expected", expected)
                .with("actual", actual),
            VaultError::Exists { path } => {
                Self::new(FILE_EXISTS).with("path", path.display().to_string())
            }
            VaultError::Io { path, kind } => Self::new(IO_FAILED)
                .with("path", path.display().to_string())
                .with("kind", kind.to_string()),
        }
    }
}

// A rejected client value (render `values`): the codec's `{code, params}` moved over unchanged.
impl From<mda_core::error::VarsError> for OpError {
    fn from(e: mda_core::error::VarsError) -> Self {
        Self {
            code: e.code,
            params: e.params,
        }
    }
}

// A rejected output file name: `field` on injection, `max` on length, nothing else.
impl From<mda_core::naming::NameError> for OpError {
    fn from(e: mda_core::naming::NameError) -> Self {
        use mda_core::naming::{MAX_NAME_BYTES, NameError};
        let op = Self::new(e.code());
        match e {
            NameError::PathInjection(f) => op.with("field", f.as_str()),
            NameError::TooLong => op.with("max", MAX_NAME_BYTES.to_string()),
            _ => op,
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
    fn conflict_maps_relative_with_hashes() {
        let root = Root::new(&std::env::temp_dir()).unwrap();
        let e = vault_error(
            VaultError::Conflict {
                path: root.path().join("Templates/a.md"),
                expected: "a".into(),
                actual: "b".into(),
            },
            &root,
        );
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"code": "file.conflict",
                "params": {"path": "Templates/a.md", "expected": "a", "actual": "b"}})
        );
    }

    #[test]
    fn exists_maps_relative() {
        let root = Root::new(&std::env::temp_dir()).unwrap();
        let e = vault_error(
            VaultError::Exists {
                path: root.path().join("Templates/a.md"),
            },
            &root,
        );
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"code": "file.exists", "params": {"path": "Templates/a.md"}})
        );
    }

    #[test]
    fn write_codes_exact_json() {
        let cases = [
            (ARTIFACT_VARS_INVALID, "path", "artifact.vars_invalid"),
            (
                ARTIFACT_BLOCK_NOT_FOUND,
                "block",
                "artifact.block_not_found",
            ),
            (
                ARTIFACT_NOT_SERIALIZABLE,
                "reason",
                "artifact.not_serializable",
            ),
            (
                ARTIFACT_UNREPRESENTABLE,
                "field",
                "artifact.unrepresentable",
            ),
            (VKS_UNREPRESENTABLE, "var", "vks.unrepresentable"),
        ];
        for (code, key, want) in cases {
            let e = OpError::new(code).with(key, "x");
            assert_eq!(
                serde_json::to_value(&e).unwrap(),
                serde_json::json!({"code": want, "params": {key: "x"}})
            );
        }
    }

    #[test]
    fn naming_codes_listed() {
        for c in mda_core::error::NAMING_CODES {
            assert!(ALL_CODES.contains(c), "{c}");
        }
    }

    #[test]
    fn warning_codes_are_not_error_codes() {
        for c in RENDER_WARNING_CODES {
            assert!(!ALL_CODES.contains(c), "{c}");
        }
    }

    #[test]
    fn w3_codes_exact_json() {
        let bare = [
            (RENDER_CONTAINS_ESCAPE, "render.contains_escape"),
            (NAMING_EMPTY, "naming.empty"),
            (NAMING_EDGE_SPACE, "naming.edge_space"),
            (NAMING_EDGE_DOT, "naming.edge_dot"),
            (NAMING_ILLEGAL_CHAR, "naming.illegal_char"),
            (NAMING_CONTROL_CHAR, "naming.control_char"),
            (NAMING_RESERVED, "naming.reserved"),
            (ARTIFACT_IS_INDEX, "artifact.is_index"),
        ];
        for (code, want) in bare {
            assert_eq!(
                serde_json::to_value(OpError::new(code)).unwrap(),
                serde_json::json!({"code": want, "params": {}})
            );
        }
        let with = [
            (NAMING_PATH_INJECTION, "field", "naming.path_injection"),
            (NAMING_TOO_LONG, "max", "naming.too_long"),
            (ARTIFACT_NOT_WHOLE_FILE, "type", "artifact.not_whole_file"),
            (ARTIFACT_MULTI_BLOCK, "count", "artifact.multi_block"),
        ];
        for (code, key, want) in with {
            assert_eq!(
                serde_json::to_value(OpError::new(code).with(key, "x")).unwrap(),
                serde_json::json!({"code": want, "params": {key: "x"}})
            );
        }
        let limit = OpError::new(RENDER_LIMIT)
            .with("limit", "steps")
            .with("max", "1000000");
        assert_eq!(
            serde_json::to_value(limit).unwrap(),
            serde_json::json!({"code": "render.limit", "params": {"limit": "steps", "max": "1000000"}})
        );
    }

    #[test]
    fn name_error_maps_field() {
        use mda_core::naming::{NameError, NameField};
        let e = OpError::from(NameError::PathInjection(NameField::Title));
        assert_eq!(
            serde_json::to_value(e).unwrap(),
            serde_json::json!({"code": "naming.path_injection", "params": {"field": "title"}})
        );
        let e = OpError::from(NameError::TooLong);
        assert_eq!(e.params["max"], "255");
        assert!(OpError::from(NameError::Reserved).params.is_empty());
    }

    #[test]
    fn vars_error_maps_unchanged() {
        let v = mda_core::error::VarsError::new(VKS_CONTROL_CHAR, 0).with("var", "VK-a");
        let e = OpError::from(v);
        assert_eq!(e.code, "vks.control_char");
        assert_eq!(e.params["var"], "VK-a");
    }

    #[test]
    fn vks_codes_listed() {
        for c in mda_core::error::VKS_CODES {
            assert!(ALL_CODES.contains(c), "{c}");
        }
    }
}
