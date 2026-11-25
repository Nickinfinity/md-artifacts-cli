//! Frontmatter of an exercise: the field accumulator (`fm.rs`), the summary, the discriminator and
//! the key order (TS `leetcode-parser.helpers.ts`, `frontmatter-order.helpers.ts`,
//! `parseFrontmatterOnly`). All entry points take decoded text (Q-36).

mod fm;
#[cfg(test)]
mod tests;

pub(crate) use fm::parse_frontmatter;

use super::config::{CANONICAL_ORDER, split_frontmatter, top_level_key};
use super::{LeetCodeSummary, truncate_param};
use crate::migrate::frontmatter::lines;
use crate::parse::text::{js_lines, js_trim};
use crate::parse::{body_range, key_of};

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

/// Classify the discriminator on the decoded text, by the rule `migrate::frontmatter::rewrite`
/// uses (the parser's `key_of` over the frontmatter lines, the value `js_trim`med, no unquoting):
/// the last `artifactType` line decides, as in the parser. With two such lines `rewrite` leaves the
/// file alone, so a last line of `leetcode` is [`Discriminator::Other`], not a pending migration.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{Discriminator, discriminator};
/// assert_eq!(discriminator(""), Discriminator::Absent);
/// ```
pub fn discriminator(src: &str) -> Discriminator {
    let Some(block) = body_range(src) else {
        return Discriminator::Absent;
    };
    let mut found = lines(src, block)
        .into_iter()
        .filter(|(_, l)| key_of(l) == Some("artifactType"))
        .map(|(_, l)| l.split_once(':').map_or("", |(_, v)| js_trim(v)))
        .peekable();
    let mut last = None;
    let mut count = 0;
    for raw in &mut found {
        last = Some(raw);
        count += 1;
    }
    match last {
        None => Discriminator::Absent,
        Some("LeetCode") => Discriminator::Exact,
        Some("leetcode") if count == 1 => Discriminator::Lowercase,
        Some(raw) => Discriminator::Other(truncate_param(raw)),
    }
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

/// The frontmatter-only summary (TS `parseFrontmatterOnly`; never reads the body). `leetcode_type`
/// is `Some` only when the frontmatter declares `leetcodeType:` (an unknown value reports the
/// parse's fallback), never derived from a `test:` block or defaulted (Q-15).
///
/// # Examples
///
/// ```
/// let s = mda_core::leetcode::summary("---\ntitle: Two Sum\n---\n");
/// assert_eq!(s.title, "Two Sum");
/// assert!(s.tags.is_empty());
/// ```
pub fn summary(src: &str) -> LeetCodeSummary {
    let fm = parse_frontmatter(split_frontmatter(src).0);
    LeetCodeSummary {
        title: fm.title.unwrap_or_default(),
        leetcode_type: fm.declared_type.is_some().then_some(fm.leetcode_type),
        difficulty: fm.difficulty,
        status: fm.status,
        algorithm: fm.algorithm,
        tags: fm.tags.unwrap_or_default(),
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

/// The first canonical-order violation of the raw frontmatter (TS `orderViolation`): the first
/// known top-level key whose canonical index is below the previous known key's. Unknown keys and
/// repeats never violate.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::leetcode::order_violation("artifactType: LeetCode\n"), None);
/// ```
pub fn order_violation(fm_raw: &str) -> Option<OrderViolation> {
    let mut last: Option<(usize, &str)> = None;
    for key in js_lines(fm_raw).filter_map(top_level_key) {
        let Some(index) = CANONICAL_ORDER.iter().position(|k| *k == key) else {
            continue;
        };
        if let Some((prev, before)) = last
            && index < prev
        {
            return Some(OrderViolation {
                key: key.to_owned(),
                before: before.to_owned(),
            });
        }
        last = Some((index, key));
    }
    None
}
