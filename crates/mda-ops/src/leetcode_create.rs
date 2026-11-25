//! `leetcode.create` (W6 D-28): the canonical skeleton at a new path, never clobbering. H6.0 stub with
//! the request type; T6.4 implements it.

use std::path::Path;

use mda_core::error::BadField;
use mda_core::leetcode::{
    LeetCodeDifficulty, LeetcodeType, SkeletonInput, skeleton, validate_tag, validate_title,
};
use mda_core::naming::validate_stem;
use mda_vault::create_new;

use crate::artifact::{bad_path, check_dirs_at, leetcode_path, locate, mid_dirs, segments};
use crate::artifact_write::{WriteResponse, too_large, written};
use crate::location::Resolved;
use crate::write_file::exists_actual;
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

/// The create-side path rule shared with `leetcode.write`: segments, mid directories, file stem.
pub(crate) fn check_new_path(r: &Resolved, path: &str) -> Result<(), OpError> {
    let segs = segments(path).map_err(|()| bad_path(path))?;
    check_dirs_at(&r.place, mid_dirs(&segs))?;
    let file = path.rsplit('/').next().unwrap_or_default();
    validate_stem(file.strip_suffix(".md").unwrap_or(file))?;
    Ok(())
}

/// `leetcode.create` (D-28): write the canonical skeleton at a new path; never overwrites
/// (`file.exists` carries `actual`). Field checks run before any I/O, then the path rule as
/// `artifact.create`'s, through `locate` (the generic locator refuses LeetCode).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, leetcode_create};
/// let req: leetcode_create::CreateRequest = serde_json::from_value(serde_json::json!({
///     "path": "LeetCode/x.md", "title": "X", "leetcodeType": "function", "difficulty": "easy"
/// })).unwrap();
/// assert_eq!(leetcode_create::create(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn create(ctx: &Ctx, req: CreateRequest) -> Result<WriteResponse, OpError> {
    let bad = |f: BadField| OpError::new(error::OP_BAD_REQUEST).with("reason", f.as_str());
    for t in std::iter::once(&req.title).chain(&req.algorithm) {
        validate_title(t).map_err(bad)?;
    }
    for t in req.tags.iter().flatten() {
        validate_tag(t).map_err(bad)?;
    }
    leetcode_path(&req.path)?;
    let r = locate(ctx, &req.path)?;
    check_new_path(&r, &req.path)?;
    let path = req.path.clone();
    let text = skeleton(&req.into());
    too_large(&path, text.len())?;
    let rel = Path::new(&r.rel);
    create_new(&r.place.root, rel, text.as_bytes())
        .map_err(|e| exists_actual(&r.place.root, rel, &path, e))?;
    Ok(written(path, &text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::MAX_ARTIFACT_BYTES;

    /// Q-49: title/tags are unbounded, so the skeleton size is the only bound; refused before any I/O.
    #[test]
    fn oversize_title_is_refused_and_nothing_is_created() {
        let dir = std::env::temp_dir().join(format!("mda-lc-create-big-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".obsidian")).unwrap();
        std::fs::create_dir_all(dir.join("LeetCode")).unwrap();
        let ctx = Ctx::new(Some(crate::Root::new(&dir).unwrap())).with_config_dir(dir.join("cfg"));
        let req = CreateRequest {
            path: "LeetCode/big/x.md".into(),
            title: "x".repeat(MAX_ARTIFACT_BYTES as usize + 1),
            leetcode_type: LeetcodeType::Function,
            difficulty: LeetCodeDifficulty::Easy,
            algorithm: None,
            tags: None,
        };
        let err = create(&ctx, req).unwrap_err();
        let (file, dir_made) = (
            dir.join("LeetCode/big/x.md").exists(),
            dir.join("LeetCode/big").exists(),
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(err.code, "file.too_large");
        assert_eq!(err.params["path"], "LeetCode/big/x.md");
        assert_eq!(err.params["max"], MAX_ARTIFACT_BYTES.to_string());
        assert!(err.params["size"].parse::<u64>().unwrap() > MAX_ARTIFACT_BYTES);
        assert!(!file && !dir_made);
    }
}
