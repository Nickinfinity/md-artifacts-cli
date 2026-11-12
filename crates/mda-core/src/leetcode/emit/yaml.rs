//! Canonical config key groups for the frontmatter-shaped keys `function`/`functions`/`params`/
//! `returns`/`practice` (W7 T7.3; no `test:` line, Q-76). Values quote only through `quote`.
//! H7.pre stubs with pinned signatures.

use crate::error::NotSerializableReason;
use crate::leetcode::LeetCodeExercise;

/// The signature key groups (each a key line plus its indented lines, `\n`-terminated), only keys
/// whose value is non-default (Q-73).
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::signature_groups(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn signature_groups(x: &LeetCodeExercise) -> Result<Vec<String>, NotSerializableReason> {
    let _ = x;
    Ok(vec![])
}

/// The `practice:` group, `None` when the value is the default.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::practice_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn practice_group(x: &LeetCodeExercise) -> Result<Option<String>, NotSerializableReason> {
    let _ = x;
    Ok(None)
}
