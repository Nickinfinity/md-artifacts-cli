//! Canonical frontmatter and prose items (W7 T7.2). Frontmatter is `skeleton()`'s form (Q-73).
//!
//! Each item is `\n`-terminated with no leading blank line; the assembler separates items. Every
//! emitter is checked against the engine's own parser (`probe`): text that does not parse back to the
//! model value is refused, never sanitised, so no per-rule scanner can drift from the reader.

use crate::error::NotSerializableReason as Why;
use crate::leetcode::{
    Example, ExerciseSetup, LeetCodeExercise, LeetCodeSolution, LeetCodeStatus, SkeletonInput,
    insert_solved_meta, parse, skeleton, validate_tag, validate_title,
};

/// Wrong-reason picker for code-bearing items: a ``` run anywhere closes the fence early.
fn fence_or(text: &str) -> Why {
    if text.contains("```") {
        Why::Fence
    } else {
        Why::Other
    }
}

/// `Ok(text)` when `holds(parse(wrapped))`, else `Err(why)`.
fn probe(
    text: String,
    wrapped: &str,
    why: Why,
    holds: impl Fn(&LeetCodeExercise) -> bool,
) -> Result<String, Why> {
    if holds(&parse(wrapped)) {
        Ok(text)
    } else {
        Err(why)
    }
}

/// The canonical frontmatter (`---` ... `---\n`), spec §2 order, `skeleton()`'s form (Q-73): the
/// block is cut from `skeleton()`'s own output, so there is one order authority; only `status` is
/// patched (the skeleton always writes `unsolved`; `title` is single-line, so the marker is unique).
/// `title`/`algorithm` go raw through `single_line`; a value the parser would read back differently
/// (double spaces, empty, a control char, a bad tag) is refused.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, emit::frontmatter};
/// let x = LeetCodeExercise { title: "Two Sum".into(), ..Default::default() };
/// assert!(frontmatter(&x).unwrap().starts_with("---\nartifactType: LeetCode\n"));
/// assert!(frontmatter(&LeetCodeExercise::default()).is_err());
/// ```
pub fn frontmatter(x: &LeetCodeExercise) -> Result<String, Why> {
    let bad = |_| Why::Frontmatter;
    validate_title(&x.title).map_err(bad)?;
    x.algorithm
        .as_deref()
        .map_or(Ok(()), validate_title)
        .map_err(bad)?;
    x.tags
        .iter()
        .try_for_each(|t| validate_tag(t))
        .map_err(bad)?;
    let sk = skeleton(&SkeletonInput {
        title: x.title.clone(),
        leetcode_type: x.leetcode_type,
        difficulty: x.difficulty,
        algorithm: x.algorithm.clone(),
        tags: x.tags.clone(),
    });
    let end = sk.find("\n---\n").map_or(0, |i| i + "\n---\n".len());
    let status = match x.status {
        LeetCodeStatus::Unsolved => "unsolved",
        LeetCodeStatus::Attempted => "attempted",
        LeetCodeStatus::Solved => "solved",
    };
    let fm = sk.get(..end).ok_or(Why::Frontmatter)?.replacen(
        "\nstatus: unsolved\n",
        &format!("\nstatus: {status}\n"),
        1,
    );
    probe(fm.clone(), &fm, Why::Frontmatter, |p| {
        (
            &p.title,
            p.leetcode_type,
            p.difficulty,
            p.status,
            &p.algorithm,
            &p.tags,
        ) == (
            &x.title,
            x.leetcode_type,
            x.difficulty,
            x.status,
            &x.algorithm,
            &x.tags,
        )
    })
}

