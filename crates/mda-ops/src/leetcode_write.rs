//! `leetcode.record_outcome` (W6 D-27): one hash-checked read, the pure patches, the re-parse guard, one
//! atomic `replace`. H6.0 stub with the request/response types; T6.3 implements it.

use std::path::Path;

use mda_core::error::NotSerializableReason;
use mda_core::leetcode::{
    AttemptEntry, Discriminator, LeetCodeStatus, OutcomeExpect, discriminator, insert_solved_meta,
    outcome_guard, parse, patch_status, prepend_attempt, validate_language,
};
use mda_core::parse::decode;
use mda_vault::{content_hash, replace};

use crate::artifact::{MAX_ARTIFACT_BYTES, leetcode_path, locate};
use crate::artifact_rewrite::read_checked_at;
use crate::artifact_write::{WriteResponse, too_large, written};
use crate::error::vault_error_at;
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

/// 64 KiB: the largest attempt `code` (Q-47).
const MAX_CODE: usize = 64 * 1024;

fn bad(reason: &str) -> OpError {
    OpError::new(error::OP_BAD_REQUEST).with("reason", reason)
}

fn refuse(r: NotSerializableReason) -> OpError {
    OpError::new(error::LEETCODE_NOT_SERIALIZABLE).with("reason", r.as_str())
}

/// `leetcode.record_outcome`: set the status, optionally stamp the solved meta and prepend one
/// attempt, in one hash-checked, re-parse-guarded atomic write. Pure request checks run first; a
/// request that changes nothing writes nothing and returns the current `{path, hash}`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode_write};
/// let req: leetcode_write::RecordOutcomeRequest = serde_json::from_value(serde_json::json!({
///     "path": "LeetCode/ex.md", "expectedHash": "h", "status": "solved", "language": "python"
/// })).unwrap();
/// assert_eq!(leetcode_write::record_outcome(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn record_outcome(ctx: &Ctx, req: RecordOutcomeRequest) -> Result<WriteResponse, OpError> {
    let lang = validate_language(&req.language).map_err(|e| bad(e.as_str()))?;
    if req
        .attempt
        .as_ref()
        .is_some_and(|a| a.code.len() > MAX_CODE)
    {
        return Err(bad("code_too_large"));
    }
    if req.solved.is_some() && req.status != LeetCodeStatus::Solved {
        return Err(bad("solved_requires_status"));
    }
    leetcode_path(&req.path)?;
    let r = locate(ctx, &req.path)?;
    let bytes = read_checked_at(&r, &req.expected_hash)?;
    let text = decode(&bytes);
    if discriminator(&text) != Discriminator::Exact {
        return Err(OpError::new(error::LEETCODE_NOT_EXERCISE).with("path", r.virt));
    }
    let before = parse(&text);
    let RecordOutcomeRequest {
        status,
        solved,
        attempt,
        ..
    } = req;
    // Q-43: no `duration` writes no meta, so nothing is expected of the solutions.
    let solved = solved.and_then(|s| Some((s.solved_at, s.duration.filter(|d| !d.is_empty())?)));
    if solved.is_none() && attempt.is_none() && status == before.status {
        return Ok(WriteResponse {
            path: r.virt,
            hash: content_hash(&bytes),
        });
    }
    let mut out = text;
    if status != before.status {
        out = patch_status(&out, status).map_err(refuse)?;
    }
    if let Some((at, dur)) = &solved {
        out = insert_solved_meta(&out, &lang, at, dur).map_err(refuse)?;
    }
    let attempt: Option<AttemptEntry> = attempt.map(Into::into);
    if let Some(a) = &attempt {
        out = prepend_attempt(&out, &lang, a).map_err(refuse)?;
    }
    let want = OutcomeExpect {
        status,
        solved: solved.map(|(at, d)| (lang.clone(), at, d)),
        attempt: attempt.map(|a| (lang, a)),
    };
    outcome_guard(&before, &parse(&out), &want).map_err(refuse)?;
    too_large(&r.virt, out.len())?;
    replace(
        &r.place.root,
        Path::new(&r.rel),
        out.as_bytes(),
        &req.expected_hash,
        MAX_ARTIFACT_BYTES,
    )
    .map_err(|e| vault_error_at(e, &r.place.root, &r.virt))?;
    Ok(written(r.virt, &out))
}
