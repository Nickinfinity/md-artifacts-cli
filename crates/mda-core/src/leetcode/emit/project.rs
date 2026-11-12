//! Canonical project content (W7 T7.4): the whole `test:` group (scalars + `checks`, Q-76), `libs`,
//! `packages`, `program`, and `## Files` path fences. Values quote only through `quote`. H7.pre stubs.

use crate::error::NotSerializableReason;
use crate::leetcode::{FileSpec, LeetCodeExercise};

/// The `test:` group (header, scalars, `checks`), `None` when the value is the default.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::test_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn test_group(x: &LeetCodeExercise) -> Result<Option<String>, NotSerializableReason> {
    let _ = x;
    Ok(None)
}

/// The `libs:` group, `None` when absent.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::libs_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn libs_group(x: &LeetCodeExercise) -> Result<Option<String>, NotSerializableReason> {
    let _ = x;
    Ok(None)
}

/// The `packages:` group, `None` when absent.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::packages_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn packages_group(x: &LeetCodeExercise) -> Result<Option<String>, NotSerializableReason> {
    let _ = x;
    Ok(None)
}

/// The `program:` group, `None` when absent.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::program_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn program_group(x: &LeetCodeExercise) -> Result<Option<String>, NotSerializableReason> {
    let _ = x;
    Ok(None)
}

/// One `## Files` path fence (```` ```<lang> path=… role=… ````).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{FileRole, FileSpec};
/// let f = FileSpec { path: "a.py".into(), language: "python".into(), role: FileRole::Editable, content: "x".into() };
/// let _ = mda_core::leetcode::emit::file_fence(&f);
/// ```
pub fn file_fence(f: &FileSpec) -> Result<String, NotSerializableReason> {
    let _ = f;
    Ok(String::new())
}