/// The description prose between the frontmatter and the first section (`"" ` when empty). Refused
/// when it would not read back identical: surrounding whitespace, a heading line outside a fence, an
/// unclosed fence, a config fence.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, emit::description};
/// let x = LeetCodeExercise { description: "Given nums.".into(), ..Default::default() };
/// assert_eq!(description(&x).unwrap(), "Given nums.\n");
/// let h = LeetCodeExercise { description: "a\n# H".into(), ..Default::default() };
/// assert!(description(&h).is_err());
/// ```
pub fn description(x: &LeetCodeExercise) -> Result<String, Why> {
    let d = &x.description;
    let text = if d.is_empty() {
        String::new()
    } else {
        format!("{d}\n")
    };
    // The trailing `## Examples` makes an unclosed fence swallow it, so the mismatch shows.
    let wrapped = format!("---\n\n---\n\n{text}\n## Examples\n");
    probe(text, &wrapped, Why::Other, |p| &p.description == d)
}

/// One ```` ```example ```` fence. Refused when a value is empty, multi-line, padded, holds a ``` run
/// or U+2028/U+2029 (the reader takes one trimmed line per key).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::Example;
/// let e = Example { input: "x = 1".into(), output: "1".into() };
/// assert_eq!(mda_core::leetcode::emit::example(&e).unwrap(), "```example\ninput: x = 1\noutput: 1\n```\n");
/// ```
pub fn example(e: &Example) -> Result<String, Why> {
    let text = format!(
        "```example\ninput: {}\noutput: {}\n```\n",
        e.input, e.output
    );
    let wrapped = format!("---\n\n---\n\n## Examples\n\n{text}");
    probe(
        text,
        &wrapped,
        fence_or(&format!("{}{}", e.input, e.output)),
        |p| p.examples.len() == 1 && p.examples.first() == Some(e),
    )
}

/// A language fence under a heading: `## <lang>\n` + optional `### label` + the fence.
fn lang_item(lang: &str, label: Option<&str>, code: &str) -> String {
    let label = label.map(|l| format!("### {l}\n")).unwrap_or_default();
    format!("## {lang}\n{label}```{lang}\n{code}\n```\n")
}

/// One `# Setup` language item (`## <Lang>` + its fence). Refused when the language is not a
/// lowercase fence id, or the code holds a ``` run or ends in whitespace (the reader trims it).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::ExerciseSetup;
/// let s = ExerciseSetup { language: "python".into(), code: "pass".into() };
/// assert_eq!(mda_core::leetcode::emit::setup(&s).unwrap(), "## python\n```python\npass\n```\n");
/// ```
pub fn setup(s: &ExerciseSetup) -> Result<String, Why> {
    let text = lang_item(&s.language, None, &s.code);
    let wrapped = format!("---\n\n---\n\n# Setup\n\n{text}");
    probe(text, &wrapped, fence_or(&s.code), |p| {
        p.setups.len() == 1 && p.setups.first() == Some(s)
    })
}

/// One `# Solutions` item (heading, optional `### label`, meta comment, fence). The meta comment is
/// never hand-written: `insert_solved_meta` places it. `solved_at` without `duration` (or the
/// reverse) cannot be carried by that patcher and is refused (`Meta`), as is a value that would close
/// the comment.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetCodeSolution;
/// let s = LeetCodeSolution { language: "python".into(), label: None, code: "pass".into(), solved_at: None, duration: None };
/// assert_eq!(mda_core::leetcode::emit::solution(&s).unwrap(), "## python\n```python\npass\n```\n");
/// ```
pub fn solution(s: &LeetCodeSolution) -> Result<String, Why> {
    let plain = lang_item(&s.language, s.label.as_deref(), &s.code);
    let text = match (&s.solved_at, &s.duration) {
        (None, None) => plain,
        (Some(at), Some(dur)) => {
            let out = insert_solved_meta(&format!("# Solutions\n{plain}"), &s.language, at, dur)?;
            out.strip_prefix("# Solutions\n")
                .filter(|t| t.contains("<!-- meta:"))
                .ok_or(Why::Meta)?
                .to_owned()
        }
        _ => return Err(Why::Meta),
    };
    let wrapped = format!("---\n\n---\n\n# Solutions\n\n{text}");
    probe(text, &wrapped, fence_or(&s.code), |p| {
        p.solutions.len() == 1 && p.solutions.first() == Some(s)
    })
}

#[cfg(test)]
mod tests;
