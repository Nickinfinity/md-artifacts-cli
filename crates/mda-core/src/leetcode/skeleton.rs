//! The canonical skeleton of a new exercise (W6 D-28; the bytes are `spec/LEETCODE_FILE_FORMAT.md` §3.8). H6.0 stubs with the
//! pinned signatures; T6.2 owns this file and replaces every body.

use super::{LeetCodeDifficulty, LeetcodeType};
use crate::error::BadField;

/// What `skeleton` needs; the op validates every field first (`validate_title`, `validate_tag`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeDifficulty, LeetcodeType, SkeletonInput};
/// let i = SkeletonInput { title: "Two Sum".into(), leetcode_type: LeetcodeType::Function,
///     difficulty: LeetCodeDifficulty::Easy, algorithm: None, tags: vec![] };
/// assert_eq!(i.title, "Two Sum");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonInput {
    pub title: String,
    pub leetcode_type: LeetcodeType,
    pub difficulty: LeetCodeDifficulty,
    pub algorithm: Option<String>,
    pub tags: Vec<String>,
}

/// The skeleton file text. Stub until T6.2: empty.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeDifficulty, LeetcodeType, SkeletonInput, skeleton};
/// let i = SkeletonInput { title: "Two Sum".into(), leetcode_type: LeetcodeType::Function,
///     difficulty: LeetCodeDifficulty::Easy, algorithm: None, tags: vec![] };
/// let _ = skeleton(&i);
/// ```
pub fn skeleton(_input: &SkeletonInput) -> String {
    String::new()
}

/// A `title` or `algorithm` the skeleton can write (Q-55, Q-58). Stub until T6.2: never refused.
///
/// # Examples
///
/// ```
/// assert!(mda_core::leetcode::validate_title("Two Sum").is_ok());
/// ```
pub fn validate_title(_title: &str) -> Result<(), BadField> {
    Ok(())
}

/// A tag the skeleton can write inline (Q-58). Stub until T6.2: never refused.
///
/// # Examples
///
/// ```
/// assert!(mda_core::leetcode::validate_tag("arrays").is_ok());
/// ```
pub fn validate_tag(_tag: &str) -> Result<(), BadField> {
    Ok(())
}
