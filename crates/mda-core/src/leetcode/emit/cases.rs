//! Canonical case fences (W7 T7.3; the YAML emitter W4 Q-31 deferred). H7.pre stub.

use crate::error::NotSerializableReason;
use crate::leetcode::TestCase;

/// One case fence holding `cases`, bound to `check` (a `check="…"` attribute) when given.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::case_fence(&[], None);
/// ```
pub fn case_fence(
    cases: &[TestCase],
    check: Option<&str>,
) -> Result<String, NotSerializableReason> {
    let _ = (cases, check);
    Ok(String::new())
}
