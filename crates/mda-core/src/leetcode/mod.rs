//! LeetCode exercises (`spec/LEETCODE_FILE_FORMAT.md`): the wire model (`model`) and the parser.
//! The parser lands in W4; until then [`parse`] returns the undeclared exercise.

mod model;

pub use model::*;

/// Parse an exercise file. Never fails: malformed input degrades to the spec default plus a warning
/// (D-24). W3 stub: always the undeclared exercise.
///
/// # Examples
///
/// ```
/// let e = mda_core::leetcode::parse("---\nartifactType: LeetCode\n---\n");
/// assert_eq!(format!("{e:?}"), format!("{:?}", mda_core::leetcode::LeetCodeExercise::default()));
/// ```
pub fn parse(_src: &str) -> LeetCodeExercise {
    LeetCodeExercise::default()
}
