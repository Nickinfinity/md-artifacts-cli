//! The re-parse guard of `leetcode.record_outcome` (W6 D-27): everything but what the request asked to
//! change is unchanged. H6.0 stub with the pinned signature; T6.3 owns this file.

use super::{AttemptEntry, LeetCodeExercise, LeetCodeStatus};
use crate::error::NotSerializableReason;

/// What the write was asked to change.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeStatus, OutcomeExpect};
/// let w = OutcomeExpect { status: LeetCodeStatus::Solved, solved: None, attempt: None };
/// assert!(w.solved.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeExpect {
    pub status: LeetCodeStatus,
    /// `(resolved language, solvedAt, duration)`: only when `solved` came with a `duration` (Q-43).
    pub solved: Option<(String, String, String)>,
    /// `(resolved language, the whole new entry)` (Q-60).
    pub attempt: Option<(String, AttemptEntry)>,
}

/// Compare the parses before and after a write. Stub until T6.3: always accepts.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, LeetCodeStatus, OutcomeExpect, outcome_guard};
/// let e = LeetCodeExercise::default();
/// let w = OutcomeExpect { status: LeetCodeStatus::Unsolved, solved: None, attempt: None };
/// let _ = outcome_guard(&e, &e, &w);
/// ```
pub fn outcome_guard(
    _before: &LeetCodeExercise,
    _after: &LeetCodeExercise,
    _want: &OutcomeExpect,
) -> Result<(), NotSerializableReason> {
    Ok(())
}
