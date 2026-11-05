//! The `safe_rel_path` rejection codes (spec §8.3).

/// A vault-authored relative path that is empty (or only `.` / empty segments). Param `path`.
pub const PATH_EMPTY: &str = "path.empty";
/// A vault-authored path holding a C0, DEL or C1 control character. Param `path`.
pub const PATH_CONTROL_CHAR: &str = "path.control_char";
/// A vault-authored path holding `:` (drive letter or URI scheme). Param `path`.
pub const PATH_DRIVE_OR_SCHEME: &str = "path.drive_or_scheme";
/// A vault-authored path holding `\`. Param `path`.
pub const PATH_BACKSLASH: &str = "path.backslash";
/// A vault-authored path starting with `/`. Param `path`.
pub const PATH_ABSOLUTE: &str = "path.absolute";
/// A vault-authored path with a `..` segment. Param `path`.
pub const PATH_PARENT_SEGMENT: &str = "path.parent_segment";

/// Every `safe_rel_path` rejection code (spec §8.3), for the `ALL_CODES` contract pin in `mda-ops`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::PATH_CODES.len(), 6);
/// ```
pub const PATH_CODES: &[&str] = &[
    PATH_EMPTY,
    PATH_CONTROL_CHAR,
    PATH_DRIVE_OR_SCHEME,
    PATH_BACKSLASH,
    PATH_ABSOLUTE,
    PATH_PARENT_SEGMENT,
];
