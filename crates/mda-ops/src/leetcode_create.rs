//! `leetcode.create` (W6 D-28): the canonical skeleton at a new path, never clobbering. H6.0 stub with
//! the request type; T6.4 implements it.

use mda_core::leetcode::{LeetCodeDifficulty, LeetcodeType, SkeletonInput};

use crate::artifact_write::WriteResponse;
use crate::{Ctx, OpError, error};

/// `leetcode.create` params.
///
/// # Examples
///
/// ```
/// let r: mda_ops::leetcode_create::CreateRequest = serde_json::from_value(serde_json::json!({
///     "path": "LeetCode/two-sum.md", "title": "Two Sum", "leetcodeType": "function",
///     "difficulty": "easy"
/// })).unwrap();
/// assert!(r.tags.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateRequest {
    pub path: String,
    pub title: String,
    pub leetcode_type: LeetcodeType,
    pub difficulty: LeetCodeDifficulty,
    #[serde(default)]
    pub algorithm: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

impl From<CreateRequest> for SkeletonInput {
    fn from(r: CreateRequest) -> Self {
        SkeletonInput {
            title: r.title,
            leetcode_type: r.leetcode_type,
            difficulty: r.difficulty,
            algorithm: r.algorithm,
            tags: r.tags.unwrap_or_default(),
        }
    }
}

/// `leetcode.create` (stub until T6.4): every request is refused `op.bad_request`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode_create};
/// let req: leetcode_create::CreateRequest = serde_json::from_value(serde_json::json!({
///     "path": "LeetCode/x.md", "title": "X", "leetcodeType": "function", "difficulty": "easy"
/// })).unwrap();
/// assert_eq!(leetcode_create::create(&Ctx::new(None), req).unwrap_err().code, "op.bad_request");
/// ```
pub fn create(_ctx: &Ctx, _req: CreateRequest) -> Result<WriteResponse, OpError> {
    Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "not_implemented"))
}
