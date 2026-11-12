//! `leetcode.write` (W7 D-43): a whole LeetCode file from the model, re-emitting an unchanged layout
//! block's raw text byte for byte. No `expectedHash` → create (never clobbers, `file.exists`);
//! with it → hash-checked replace (`file.conflict`), the layout derived from disk (Q-78). Every write
//! passes the re-parse guard (`write_guard`). H7.pre stub with the request type; T7.5b implements it.
//! (`leetcode_write.rs` is `leetcode.record_outcome`'s.)

use mda_core::leetcode::{Block, LeetCodeExercise};

use crate::artifact_write::WriteResponse;
use crate::{Ctx, OpError, error};

/// `leetcode.write` params. `exercise` is the model (a `leetcode.read` response's `exercise` is taken
/// verbatim; its `warnings`/`unimplementedKinds` are ignored, Q-71); `layout` the carrier, used as-is
/// on create only.
///
/// # Examples
///
/// ```
/// let e = serde_json::to_value(mda_core::leetcode::LeetCodeExercise::default()).unwrap();
/// let r: mda_ops::leetcode_put::WriteRequest =
///     serde_json::from_value(serde_json::json!({"path": "LeetCode/a.md", "exercise": e})).unwrap();
/// assert!(r.layout.is_none() && r.expected_hash.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WriteRequest {
    pub path: String,
    pub exercise: LeetCodeExercise,
    #[serde(default)]
    pub layout: Option<Vec<Block>>,
    #[serde(default)]
    pub expected_hash: Option<String>,
}

/// `leetcode.write`: stub until T7.5b (`op.bad_request{reason: not_implemented}`).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode_put};
/// let e = mda_core::leetcode::LeetCodeExercise::default();
/// let req = leetcode_put::WriteRequest { path: "LeetCode/a.md".into(), exercise: e, layout: None, expected_hash: None };
/// assert!(leetcode_put::write(&Ctx::new(None), req).is_err());
/// ```
pub fn write(ctx: &Ctx, req: WriteRequest) -> Result<WriteResponse, OpError> {
    let _ = (ctx, req);
    Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "not_implemented"))
}
