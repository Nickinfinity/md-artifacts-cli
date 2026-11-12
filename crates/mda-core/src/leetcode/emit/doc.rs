//! Canonical frontmatter and prose items (W7 T7.2). Frontmatter is `skeleton()`'s form (Q-73).
//! H7.pre stubs with pinned signatures: each returns wrong-but-plausible empty text.

use crate::error::NotSerializableReason;
use crate::leetcode::{Example, ExerciseSetup, LeetCodeExercise, LeetCodeSolution};

/// The canonical frontmatter (`---` … `---\n`), spec §2 order, `skeleton()` form.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::frontmatter(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn frontmatter(x: &LeetCodeExercise) -> Result<String, NotSerializableReason> {
    let _ = x;
    Ok(String::new())
}

/// The description prose between the frontmatter and the first section.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::description(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn description(x: &LeetCodeExercise) -> Result<String, NotSerializableReason> {
    let _ = x;
    Ok(String::new())
}

/// One ```` ```example ```` fence.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::Example;
/// let _ = mda_core::leetcode::emit::example(&Example { input: "x".into(), output: "y".into() });
/// ```
pub fn example(e: &Example) -> Result<String, NotSerializableReason> {
    let _ = e;
    Ok(String::new())
}

/// One `# Setup` language item (`## <Lang>` + its fence).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::ExerciseSetup;
/// let _ = mda_core::leetcode::emit::setup(&ExerciseSetup { language: "python".into(), code: "pass".into() });
/// ```
pub fn setup(s: &ExerciseSetup) -> Result<String, NotSerializableReason> {
    let _ = s;
    Ok(String::new())
}

/// One `# Solutions` item (heading, optional `### label`, meta comment, fence).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetCodeSolution;
/// let s = LeetCodeSolution { language: "python".into(), label: None, code: "pass".into(), solved_at: None, duration: None };
/// let _ = mda_core::leetcode::emit::solution(&s);
/// ```
pub fn solution(s: &LeetCodeSolution) -> Result<String, NotSerializableReason> {
    let _ = s;
    Ok(String::new())
}
