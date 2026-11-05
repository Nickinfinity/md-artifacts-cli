//! LeetCode parse-warning codes (spec §10.4) and the structural rule codes (D-29).

/// LeetCode parse warning (§10 W01, TS `LP:184`). Params: `key`.
pub const LEETCODE_FRONTMATTER_IGNORED_KEY: &str = "leetcode.frontmatter.ignored_key";
/// LeetCode parse warning (§10 W02, TS `LT:103`). Params: `declared`, `derived`.
pub const LEETCODE_TYPE_UNKNOWN: &str = "leetcode.type.unknown";
/// LeetCode parse warning (§10 W03, TS `CB:233`). Params: `line`.
pub const LEETCODE_CONFIG_INDENTED: &str = "leetcode.config.indented";
/// LeetCode parse warning (§10 W04, TS `CB:269`). Params: `key`.
pub const LEETCODE_CONFIG_KEY_IN_FRONTMATTER: &str = "leetcode.config.key_in_frontmatter";
/// LeetCode parse warning (§10 W05, TS `CB:283`). Params: `key`, `count`, `winner`.
pub const LEETCODE_CONFIG_DUPLICATE_KEY: &str = "leetcode.config.duplicate_key";
/// LeetCode parse warning (§10 W06, TS `CB:325`). Params: none.
pub const LEETCODE_CONFIG_UNTERMINATED_FENCE: &str = "leetcode.config.unterminated_fence";
/// LeetCode parse warning (§10 W07, TS `CB:347`). Params: none.
pub const LEETCODE_CONFIG_HIDDEN_BY_OPEN_FENCE: &str = "leetcode.config.hidden_by_open_fence";
/// LeetCode parse warning (§10 W08, TS `PG:98`). Params: none.
pub const LEETCODE_PROGRAM_NO_CHANNEL: &str = "leetcode.program.no_channel";
/// LeetCode parse warning (§10 W09, TS `PG:109`). Params: `channel`.
pub const LEETCODE_PROGRAM_UNKNOWN_CHANNEL: &str = "leetcode.program.unknown_channel";
/// LeetCode parse warning (§10 W10, TS `PG:117`). Params: `entry`, `reason`.
pub const LEETCODE_PROGRAM_ENTRY_REJECTED: &str = "leetcode.program.entry_rejected";
/// LeetCode parse warning (§10 W11, TS `PP:306`). Params: `role`.
pub const LEETCODE_FILES_UNKNOWN_ROLE: &str = "leetcode.files.unknown_role";
/// LeetCode parse warning (§10 W12, TS `PP:348`). Params: `language`.
pub const LEETCODE_LIBS_NO_REGISTRY: &str = "leetcode.libs.no_registry";
/// LeetCode parse warning (§10 W13, TS `PP:355`). Params: `specs`, `ecosystem`.
pub const LEETCODE_LIBS_REJECTED: &str = "leetcode.libs.rejected";
/// LeetCode parse warning (§10 W14, TS `PP:466`). Params: `got`.
pub const LEETCODE_CHECKS_NOT_A_LIST: &str = "leetcode.checks.not_a_list";
/// LeetCode parse warning (§10 W15, TS `PP:502`). Params: `name`, `kind`.
pub const LEETCODE_CHECKS_RESERVED_KIND: &str = "leetcode.checks.reserved_kind";
/// LeetCode parse warning (§10 W16, TS `PP:507 and PP:552`). Params: `name`, `kind`.
pub const LEETCODE_CHECKS_UNKNOWN_KIND: &str = "leetcode.checks.unknown_kind";
/// LeetCode parse warning (§10 W17, TS `PP:517`). Params: `name`.
pub const LEETCODE_CHECKS_HTTP_NEEDS_PACKAGE: &str = "leetcode.checks.http_needs_package";
/// LeetCode parse warning (§10 W18, TS `PP:526`). Params: `name`.
pub const LEETCODE_CHECKS_BUILD_NEEDS_ARGV: &str = "leetcode.checks.build_needs_argv";
/// LeetCode parse warning (§10 W19, TS `PP:535`). Params: `name`.
pub const LEETCODE_CHECKS_NEEDS_FILE: &str = "leetcode.checks.needs_file";
/// LeetCode parse warning (§10 W20, TS `PP:541`). Params: `name`.
pub const LEETCODE_CHECKS_CALL_NEEDS_FUNCTION: &str = "leetcode.checks.call_needs_function";
/// LeetCode parse warning (§10 W21, TS `PP:595`). Params: none.
pub const LEETCODE_CHECKS_FENCE_UNBOUND: &str = "leetcode.checks.fence_unbound";
/// LeetCode parse warning (§10 W22, TS `PP:600`). Params: `name`.
pub const LEETCODE_CHECKS_FENCE_UNKNOWN_CHECK: &str = "leetcode.checks.fence_unknown_check";
/// LeetCode parse warning (§10 W23, TS `PP:635`). Params: `key`, `suggestion`.
pub const LEETCODE_CONFIG_UNKNOWN_KEY: &str = "leetcode.config.unknown_key";
/// LeetCode parse warning (§10 W24, TS `PK:174`). Params: `got`.
pub const LEETCODE_PACKAGES_NOT_A_LIST: &str = "leetcode.packages.not_a_list";
/// LeetCode parse warning (§10 W25, TS `PK:216`). Params: none.
pub const LEETCODE_PACKAGES_MISSING_NAME: &str = "leetcode.packages.missing_name";
/// LeetCode parse warning (§10 W26, TS `PK:220`). Params: `name`, `dir`, `reason`.
pub const LEETCODE_PACKAGES_DIR_REJECTED: &str = "leetcode.packages.dir_rejected";
/// LeetCode parse warning (§10 W27, TS `PK:243`). Params: `name`, `field`.
pub const LEETCODE_PACKAGES_MISSING_FIELD: &str = "leetcode.packages.missing_field";
/// LeetCode parse warning (§10 W28, TS `PK:248`). Params: `name`, `field`.
pub const LEETCODE_PACKAGES_ARGV_NOT_ARRAY: &str = "leetcode.packages.argv_not_array";
/// LeetCode parse warning (§10 W29, TS `PK:252`). Params: `name`, `field`.
pub const LEETCODE_PACKAGES_ARGV_EMPTY: &str = "leetcode.packages.argv_empty";
/// LeetCode parse warning (§10 W30, TS `PK:257`). Params: `name`, `field`, `element`.
pub const LEETCODE_PACKAGES_ARGV_SUBSTITUTION: &str = "leetcode.packages.argv_substitution";
/// LeetCode parse warning (§10 W31, TS `PK:303`). Params: `name`.
pub const LEETCODE_PACKAGES_EXPOSE_NAME_INVALID: &str = "leetcode.packages.expose_name_invalid";
/// LeetCode parse warning (§10 W32, TS `PK:307`). Params: `name`.
pub const LEETCODE_PACKAGES_EXPOSE_NAME_RESERVED: &str = "leetcode.packages.expose_name_reserved";
/// LeetCode parse warning (§10 W33, TS `PK:311`). Params: `name`, `value`.
pub const LEETCODE_PACKAGES_EXPOSE_VALUE_NOT_LOOPBACK: &str =
    "leetcode.packages.expose_value_not_loopback";
