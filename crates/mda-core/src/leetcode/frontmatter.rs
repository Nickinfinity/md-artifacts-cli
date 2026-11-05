//! Frontmatter of an exercise: summary, discriminator, key order (TS `leetcode-parser.helpers.ts`,
//! `frontmatter-order.helpers.ts`). H4.0a stubs of the W5-consumed exports; T4.1 owns this file.

use super::{LeetCodeDifficulty, LeetCodeStatus, LeetCodeSummary};

/// The `artifactType` discriminator of a file (D-25, D-29, Q-21), classified on the decoded text.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{Discriminator, discriminator};
/// assert_eq!(discriminator("# just a note\n"), Discriminator::Absent);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discriminator {
    /// No `artifactType` key (a note, a README).
    Absent,
    /// The extension's lowercase `leetcode` (migration pending, D-37).
    Lowercase,
    /// Any other value, quoted or commented ones included; `found` ≤ 200 chars.
    Other(String),
    /// Exactly `artifactType: LeetCode`.
    Exact,
}

/// Classify the discriminator. Stub until T4.1: always [`Discriminator::Absent`].
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{Discriminator, discriminator};
/// assert_eq!(discriminator(""), Discriminator::Absent);
/// ```
pub fn discriminator(_src: &str) -> Discriminator {
    Discriminator::Absent
}

/// `true` iff the file is an exercise: the one predicate of D-25.
///
/// # Examples
///
/// ```
/// assert!(!mda_core::leetcode::is_exercise("# note\n"));
/// ```
pub fn is_exercise(src: &str) -> bool {
    discriminator(src) == Discriminator::Exact
}

/// The frontmatter-only summary (TS `parseFrontmatterOnly`; never the full parser). Stub until
/// T4.1: the undeclared summary.
///
/// # Examples
///
/// ```
/// let s = mda_core::leetcode::summary("");
/// assert_eq!(s.title, "");
/// assert!(s.tags.is_empty());
/// ```
pub fn summary(_src: &str) -> LeetCodeSummary {
    LeetCodeSummary {
        title: String::new(),
        leetcode_type: None,
        difficulty: LeetCodeDifficulty::Easy,
        status: LeetCodeStatus::Unsolved,
        algorithm: None,
        tags: vec![],
    }
}

/// The first canonical-order violation: `key` must come before `before` (D-29 rule 2 params; TS
/// `orderViolation` returns prose, the engine returns data).
///
/// # Examples
///
/// ```
/// let v = mda_core::leetcode::OrderViolation { key: "leetcodeType".into(), before: "title".into() };
/// assert_eq!(v.key, "leetcodeType");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderViolation {
    pub key: String,
    pub before: String,
}

/// The first canonical-order violation of the raw frontmatter (TS `orderViolation`). Stub until
/// T4.1: always `None`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::leetcode::order_violation("artifactType: LeetCode\n"), None);
/// ```
pub fn order_violation(_fm_raw: &str) -> Option<OrderViolation> {
    None
}
