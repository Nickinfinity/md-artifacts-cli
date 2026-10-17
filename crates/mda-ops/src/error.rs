//! THE error-code table. Codes are protocol: renaming one is a breaking change.

use std::collections::BTreeMap;

use mda_vault::{Root, VaultError};

pub use mda_core::error::{
    ARTIFACT_UNREPRESENTABLE, MIGRATE_LEADING_BLANK, MIGRATE_QUOTED_TYPE, MIGRATE_UNFENCED_VARS,
    NAMING_CONTROL_CHAR, NAMING_EDGE_DOT, NAMING_EDGE_SPACE, NAMING_EMPTY, NAMING_ILLEGAL_CHAR,
    NAMING_PATH_INJECTION, NAMING_RESERVED, NAMING_TOO_LONG, PATH_ABSOLUTE, PATH_BACKSLASH,
    PATH_CODES, PATH_CONTROL_CHAR, PATH_DRIVE_OR_SCHEME, PATH_EMPTY, PATH_PARENT_SEGMENT,
    RENDER_EACH_NOT_LIST, RENDER_JOIN_EMPTY, RENDER_JOIN_RECORD, RENDER_NOT_SCALAR,
    RENDER_SELF_NESTED, RENDER_UNKNOWN_VAR, RENDER_UNMATCHED_END, RENDER_UNTERMINATED,
    RENDER_WARNING_CODES, VKS_BAD_KEY, VKS_CONTROL_CHAR, VKS_DUPLICATE_KEY, VKS_INDENT,
    VKS_INLINE_COMMENT, VKS_LIMIT, VKS_MIXED_DIALECT, VKS_MIXED_LIST, VKS_SYNTAX,
    VKS_UNREPRESENTABLE, VKS_UNSUPPORTED,
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
/// The op needs a vault and none is selected (no `--vault`, no default). No params.
pub const VAULT_NOT_SELECTED: &str = "vault.not_selected";
/// Not an artifact path: not `.md`, or not under a type directory. Param `path`.
pub const ARTIFACT_BAD_PATH: &str = "artifact.bad_path";

/// The file's content hash differs from the one the client sent. Params `path`, `expected`, `actual`.
pub const FILE_CONFLICT: &str = "file.conflict";
/// A create found a file already at the path. Param `path`.
pub const FILE_EXISTS: &str = "file.exists";
/// The file carries a `varsError`; create/update will not write over it (D-6, W-6). Param `path`.
pub const ARTIFACT_VARS_INVALID: &str = "artifact.vars_invalid";
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

/// `index.plan`/`index.run` on a file that is not a template index. Param `path`.
pub const INDEX_NOT_INDEX: &str = "index.not_index";
/// `index.create`: a path segment with no letters or digits to slug. Param `segment`.
pub const INDEX_UNSLUGGABLE: &str = "index.unsluggable";
/// An index size limit (spec §8.2). Params `limit` (`links` | `steps`), `max`.
pub const INDEX_LIMIT: &str = "index.limit";

/// Not an Obsidian vault: no `.obsidian/` directory (spec §12.1). Param `path`.
pub const VAULT_NOT_OBSIDIAN: &str = "vault.not_obsidian";
/// The path exists but is not a directory (vault, workspace root, type dir, `.mda`). Param `path`.
pub const PATH_NOT_DIR: &str = "path.not_dir";
/// `vault.use` on a vault not in the machine file's known list. Param `path`.
pub const VAULT_UNKNOWN: &str = "vault.unknown";
/// The stored default vault no longer validates; the op needed a vault. Param `path` (the default).
pub const VAULT_UNAVAILABLE: &str = "vault.unavailable";
/// A configuration file is corrupt or breaks a spec §12 rule, or no config dir exists. Param `path`.
pub const CONFIG_INVALID: &str = "config.invalid";

/// `vars.var.add` / `vars.var.rename`: not `VK-` + a letter, then letters, digits or `_`. Param `name`.
pub const VARS_BAD_NAME: &str = "vars.bad_name";
/// A variable with that name already exists in the sub-set. Param `name`.
pub const VARS_VAR_EXISTS: &str = "vars.var_exists";
/// No variable of that exact name in the sub-set. Param `name`.
pub const VARS_VAR_NOT_FOUND: &str = "vars.var_not_found";
/// Another sub-set already has that (trimmed) heading. Param `heading`.
pub const VARS_SUBSET_EXISTS: &str = "vars.subset_exists";
/// The sub-set heading is invalid. Param `reason` (`empty`).
pub const VARS_BAD_HEADING: &str = "vars.bad_heading";
/// A description on the untitled block. No params.
pub const VARS_UNTITLED_DESCRIPTION: &str = "vars.untitled_description";
/// `vars.file.new`: the title is empty after trimming. No params.
pub const VARS_BAD_TITLE: &str = "vars.bad_title";
/// A folder walk hit a cap (spec §11, §13.2; renamed from `vars.limit` in W6, M-2). Params `limit`
/// (`depth` | `files` | `bytes`), `max`.
pub const WALK_LIMIT: &str = "walk.limit";
/// Migration apply: the rewrite on the fresh bytes finds nothing to change (spec §11). No params.
pub const MIGRATE_UP_TO_DATE: &str = "migrate.up_to_date";
/// Validate warning: a migration is pending for this file. Param `kind` (`frontmatter` | `vks`).
pub const MIGRATE_PENDING: &str = "migrate.pending";
/// Validate warning: a `## ` section with no `vks` fence in a Variables file (dropped by every rewrite,
/// #54). Param `heading`.
pub const VARS_FENCELESS_SECTION: &str = "vars.fenceless_section";
/// Validate warning: a Variables fence sits before the first `## ` section (spec §13.1). No params.
pub const VARS_MIXED_FILE: &str = "vars.mixed_file";

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
    PATH_EMPTY,
    PATH_CONTROL_CHAR,
    PATH_DRIVE_OR_SCHEME,
    PATH_BACKSLASH,
    PATH_ABSOLUTE,
    PATH_PARENT_SEGMENT,
    INDEX_NOT_INDEX,
    INDEX_UNSLUGGABLE,
    INDEX_LIMIT,
    VAULT_NOT_OBSIDIAN,
    PATH_NOT_DIR,
    VAULT_UNKNOWN,
    VAULT_UNAVAILABLE,
    CONFIG_INVALID,
    VARS_BAD_NAME,
    VARS_VAR_EXISTS,
    VARS_VAR_NOT_FOUND,
    VARS_SUBSET_EXISTS,
    VARS_BAD_HEADING,
    VARS_UNTITLED_DESCRIPTION,
    VARS_BAD_TITLE,
    WALK_LIMIT,
    MIGRATE_UP_TO_DATE,
    MIGRATE_LEADING_BLANK,
    MIGRATE_QUOTED_TYPE,
    MIGRATE_UNFENCED_VARS,
    MIGRATE_PENDING,
    VARS_FENCELESS_SECTION,
    VARS_MIXED_FILE,
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
    // `Limit` carries no path (the whole walk is refused); every other variant is relativized.
    if let VaultError::OutsideRoot { path }
    | VaultError::NotFound { path }
    | VaultError::TooLarge { path, .. }
    | VaultError::NotRegular { path }
    | VaultError::Conflict { path, .. }
    | VaultError::Exists { path }
    | VaultError::NotDir { path }
    | VaultError::NotObsidian { path }
    | VaultError::ConfigInvalid { path }
    | VaultError::Io { path, .. } = &mut e
        && let Ok(rel) = path.strip_prefix(root.path())
    {
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
            VaultError::NotDir { path } => {
                Self::new(PATH_NOT_DIR).with("path", path.display().to_string())
            }
            VaultError::NotObsidian { path } => {
                Self::new(VAULT_NOT_OBSIDIAN).with("path", path.display().to_string())
            }
            VaultError::ConfigInvalid { path } => {
                Self::new(CONFIG_INVALID).with("path", path.display().to_string())
            }
            VaultError::Limit { limit, max } => Self::new(WALK_LIMIT)
                .with("limit", limit.as_str())
                .with("max", max.to_string()),
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

// A refused Variables-file edit (`mda_core::variables`): one code per variant.
impl From<mda_core::variables::EditError> for OpError {
    fn from(e: mda_core::variables::EditError) -> Self {
        use mda_core::variables::EditError;
        match e {
            EditError::BadName(n) => Self::new(VARS_BAD_NAME).with("name", n),
            EditError::VarExists(n) => Self::new(VARS_VAR_EXISTS).with("name", n),
            EditError::VarNotFound(n) => Self::new(VARS_VAR_NOT_FOUND).with("name", n),
            EditError::SubsetExists(h) => Self::new(VARS_SUBSET_EXISTS).with("heading", h),
            EditError::EmptyHeading => Self::new(VARS_BAD_HEADING).with("reason", "empty"),
            EditError::UntitledDescription => Self::new(VARS_UNTITLED_DESCRIPTION),
            EditError::BlockNotFound(i) => Self::new(ARTIFACT_BLOCK_NOT_FOUND)
                .with("block", i.map(|i| i.to_string()).unwrap_or_default()),
            EditError::Mixed => Self::new(ARTIFACT_NOT_SERIALIZABLE)
                .with("reason", mda_core::error::Refusal::Variables.as_str()),
        }
    }
}

#[cfg(test)]
mod tests;
