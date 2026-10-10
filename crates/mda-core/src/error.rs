//! The `vks.*` code constants (spec §9.1) and [`VarsError`], the per-fence rejection a parsed
//! artifact carries. Codes are protocol: `mda-ops` lists them in `ALL_CODES`; renaming one is a
//! breaking change.

use std::collections::BTreeMap;

/// A line that is not an entry, item or comment, or a malformed scalar. Param `rule`.
pub const VKS_SYNTAX: &str = "vks.syntax";
/// A tab in indentation, an odd indent, or an entry at the wrong indent.
pub const VKS_INDENT: &str = "vks.indent";
/// A key outside the §9.1 key grammar. Param `key`.
pub const VKS_BAD_KEY: &str = "vks.bad_key";
/// The same key twice in one map. Param `key`.
pub const VKS_DUPLICATE_KEY: &str = "vks.duplicate_key";
/// A YAML construct the subset refuses. Param `construct`.
pub const VKS_UNSUPPORTED: &str = "vks.unsupported";
/// One list holding both strings and records.
pub const VKS_MIXED_LIST: &str = "vks.mixed_list";
/// ` #` after a value on the same line.
pub const VKS_INLINE_COMMENT: &str = "vks.inline_comment";
/// A C0 control character or DEL other than tab.
pub const VKS_CONTROL_CHAR: &str = "vks.control_char";
/// A §9.7 limit. Params `limit`, `max`.
pub const VKS_LIMIT: &str = "vks.limit";
/// A `KEY=value` line inside a fence classified as YAML.
pub const VKS_MIXED_DIALECT: &str = "vks.mixed_dialect";
/// A value the emitter cannot write (spec §9.5). Params `var`, `reason` ([`EmitReason`]).
pub const VKS_UNREPRESENTABLE: &str = "vks.unrepresentable";

/// Every `vks.*` code, for the `ALL_CODES` contract pin in `mda-ops`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::VKS_CODES.len(), 11);
/// ```
pub const VKS_CODES: &[&str] = &[
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
];

/// The `rule` vocabulary of `vks.syntax`.
///
/// # Examples
///
/// ```
/// use mda_core::error::SyntaxRule;
/// assert_eq!(SyntaxRule::DashAtColumn0.as_str(), "dash_at_column_0");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxRule {
    DashAtColumn0,
    ValueAndChildren,
    UnclosedQuote,
    BadEscape,
    BadPlain,
    ExpectedEntry,
}

impl SyntaxRule {
    /// The exact spec §9.1 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DashAtColumn0 => "dash_at_column_0",
            Self::ValueAndChildren => "value_and_children",
            Self::UnclosedQuote => "unclosed_quote",
            Self::BadEscape => "bad_escape",
            Self::BadPlain => "bad_plain",
            Self::ExpectedEntry => "expected_entry",
        }
    }
}

/// The `construct` vocabulary of `vks.unsupported`.
///
/// # Examples
///
/// ```
/// use mda_core::error::Construct;
/// assert_eq!(Construct::FlowMap.as_str(), "flow_map");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    FlowMap,
    FlowList,
    Anchor,
    Alias,
    Tag,
    MergeKey,
    Folded,
    BlockIndicator,
    BlockInList,
    NestedList,
    DocumentMarker,
    Directive,
    ComplexKey,
}

impl Construct {
    /// The exact spec §9.1 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlowMap => "flow_map",
            Self::FlowList => "flow_list",
            Self::Anchor => "anchor",
            Self::Alias => "alias",
            Self::Tag => "tag",
            Self::MergeKey => "merge_key",
            Self::Folded => "folded",
            Self::BlockIndicator => "block_indicator",
            Self::BlockInList => "block_in_list",
            Self::NestedList => "nested_list",
            Self::DocumentMarker => "document_marker",
            Self::Directive => "directive",
            Self::ComplexKey => "complex_key",
        }
    }
}

/// The `limit` vocabulary of `vks.limit`.
///
/// # Examples
///
/// ```
/// use mda_core::error::Limit;
/// assert_eq!(Limit::BodyBytes.as_str(), "body_bytes");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    BodyBytes,
    Depth,
    Nodes,
    ListItems,
    MapKeys,
    BlockDefaults,
}

impl Limit {
    /// The exact spec §9.1 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BodyBytes => "body_bytes",
            Self::Depth => "depth",
            Self::Nodes => "nodes",
            Self::ListItems => "list_items",
            Self::MapKeys => "map_keys",
            Self::BlockDefaults => "block_defaults",
        }
    }
}

/// A rejected `vks` fence: a code plus string params, `line` always present (1-based within the
/// fence body; `0` for the body-size limit). Same JSON shape as `OpError`.
///
/// # Examples
///
/// ```
/// use mda_core::error::{VarsError, VKS_BAD_KEY};
/// let e = VarsError::new(VKS_BAD_KEY, 2).with("key", "__proto__");
/// assert_eq!(e.params["line"], "2");
/// assert_eq!(e.params["key"], "__proto__");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VarsError {
    pub code: &'static str,
    pub params: BTreeMap<String, String>,
}

