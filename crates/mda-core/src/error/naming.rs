//! The `naming.*` codes: vault file names and output names (spec §5.1, §14.1).

/// An output file name containing `/`, `\`, NUL or `..` (spec §5.1). Param `field` ([`crate::naming::NameField`]).
pub const NAMING_PATH_INJECTION: &str = "naming.path_injection";
/// An output file name that is empty after trimming.
pub const NAMING_EMPTY: &str = "naming.empty";
/// An output file name starting or ending with a space.
pub const NAMING_EDGE_SPACE: &str = "naming.edge_space";
/// An output file name ending with a dot (a leading dot is allowed).
pub const NAMING_EDGE_DOT: &str = "naming.edge_dot";
/// An output file name containing one of `\ / : * ? " < > |`.
pub const NAMING_ILLEGAL_CHAR: &str = "naming.illegal_char";
/// An output file name containing a C0 control character or DEL.
pub const NAMING_CONTROL_CHAR: &str = "naming.control_char";
/// A Windows reserved name, whole or as the stem (`CON.txt`).
pub const NAMING_RESERVED: &str = "naming.reserved";
/// An output file name longer than 255 bytes. Param `max`.
pub const NAMING_TOO_LONG: &str = "naming.too_long";
/// A vault file-name stem that already ends in `.md` (`x.md.md`; spec §14). No params.
pub const NAMING_MD_EXTENSION: &str = "naming.md_extension";
/// A vault file-name stem or new directory segment that is not in Unicode NFC form (spec §14.1). Param
/// `segment` for a directory segment.
pub const NAMING_NOT_NFC: &str = "naming.not_nfc";

/// Every `naming.*` code, for the `ALL_CODES` contract pin in `mda-ops`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::NAMING_CODES.len(), 10);
/// ```
pub const NAMING_CODES: &[&str] = &[
    NAMING_PATH_INJECTION,
    NAMING_EMPTY,
    NAMING_EDGE_SPACE,
    NAMING_EDGE_DOT,
    NAMING_ILLEGAL_CHAR,
    NAMING_CONTROL_CHAR,
    NAMING_RESERVED,
    NAMING_TOO_LONG,
    NAMING_MD_EXTENSION,
    NAMING_NOT_NFC,
];
