//! LeetCode exercises (`spec/LEETCODE_FILE_FORMAT.md`): the wire model (`model`) and the parser.
//! The parser lands in W4; until then [`parse`] returns the undeclared exercise. Module list:
//! orchestrator-owned (H4.0a); a task may turn a module it owns into `<name>/mod.rs`.

mod bounds;
mod cases;
mod checks;
mod config;
mod fm_blocks;
mod frontmatter;
mod kind;
mod libs;
mod model;
mod packages;
mod program;
mod project;
mod rules;
mod safe_json;
mod sections;
mod shared;

pub use config::legacy_frontmatter_keys;
pub use frontmatter::{
    Discriminator, OrderViolation, discriminator, is_exercise, order_violation, summary,
};
pub use model::*;
pub use rules::frontmatter_rules;
pub use shared::{MAX_PARAM_CHARS, resolve_lang_id, truncate_param};

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