impl VarsError {
    /// An error at `line` (sets `params.line`).
    pub fn new(code: &'static str, line: usize) -> Self {
        let mut params = BTreeMap::new();
        params.insert("line".to_owned(), line.to_string());
        Self { code, params }
    }

    /// Add one parameter (overwrites).
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

/// The `reason` vocabulary of `vks.unrepresentable` (spec §9.5).
///
/// # Examples
///
/// ```
/// use mda_core::error::EmitReason;
/// assert_eq!(EmitReason::TrailingNewlines.as_str(), "trailing_newlines");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitReason {
    Backticks,
    TrailingNewlines,
    EmptyRecord,
}

impl EmitReason {
    /// The exact spec §9.5 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backticks => "backticks",
            Self::TrailingNewlines => "trailing_newlines",
            Self::EmptyRecord => "empty_record",
        }
    }
}

/// The `reason` vocabulary of `artifact.unrepresentable`.
///
/// # Examples
///
/// ```
/// use mda_core::error::FieldReason;
/// assert_eq!(FieldReason::RoundTrip.as_str(), "round_trip");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldReason {
    RoundTrip,
    MultiBlock,
}

impl FieldReason {
    /// The protocol spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoundTrip => "round_trip",
            Self::MultiBlock => "multi_block",
        }
    }
}

/// The `reason` vocabulary of `artifact.not_serializable`: why the engine will not rewrite a file.
///
/// # Examples
///
/// ```
/// use mda_core::error::Refusal;
/// assert_eq!(Refusal::NoFrontmatter.as_str(), "no_frontmatter");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Flagged,
    Index,
    Variables,
    NoFrontmatter,
    NoFence,
}

impl Refusal {
    /// The protocol spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flagged => "flagged",
            Self::Index => "index",
            Self::Variables => "variables",
            Self::NoFrontmatter => "no_frontmatter",
            Self::NoFence => "no_fence",
        }
    }
}

/// A value the emitter refused: the top-level var it belongs to and why.
///
/// # Examples
///
/// ```
/// use mda_core::error::{EmitReason, Unrepresentable};
/// let u = Unrepresentable { var: "VK-a".into(), reason: EmitReason::Backticks };
/// assert_eq!(u.reason.as_str(), "backticks");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unrepresentable {
    pub var: String,
    pub reason: EmitReason,
}

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

/// Every `naming.*` code, for the `ALL_CODES` contract pin in `mda-ops`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::NAMING_CODES.len(), 8);
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
];

/// Render warning: a variable with no effective value, a missing field, or a field of a string.
pub const RENDER_UNKNOWN_VAR: &str = "render.unknown_var";
/// Render warning: `each` over a value that is not a list.
pub const RENDER_EACH_NOT_LIST: &str = "render.each_not_list";
/// Render warning: `each` with no matching `end`.
pub const RENDER_UNTERMINATED: &str = "render.unterminated";
/// Render warning: `end` with no matching `each`.
pub const RENDER_UNMATCHED_END: &str = "render.unmatched_end";
/// Render warning: `each` over a path it is already inside.
pub const RENDER_SELF_NESTED: &str = "render.self_nested";
/// Render warning: a token reaching a list or record outside a loop (Choice excepted).
pub const RENDER_NOT_SCALAR: &str = "render.not_scalar";
/// Render warning: `join` reaching a record.
pub const RENDER_JOIN_RECORD: &str = "render.join_record";
/// Render warning: `join` with an empty result.
pub const RENDER_JOIN_EMPTY: &str = "render.join_empty";

/// Every render warning code (spec §10). Warnings travel inside a successful render response, so
/// none of these is an error code (`ALL_CODES` excludes them).
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::RENDER_WARNING_CODES.len(), 8);
/// ```
pub const RENDER_WARNING_CODES: &[&str] = &[
    RENDER_UNKNOWN_VAR,
    RENDER_EACH_NOT_LIST,
    RENDER_UNTERMINATED,
    RENDER_UNMATCHED_END,
    RENDER_SELF_NESTED,
    RENDER_NOT_SCALAR,
    RENDER_JOIN_RECORD,
    RENDER_JOIN_EMPTY,
];

/// The `limit` vocabulary of `render.limit` (spec §10).
///
/// # Examples
///
/// ```
/// use mda_core::error::RenderLimit;
/// assert_eq!(RenderLimit::OutputBytes.as_str(), "output_bytes");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLimit {
    OutputBytes,
    Iterations,
    Steps,
    Depth,
}

impl RenderLimit {
    /// The exact spec §10 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OutputBytes => "output_bytes",
            Self::Iterations => "iterations",
            Self::Steps => "steps",
            Self::Depth => "depth",
        }
    }
}

/// A render expansion limit was reached: the whole render is refused (never truncated).
///
/// # Examples
///
/// ```
/// use mda_core::error::{LimitExceeded, RenderLimit};
/// let e = LimitExceeded { limit: RenderLimit::Depth, max: 64 };
/// assert_eq!(e.limit.as_str(), "depth");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitExceeded {
    pub limit: RenderLimit,
    pub max: usize,
}
