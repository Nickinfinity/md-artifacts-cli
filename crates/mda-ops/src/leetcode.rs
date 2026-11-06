//! `leetcode.read`: parse one LeetCode exercise with its content hash. H5.0 stub; T5.1 implements
//! it on `artifact::leetcode_path`, `artifact::locate`, `artifact::read_at` and `leetcode::parse`.

use mda_core::leetcode::{LeetCodeExercise, is_exercise, parse};
use mda_core::parse::decode;
use mda_vault::content_hash;

use crate::artifact::{leetcode_path, locate, read_at};
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

/// `leetcode.read` response: the parsed exercise (warnings live inside it, D-24) and the content
/// hash of the raw bytes, which writes must send back.
///
/// # Examples
///
/// ```
/// let e = mda_core::leetcode::parse("---\ntitle: T\n---\n");
/// let r = mda_ops::leetcode::ReadResponse { exercise: e, hash: "h".into() };
/// assert_eq!(serde_json::to_value(&r).unwrap()["hash"], "h");
/// ```
#[derive(Debug, serde::Serialize)]
pub struct ReadResponse {
    pub exercise: LeetCodeExercise,
    pub hash: String,
}

/// `leetcode.read`: parse one exercise. The path rule runs before any I/O; the file is read once
/// (bounded), hashed as bytes, then decoded (BOM, CRLF) for the discriminator check and the parse.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode};
/// let req = leetcode::ReadRequest { path: "Snippets/a.md".into() };
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
    Ok(ReadResponse {
        exercise: parse(&text),
        hash,
    })
}
