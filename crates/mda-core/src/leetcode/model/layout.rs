//! The layout carrier (W7 D-42, spec §10.5): a LeetCode file as ordered blocks of raw text, each
//! tagged with the model fields it feeds. `leetcode.read {layout: true}` returns it;
//! `leetcode.write` re-emits an unchanged block's raw text byte for byte. Concatenating every
//! block's `raw` gives the decoded file exactly. Wire spelling (pinned at W7 H7.pre): a block is
//! `{"raw", "kind", "fields"}`, `kind` a camelCase string, a field `"title"` or `{"solutions": 0}`.

use serde::{Deserialize, Serialize};

/// The most blocks a layout may hold; checked before building (a 1 MiB file of ```` ``` ```` lines
/// would otherwise yield ~260 000 blocks). Over → `op.bad_request{reason: layout_too_large}`.
pub const MAX_LAYOUT_BLOCKS: usize = 20_000;

/// What a block is. Config fences are not one block (Q-69): opener · key groups · comments · closer.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::BlockKind;
/// assert_eq!(serde_json::to_value(BlockKind::ConfigKey).unwrap(), "configKey");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockKind {
    FmOpen,
    FmKey,
    FmClose,
    Heading,
    ConfigOpen,
    ConfigKey,
    ConfigComment,
    ConfigClose,
    CaseFence,
    CodeFence,
    FileFence,
    Comment,
    Prose,
}

/// A model field a block feeds: a spec §10 top-level key, with the item index for list fields
/// (Q-70: lists compare per index). Never deeper paths (YAGNI).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LayoutField;
/// assert_eq!(serde_json::to_value(LayoutField::Title).unwrap(), "title");
/// assert_eq!(serde_json::to_value(LayoutField::Solutions(1)).unwrap(), serde_json::json!({"solutions": 1}));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutField {
    Title,
    LeetcodeType,
    Difficulty,
    FunctionName,
    Functions,
    Algorithm,
    Status,
    Params,
    Returns,
    Description,
    Examples(usize),
    Tests(usize),
    FinalTests(usize),
    Test,
    Setups(usize),
    Practice,
    Solutions(usize),
    Attempts(usize),
    Tags,
    Files(usize),
    Libs,
    SolutionFiles(usize),
    Checks,
    Packages,
    Program,
}

/// One block: its raw text, its kind, the fields it feeds (empty for headings, prose, blank runs).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{Block, BlockKind, LayoutField};
/// let b = Block { raw: "title: Two Sum\n".into(), kind: BlockKind::FmKey, fields: vec![LayoutField::Title] };
/// assert_eq!(
///     serde_json::to_value(&b).unwrap(),
///     serde_json::json!({"raw": "title: Two Sum\n", "kind": "fmKey", "fields": ["title"]})
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub raw: String,
    pub kind: BlockKind,
    pub fields: Vec<LayoutField>,
}
