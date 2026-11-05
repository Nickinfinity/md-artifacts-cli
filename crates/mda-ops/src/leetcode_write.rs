//! `leetcode.record_outcome` (W6 D-27): one hash-checked read, the pure patches, the re-parse guard, one
//! atomic `replace`. H6.0 stub with the request/response types; T6.3 implements it.

use mda_core::leetcode::{AttemptEntry, LeetCodeStatus};

use crate::artifact_write::WriteResponse;
use crate::{Ctx, OpError, error};

/// `leetcode.record_outcome` params.
///
/// # Examples
///
/// ```
/// let r: mda_ops::leetcode_write::RecordOutcomeRequest = serde_json::from_value(serde_json::json!({
///     "path": "LeetCode/ex.md", "expectedHash": "h", "status": "solved", "language": "python"
/// })).unwrap();
/// assert!(r.solved.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecordOutcomeRequest {
    pub path: String,
    pub expected_hash: String,
    pub status: LeetCodeStatus,
    pub language: String,
    #[serde(default)]
    pub solved: Option<SolvedRequest>,
    #[serde(default)]
    pub attempt: Option<AttemptRequest>,
}

/// `solved`: when and how long; no `duration` writes no meta (Q-43).
///
/// # Examples
///
/// ```
/// let s: mda_ops::leetcode_write::SolvedRequest =
///     serde_json::from_value(serde_json::json!({"solvedAt": "2026-11-05T10:00:00Z"})).unwrap();
/// assert!(s.duration.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SolvedRequest {
    pub solved_at: String,
    #[serde(default)]
    pub duration: Option<String>,
}

/// `attempt`: one run, prepended under `# Attempts` / `## <Language>`.
///
/// # Examples
///
/// ```
/// let a: mda_ops::leetcode_write::AttemptRequest = serde_json::from_value(serde_json::json!({
///     "at": "t", "duration": "1m", "passed": true, "code": "x", "bigO": "O(n)"
/// })).unwrap();
/// assert_eq!(a.big_o.as_deref(), Some("O(n)"));
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AttemptRequest {
    pub at: String,
    pub duration: String,
    pub passed: bool,
    #[serde(default)]
    pub big_o: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
    pub code: String,
}

impl From<AttemptRequest> for AttemptEntry {
    fn from(a: AttemptRequest) -> Self {
        AttemptEntry {
            at: a.at,
            duration: a.duration,
            passed: a.passed,
            big_o: a.big_o,
            confidence: a.confidence,
            code: a.code,
        }
    }
}

/// `leetcode.record_outcome` (stub until T6.3): every request is refused `op.bad_request`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode_write};
/// let req: leetcode_write::RecordOutcomeRequest = serde_json::from_value(serde_json::json!({
///     "path": "LeetCode/ex.md", "expectedHash": "h", "status": "solved", "language": "python"
/// })).unwrap();
/// assert_eq!(leetcode_write::record_outcome(&Ctx::new(None), req).unwrap_err().code, "op.bad_request");
/// ```
pub fn record_outcome(_ctx: &Ctx, _req: RecordOutcomeRequest) -> Result<WriteResponse, OpError> {
    Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "not_implemented"))
}
