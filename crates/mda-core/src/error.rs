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

/// Every `vks.*` code, for the `ALL_CODES` contract pin in `mda-ops`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::VKS_CODES.len(), 10);
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
