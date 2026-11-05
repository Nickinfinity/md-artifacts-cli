//! Pure, byte-surgical outcome patches of an exercise file (W6 D-27; TS `frontmatter-patcher`,
//! `attempts-writer`, `insertSolvedMeta`). H6.0 stubs with the pinned signatures; T6.1 owns this
//! file and replaces every body.

use super::{AttemptEntry, LeetCodeStatus, resolve_lang_id};
use crate::error::{BadField, NotSerializableReason};

/// Replace `status:` in place, or insert it at its canonical index. Stub until T6.1: unchanged.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeStatus, patch_status};
/// let raw = "---\nartifactType: LeetCode\nstatus: solved\n---\n";
/// assert_eq!(patch_status(raw, LeetCodeStatus::Solved).unwrap(), raw);
/// ```
pub fn patch_status(raw: &str, _status: LeetCodeStatus) -> Result<String, NotSerializableReason> {
    Ok(raw.to_owned())
}

/// Write `<!-- meta: {solvedAt, duration} -->` above the language's first bare fence in
/// `# Solutions`, replacing a meta already there (Q-56). Stub until T6.1: unchanged.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::insert_solved_meta("", "python", "2026-11-05T10:00:00Z", "5m");
/// ```
pub fn insert_solved_meta(
    raw: &str,
    _lang: &str,
    _solved_at: &str,
    _duration: &str,
) -> Result<String, NotSerializableReason> {
    Ok(raw.to_owned())
}

/// Prepend one attempt under `# Attempts` / `## <Language>`. Stub until T6.1: unchanged.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{AttemptEntry, prepend_attempt};
/// let a = AttemptEntry { at: "t".into(), duration: "1m".into(), passed: true, big_o: None,
///     confidence: None, code: "x".into() };
/// let _ = prepend_attempt("", "python", &a);
/// ```
pub fn prepend_attempt(
    raw: &str,
    _lang: &str,
    _attempt: &AttemptEntry,
) -> Result<String, NotSerializableReason> {
    Ok(raw.to_owned())
}

/// The request `language`, resolved, if it is `[A-Za-z0-9_]{1,32}` (Q-57). Stub until T6.1: resolved,
/// never refused.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::leetcode::validate_language("py").unwrap(), "python");
/// ```
pub fn validate_language(lang: &str) -> Result<String, BadField> {
    Ok(resolve_lang_id(lang))
}
