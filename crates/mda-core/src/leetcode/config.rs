//! Config blocks and the frontmatter/body split (TS `leetcode-config-blocks.helpers.ts`). H4.0a stub
//! of the W5-consumed export; T4.0 owns this file and ports the rest.

/// Body-set keys found in the frontmatter, in first-seen order, each once (TS
/// `legacyFrontmatterKeys`). Stub until T4.0: always empty.
///
/// # Examples
///
/// ```
/// assert!(mda_core::leetcode::legacy_frontmatter_keys("title: X").is_empty());
/// ```
pub fn legacy_frontmatter_keys(_fm_raw: &str) -> Vec<String> {
    vec![]
}
