//! `leetcode.write` (W7 D-43): a whole LeetCode file from the model, re-emitting an unchanged layout
//! block's raw text byte for byte. No `expectedHash` → create (never clobbers, `file.exists`);
//! with it → hash-checked replace (`file.conflict`), the layout derived from disk (Q-78). Every write
//! passes the re-parse guard (`write_guard`), which runs before any directory is created.
//! (`leetcode_write.rs` is `leetcode.record_outcome`'s.)

use std::path::Path;

use mda_core::leetcode::{
    Block, Discriminator, LayoutRefusal, LeetCodeExercise, MAX_LAYOUT_BLOCKS, discriminator,
    emit::assemble, layout, write_guard,
};
use mda_core::parse::decode;
use mda_vault::{content_hash, create_new, read_bounded, replace};

use crate::artifact::{MAX_ARTIFACT_BYTES, leetcode_path, locate};
use crate::artifact_rewrite::read_checked_at;
use crate::artifact_write::{WriteResponse, too_large, written};
use crate::error::vault_error_at;
use crate::leetcode_create::check_new_path;
use crate::leetcode_write::refuse;
use crate::write_file::exists_actual;
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

fn bad(reason: &str) -> OpError {
    OpError::new(error::OP_BAD_REQUEST).with("reason", reason)
}

/// A supplied layout is hostile when too long or when a raw text holds NUL / U+FEFF (the decoder
/// strips the BOM, so neither can come from a file).
fn check_supplied(l: &[Block]) -> Result<(), OpError> {
    // Byte cap first, summed without building the text.
    if l.len() > MAX_LAYOUT_BLOCKS
        || l.iter().map(|b| b.raw.len() as u64).sum::<u64>() > MAX_ARTIFACT_BYTES
    {
        return Err(bad("layout_too_large"));
    }
    if l.iter().any(|b| b.raw.contains(['\0', '\u{feff}'])) {
        return Err(bad("layout"));
    }
    Ok(())
}

/// `leetcode.write` (D-43): create (no `expectedHash`, never clobbers) or hash-checked replace with
/// the layout derived from disk (Q-78). The assembled text passes `write_guard` and the size cap
/// before the first byte or directory is written.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode_put};
/// let e = mda_core::leetcode::LeetCodeExercise::default();
/// let req = leetcode_put::WriteRequest { path: "LeetCode/a.md".into(), exercise: e, layout: None, expected_hash: None };
/// assert_eq!(leetcode_put::write(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn write(ctx: &Ctx, req: WriteRequest) -> Result<WriteResponse, OpError> {
    if let Some(l) = &req.layout {
        check_supplied(l)?;
    }
    leetcode_path(&req.path)?;
    let r = locate(ctx, &req.path)?;
    let rel = Path::new(&r.rel);
    let (out, disk) = match &req.expected_hash {
        None => {
            check_new_path(&r, &req.path)?;
            // Probe first so `file.exists` wins and no assemble work runs; `create_new` stays the
            // atomic authority (a race still ends in `exists_actual`).
            if let Ok(b) = read_bounded(&r.place.root, rel, MAX_ARTIFACT_BYTES) {
                return Err(OpError::new(error::FILE_EXISTS)
                    .with("path", &req.path)
                    .with("actual", content_hash(&b)));
            }
            (assemble(&req.exercise, req.layout.as_deref()), None)
        }
        Some(h) => {
            let bytes = read_checked_at(&r, h)?;
            let text = decode(&bytes);
            if discriminator(&text) != Discriminator::Exact {
                return Err(OpError::new(error::LEETCODE_NOT_EXERCISE).with("path", r.virt));
            }
            let l =
                layout(&text).map_err(|LayoutRefusal::TooManyBlocks| bad("layout_too_large"))?;
            if let Some(s) = &req.layout
                && s.iter().map(|b| b.raw.as_str()).collect::<String>() != text
            {
                return Err(bad("layout"));
            }
            (assemble(&req.exercise, Some(&l)), Some(h.clone()))
        }
    };
    let out = out.map_err(refuse)?;
    write_guard(&req.exercise, &out).map_err(refuse)?;
    too_large(&r.virt, out.len())?;
    match disk {
        Some(h) => replace(&r.place.root, rel, out.as_bytes(), &h, MAX_ARTIFACT_BYTES)
            .map_err(|e| vault_error_at(e, &r.place.root, &r.virt))?,
        None => create_new(&r.place.root, rel, out.as_bytes())
            .map_err(|e| exists_actual(&r.place.root, rel, &req.path, e))?,
    }
    Ok(written(r.virt, &out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mda_core::leetcode::BlockKind;

    fn block(raw: String) -> Block {
        Block {
            raw,
            kind: BlockKind::Prose,
            fields: vec![],
        }
    }

    fn reason(l: &[Block]) -> String {
        check_supplied(l).unwrap_err().params["reason"].clone()
    }

    /// Q-78 bounds: the block count and the raw byte sum are refused before any parse or concat.
    #[test]
    fn supplied_layout_is_bounded_by_blocks_and_bytes() {
        let many: Vec<Block> = (0..=MAX_LAYOUT_BLOCKS)
            .map(|_| block(String::new()))
            .collect();
        assert_eq!(reason(&many), "layout_too_large");
        let half = || block("x".repeat(MAX_ARTIFACT_BYTES as usize / 2 + 1));
        let big = [half(), half()];
        assert_eq!(reason(&big), "layout_too_large");
        assert!(check_supplied(&[block("ok".into())]).is_ok());
    }
}