/// LeetCode parse warning (§10 W34, TS `PK:187 (via PP:206)`). Params: `count`, `max`.
pub const LEETCODE_PACKAGES_TOO_MANY: &str = "leetcode.packages.too_many";
/// LeetCode parse warning (§10 W35, TS `PK:199 (via PP:206)`). Params: `name`.
pub const LEETCODE_PACKAGES_DUPLICATE_NAME: &str = "leetcode.packages.duplicate_name";
/// LeetCode parse warning (§10 W36, TS `PK:204 (via PP:206)`). Params: `path`.
pub const LEETCODE_PACKAGES_CYCLE: &str = "leetcode.packages.cycle";

/// Every LeetCode parse warning code (`spec/LEETCODE_FILE_FORMAT.md` §10.4). Like render warnings they
/// travel inside a successful response, never as errors: the parser never fails (D-24).
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::LEETCODE_WARNING_CODES.len(), 36);
/// ```
pub const LEETCODE_WARNING_CODES: &[&str] = &[
    LEETCODE_FRONTMATTER_IGNORED_KEY,
    LEETCODE_TYPE_UNKNOWN,
    LEETCODE_CONFIG_INDENTED,
    LEETCODE_CONFIG_KEY_IN_FRONTMATTER,
    LEETCODE_CONFIG_DUPLICATE_KEY,
    LEETCODE_CONFIG_UNTERMINATED_FENCE,
    LEETCODE_CONFIG_HIDDEN_BY_OPEN_FENCE,
    LEETCODE_PROGRAM_NO_CHANNEL,
    LEETCODE_PROGRAM_UNKNOWN_CHANNEL,
    LEETCODE_PROGRAM_ENTRY_REJECTED,
    LEETCODE_FILES_UNKNOWN_ROLE,
    LEETCODE_LIBS_NO_REGISTRY,
    LEETCODE_LIBS_REJECTED,
    LEETCODE_CHECKS_NOT_A_LIST,
    LEETCODE_CHECKS_RESERVED_KIND,
    LEETCODE_CHECKS_UNKNOWN_KIND,
    LEETCODE_CHECKS_HTTP_NEEDS_PACKAGE,
    LEETCODE_CHECKS_BUILD_NEEDS_ARGV,
    LEETCODE_CHECKS_NEEDS_FILE,
    LEETCODE_CHECKS_CALL_NEEDS_FUNCTION,
    LEETCODE_CHECKS_FENCE_UNBOUND,
    LEETCODE_CHECKS_FENCE_UNKNOWN_CHECK,
    LEETCODE_CONFIG_UNKNOWN_KEY,
    LEETCODE_PACKAGES_NOT_A_LIST,
    LEETCODE_PACKAGES_MISSING_NAME,
    LEETCODE_PACKAGES_DIR_REJECTED,
    LEETCODE_PACKAGES_MISSING_FIELD,
    LEETCODE_PACKAGES_ARGV_NOT_ARRAY,
    LEETCODE_PACKAGES_ARGV_EMPTY,
    LEETCODE_PACKAGES_ARGV_SUBSTITUTION,
    LEETCODE_PACKAGES_EXPOSE_NAME_INVALID,
    LEETCODE_PACKAGES_EXPOSE_NAME_RESERVED,
    LEETCODE_PACKAGES_EXPOSE_VALUE_NOT_LOOPBACK,
    LEETCODE_PACKAGES_TOO_MANY,
    LEETCODE_PACKAGES_DUPLICATE_NAME,
    LEETCODE_PACKAGES_CYCLE,
];

