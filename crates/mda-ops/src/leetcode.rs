//! `leetcode.read`: parse one LeetCode exercise with its content hash, and on request its layout
//! (W7 Q-72) — the carrier `leetcode.write` re-emits byte for byte.

use mda_core::leetcode::{Block, LayoutRefusal, LeetCodeExercise, is_exercise, layout, parse};
use mda_core::parse::decode;
use mda_vault::content_hash;

use crate::artifact::{leetcode_path, locate, read_at};
use crate::{Ctx, OpError, error};

/// `leetcode.read` params: a virtual path `LeetCode/<rel>.md`; `layout: true` adds the layout (Q-72).
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
    #[serde(default)]
    pub layout: bool,
}

/// `leetcode.read` response: the parsed exercise (warnings live inside it, D-24) and the content
/// hash of the raw bytes, which writes must send back.
///
/// # Examples
///
/// ```
/// let e = mda_core::leetcode::parse("---\ntitle: T\n---\n");
/// let r = mda_ops::leetcode::ReadResponse { exercise: e, hash: "h".into(), layout: None };
/// assert_eq!(serde_json::to_value(&r).unwrap()["hash"], "h");
/// ```
#[derive(Debug, serde::Serialize)]
pub struct ReadResponse {
    pub exercise: LeetCodeExercise,
    pub hash: String,
    /// Only when the request asked for it (Q-72): default responses are unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<Vec<Block>>,
}

/// `leetcode.read`: parse one exercise. The path rule runs before any I/O; the file is read once
/// (bounded), hashed as bytes, then decoded (BOM, CRLF) for the discriminator check and the parse.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode};
/// let req = leetcode::ReadRequest { path: "Snippets/a.md".into(), layout: false };
/// assert_eq!(leetcode::read(&Ctx::new(None), req).unwrap_err().code, "artifact.bad_path");
/// ```
pub fn read(ctx: &Ctx, req: ReadRequest) -> Result<ReadResponse, OpError> {
    leetcode_path(&req.path)?;
    let r = locate(ctx, &req.path)?;
    let bytes = read_at(&r)?;
    let hash = content_hash(&bytes);
    let text = decode(&bytes);
    if !is_exercise(&text) {
        return Err(OpError::new(error::LEETCODE_NOT_EXERCISE).with("path", r.virt));
    }
    let layout = if req.layout {
        Some(layout(&text).map_err(|LayoutRefusal::TooManyBlocks| {
            OpError::new(error::OP_BAD_REQUEST).with("reason", "layout_too_large")
        })?)
    } else {
        None
    };
    Ok(ReadResponse {
        exercise: parse(&text),
        hash,
        layout,
    })
}
