//! The LeetCode structural rules 0–3 (D-29; TS `exercise-verify/frontmatter.rules.ts`), pure. H4.0a
//! stub; T4.1 owns this file.

use crate::render::Warning;

/// Every rule violation of `src`, codes from `LEETCODE_RULE_CODES`; severity is the caller's
/// (`vault.validate` reports them as errors). Stub until T4.1: always empty.
///
/// # Examples
///
/// ```
/// assert!(mda_core::leetcode::frontmatter_rules("").is_empty());
/// ```
pub fn frontmatter_rules(_src: &str) -> Vec<Warning> {
    vec![]
}
