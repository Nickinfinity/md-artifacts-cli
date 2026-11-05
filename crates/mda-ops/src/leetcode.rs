//! `leetcode.read`: parse one LeetCode exercise with its content hash. H5.0 stub; T5.1 implements
//! it on `artifact::leetcode_path`, `artifact::locate`, `artifact::read_at` and `leetcode::parse`.

use crate::{Ctx, OpError, error};

/// `leetcode.read` params: a virtual path `LeetCode/<rel>.md`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::leetcode::ReadRequest =
///     serde_json::from_value(serde_json::json!({"path": "LeetCode/ex.md"})).unwrap();
/// assert_eq!(r.path, "LeetCode/ex.md");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    pub path: String,
}

/// `leetcode.read` (stub until T5.1): every request is refused `op.bad_request`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode};
/// let req = leetcode::ReadRequest { path: "LeetCode/ex.md".into() };
/// assert_eq!(leetcode::read(&Ctx::new(None), req).unwrap_err().code, "op.bad_request");
/// ```
pub fn read(_ctx: &Ctx, _req: ReadRequest) -> Result<serde_json::Value, OpError> {
    Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "not_implemented"))
}
