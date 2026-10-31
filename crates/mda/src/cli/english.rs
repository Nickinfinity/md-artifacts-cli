//! The one English error table (split from `print.rs`; reached as `mda::cli::print::english`).

/// English text for an error code; `None` for a code this table does not know.
///
/// # Examples
///
/// ```
/// assert!(mda::cli::print::english("op.unknown").is_some());
/// assert!(mda::cli::print::english("no.such").is_none());
/// ```
pub fn english(code: &str) -> Option<&'static str> {
    use mda_ops::error::*;
    Some(match code {
        OP_UNKNOWN => "no such operation",
        OP_BAD_REQUEST => "the request parameters are invalid",
        PROTOCOL_VERSION_MISMATCH => "client and engine protocol versions are incompatible",
        PROTOCOL_PARSE_ERROR => "a protocol line is not valid JSON",
        PROTOCOL_LINE_TOO_LONG => "a protocol line exceeds 16 MiB",
        PROTOCOL_ALREADY_INITIALIZED => "initialize was already sent",
        PROTOCOL_NOT_INITIALIZED => "send initialize first",
        PROTOCOL_INVALID_REQUEST => "not a valid JSON-RPC request",
        PATH_OUTSIDE_ROOT => "path is outside the allowed root",
        PATH_NOT_FOUND => "path does not exist",
        FILE_TOO_LARGE => "file exceeds the size limit",
        FILE_NOT_REGULAR => "not a regular file",
        IO_FAILED => "I/O failure",
        OP_INTERNAL => "internal engine error",
        VAULT_NOT_SELECTED => "no vault selected (run \"mda vault select <path>\" or pass --vault)",
        VAULT_NOT_OBSIDIAN => "not an Obsidian vault (no .obsidian folder)",
        PATH_NOT_DIR => "path is not a directory",
        VAULT_UNKNOWN => "not a known vault (add it with \"mda vault select\")",
        VAULT_UNAVAILABLE => "the default vault is no longer available",
        CONFIG_INVALID => "the configuration file is invalid; fix or remove it",
        VARS_BAD_NAME => "variable names are VK- then a letter, then letters, digits or _",
        VARS_VAR_EXISTS => "a variable with that name already exists in this sub-set",
        VARS_VAR_NOT_FOUND => "no variable with that name in this sub-set",
        VARS_SUBSET_EXISTS => "a sub-set with that heading already exists",
        VARS_BAD_HEADING => "the sub-set heading is invalid",
        VARS_UNTITLED_DESCRIPTION => {
            "the untitled block has no heading to keep a description under; add a sub-set first"
        }
        VARS_BAD_TITLE => "the title is empty",
        WALK_LIMIT => "a folder walk exceeds a limit (depth, entries or bytes)",
        MIGRATE_UP_TO_DATE => "nothing left to migrate in this file",
        MIGRATE_LEADING_BLANK => "blank line(s) precede the frontmatter; remove them to migrate",
        MIGRATE_QUOTED_TYPE => "the type: value is quoted; expected a bare legacy value",
        MIGRATE_PENDING => "a migration is pending for this file",
        VARS_FENCELESS_SECTION => "a ## section with no defaults fence is dropped by every rewrite",
        VARS_UNTAGGED_DEFAULTS => {
            "a fence holds VK- keys but is not tagged ```yaml VKs, so its values are not read"
        }
        RENAME_INDEX_LINK => "an index links the old name",
        RENAME_LINKS_UNCHECKED => "index links not checked",
        RENAME_LINK_UPDATE_FAILED => "an index link could not be updated",
        VARS_MIXED_FILE => {
            "a variables fence sits before the first ## section; edits refuse this file"
        }
        ARTIFACT_BAD_PATH => "not an artifact path (expected <TypeDir>/….md)",
        VKS_SYNTAX => "variables block: syntax error",
        VKS_INDENT => "variables block: bad indentation",
        VKS_BAD_KEY => "variables block: invalid key",
        VKS_DUPLICATE_KEY => "variables block: duplicate key",
        VKS_UNSUPPORTED => "variables block: unsupported YAML construct",
        VKS_MIXED_LIST => "variables block: list mixes strings and records",
        VKS_INLINE_COMMENT => "variables block: comment after a value",
        VKS_CONTROL_CHAR => "variables block: control character",
        VKS_LIMIT => "variables block: size limit exceeded",
        VKS_MIXED_DIALECT => "variables block: KEY=value line in a YAML block",
        VKS_UNREPRESENTABLE => "variables block: value cannot be written",
        FILE_CONFLICT => "the file changed since it was read (hash mismatch)",
        FILE_EXISTS => "a file already exists at that path",
        ARTIFACT_VARS_INVALID => "the file's variables block is invalid; fix it before saving",
        ARTIFACT_UNREPRESENTABLE => "the artifact cannot be written in the file format",
        ARTIFACT_BLOCK_NOT_FOUND => "no block with that index and heading",
        ARTIFACT_NOT_SERIALIZABLE => "this file cannot be rewritten by the engine",
        RENDER_LIMIT => "rendering exceeds a size limit",
        RENDER_CONTAINS_ESCAPE => {
            "rendered output contains terminal control characters; use --json to receive it"
        }
        RENDER_UNJOINABLE => "the command cannot be joined into one line safely",
        NAMING_PATH_INJECTION => "the file name contains a path separator, \"..\", or NUL",
        NAMING_EMPTY => "the file name is empty",
        NAMING_EDGE_SPACE => "the file name starts or ends with a space",
        NAMING_EDGE_DOT => "the file name starts or ends with a dot",
        NAMING_ILLEGAL_CHAR => "the file name contains an illegal character (\\ / : * ? \" < > |)",
        NAMING_CONTROL_CHAR => "the file name contains a control character",
        NAMING_RESERVED => "the file name is a reserved system name",
        NAMING_TOO_LONG => "the file name is longer than 255 bytes",
        NAMING_MD_EXTENSION => "the file name already ends in .md",
        NAMING_NOT_NFC => "the file name is not in Unicode NFC form (normalize it)",
        ARTIFACT_NOT_WHOLE_FILE => "this artifact type does not write a file",
        ARTIFACT_MULTI_BLOCK => "a whole-file artifact must have a single block",
        ARTIFACT_IS_INDEX => "a template index is run, not written",
        RENDER_UNKNOWN_VAR => "variable has no value; token left as written",
        RENDER_EACH_NOT_LIST => "each: the value is not a list",
        RENDER_UNTERMINATED => "each without a matching end",
        RENDER_UNMATCHED_END => "end without a matching each",
        RENDER_SELF_NESTED => "each over a list it is already inside",
        RENDER_NOT_SCALAR => "the token reaches a list or record outside a loop",
        RENDER_JOIN_RECORD => "join reaches a record",
        RENDER_JOIN_EMPTY => "join found no values",
        PATH_EMPTY => "the path is empty",
        PATH_CONTROL_CHAR => "the path contains a control character",
        PATH_DRIVE_OR_SCHEME => "the path contains a drive letter or URI scheme (\":\")",
        PATH_BACKSLASH => "the path contains a backslash",
        PATH_ABSOLUTE => "the path is absolute",
        PATH_PARENT_SEGMENT => "the path contains a \"..\" segment",
        INDEX_NOT_INDEX => "not a template index (index: true on a Template or AIAgentsConfig)",
        INDEX_UNSLUGGABLE => "a path segment has no letters or digits to name a file",
        INDEX_LIMIT => "the index exceeds a size limit",
        _ => return None,
    })
}