/// LeetCode structural rule 0 (D-29, TS `frontmatter.rules.ts`): body-set keys in the frontmatter.
/// A `vault.validate` error. Params: `keys` (`", "`-joined, D-36).
pub const LEETCODE_RULE_BODY_KEY: &str = "leetcode.rule.body_key";
/// LeetCode structural rule 1 (D-29): a legacy `type: leetcode` without `artifactType`. No params.
pub const LEETCODE_RULE_LEGACY_TYPE: &str = "leetcode.rule.legacy_type";
/// LeetCode structural rule 2 (D-29): the first canonical-order violation. Params: `key`, `before`.
pub const LEETCODE_RULE_KEY_ORDER: &str = "leetcode.rule.key_order";
/// LeetCode structural rule 3 (D-29): a discriminator present but not `LeetCode`. Params: `found`
/// (≤ 200 chars).
pub const LEETCODE_RULE_DISCRIMINATOR: &str = "leetcode.rule.discriminator";

/// The LeetCode structural rule codes (D-29): `vault.validate` **errors**, listed in `ALL_CODES`
/// (unlike [`LEETCODE_WARNING_CODES`]). Emitted by the pure rules in `leetcode/rules.rs`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::LEETCODE_RULE_CODES.len(), 4);
/// ```
pub const LEETCODE_RULE_CODES: &[&str] = &[
    LEETCODE_RULE_BODY_KEY,
    LEETCODE_RULE_LEGACY_TYPE,
    LEETCODE_RULE_KEY_ORDER,
    LEETCODE_RULE_DISCRIMINATOR,
];

/// A LeetCode write the engine refuses because the result would not re-parse as asked (W6, D-27):
/// the file is left byte-for-byte unchanged. Param `reason` ([`NotSerializableReason`]).
pub const LEETCODE_NOT_SERIALIZABLE: &str = "leetcode.not_serializable";

/// The `reason` vocabulary of `leetcode.not_serializable` (W6 H6.0).
///
/// # Examples
///
/// ```
/// use mda_core::error::NotSerializableReason;
/// assert_eq!(NotSerializableReason::Fence.as_str(), "fence");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotSerializableReason {
    /// The `status` write did not take (e.g. a config fence redeclares `status:`).
    Status,
    /// The solved meta could not be placed, or a value would close the comment (`-->`).
    Meta,
    /// The attempts after the write are not the old ones plus exactly the new entry.
    Attempts,
    /// The file has no closed frontmatter block.
    Frontmatter,
    /// Attempt code holding a code-fence run.
    Fence,
    /// Any other re-parse difference.
    Other,
}

impl NotSerializableReason {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Meta => "meta",
            Self::Attempts => "attempts",
            Self::Frontmatter => "frontmatter",
            Self::Fence => "fence",
            Self::Other => "other",
        }
    }
}

/// A request field a LeetCode write refuses before any I/O (W6 Q-58, Q-59): the `reason` of the
/// ops' `op.bad_request`.
///
/// # Examples
///
/// ```
/// use mda_core::error::BadField;
/// assert_eq!(BadField::Language.as_str(), "bad_language");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadField {
    /// `language` not `[A-Za-z0-9_]{1,32}` after `resolve_lang_id`.
    Language,
    /// `title` or `algorithm`: a control char, U+2028/U+2029, or empty after `single_line`.
    Title,
    /// A tag: as `Title`, plus `,` `[` `]` `#` or whitespace edges.
    Tag,
}

impl BadField {
    /// The `op.bad_request` `reason` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Language => "bad_language",
            Self::Title => "title",
            Self::Tag => "tag",
        }
    }
}
