//! The re-parse guard of `leetcode.record_outcome` (W6 D-27): everything but what the request asked to
//! change is unchanged. H6.0 stub with the pinned signature; T6.3 owns this file.

use super::{Attempt, AttemptEntry, LeetCodeExercise, LeetCodeStatus, resolve_lang_id};
use crate::error::NotSerializableReason;

type MetaOf<'a> = (&'a String, &'a Option<String>, &'a Option<String>);

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

/// Compare the parses before and after a write: `status` is the requested one; solutions differ
/// only by the target's `solvedAt`/`duration` (which must equal the request); attempts are the old
/// ones plus exactly the new entry, first under its language; everything else, warnings included,
/// is equal. Checked in that order, so the reason names the first broken promise. A value that the
/// parser reads back differently (`}` in a meta, U+2028) fails here, before any byte is written.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, LeetCodeStatus, OutcomeExpect, outcome_guard};
/// let e = LeetCodeExercise::default();
/// let w = OutcomeExpect { status: LeetCodeStatus::Unsolved, solved: None, attempt: None };
/// assert!(outcome_guard(&e, &e, &w).is_ok());
/// let w = OutcomeExpect { status: LeetCodeStatus::Solved, ..w };
/// assert!(outcome_guard(&e, &e, &w).is_err());
/// ```
pub fn outcome_guard(
    before: &LeetCodeExercise,
    after: &LeetCodeExercise,
    want: &OutcomeExpect,
) -> Result<(), NotSerializableReason> {
    use NotSerializableReason as Why;
    if after.status != want.status {
        return Err(Why::Status);
    }
    let target = want.solved.as_ref();
    let is_target =
        |lang: &str| target.is_some_and(|(l, _, _)| resolve_lang_id(lang) == resolve_lang_id(l));
    if let Some((l, at, dur)) = target {
        let landed = after.solutions.iter().any(|s| {
            resolve_lang_id(&s.language) == resolve_lang_id(l)
                && s.solved_at.as_deref() == Some(at)
                && s.duration.as_deref() == Some(dur)
        });
        if !landed {
            return Err(Why::Meta);
        }
    }
    fn meta<'a>(e: &'a LeetCodeExercise, skip: &dyn Fn(&str) -> bool) -> Vec<MetaOf<'a>> {
        e.solutions
            .iter()
            .filter(|s| !skip(&s.language))
            .map(|s| (&s.language, &s.solved_at, &s.duration))
            .collect()
    }
    if meta(before, &is_target) != meta(after, &is_target) {
        return Err(Why::Meta);
    }
    let mut rest = after.attempts.clone();
    if let Some((lang, e)) = &want.attempt {
        let lang = resolve_lang_id(lang);
        // The entry lands under a heading that resolves to `lang` (`## py` for python, as TS
        // findLanguageChunk); `language` is that heading's raw lowercase text, so take it from the match.
        let Some(i) = rest
            .iter()
            .position(|a| resolve_lang_id(&a.language) == lang)
        else {
            return Err(Why::Attempts);
        };
        let got = rest.get(i).ok_or(Why::Attempts)?;
        let new = Attempt {
            language: got.language.clone(),
            code: e.code.trim_end().to_owned(),
            at: e.at.clone(),
            duration: e.duration.clone(),
            passed: e.passed,
            big_o: e.big_o.clone(),
            confidence: e.confidence.clone(),
        };
        if *got != new {
            return Err(Why::Attempts);
        }
        rest.remove(i);
    }
    if rest != before.attempts {
        return Err(Why::Attempts);
    }
    // Everything else: solutions' code/labels, config, warnings. The parts compared above are blanked.
    let blank = |e: &LeetCodeExercise| {
        let mut e = e.clone();
        e.status = want.status;
        e.attempts.clear();
        for s in &mut e.solutions {
            s.solved_at = None;
            s.duration = None;
        }
        serde_json::to_value(e).ok()
    };
    if blank(before) != blank(after) {
        return Err(Why::Other);
    }
    Ok(())
}

/// The re-parse guard of `leetcode.write` (W7 Q-71): `to_value(parse(out))` equals `want` with
/// `warnings` and `unimplementedKinds` removed on both sides (`to_value`, never `==`: `Float(200.0)`
/// and `Int(200)` print alike). H7.pre stub with the pinned signature; T7.5a owns it.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, write_guard};
/// let _ = write_guard(&LeetCodeExercise::default(), "");
/// ```
pub fn write_guard(want: &LeetCodeExercise, out: &str) -> Result<(), NotSerializableReason> {
    let _ = (want, out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leetcode::parse;

    const BASE: &str = "---\nartifactType: LeetCode\ntitle: T\nstatus: attempted\n---\n\n# Solutions\n\n## Python\n```python\nx\n```\n";
    const META: &str = "<!-- meta: { \"solved_at\": \"s\", \"duration\": \"d\" } -->\n";

    fn want(status: LeetCodeStatus) -> OutcomeExpect {
        OutcomeExpect {
            status,
            solved: None,
            attempt: None,
        }
    }
    fn solved(l: &str, at: &str, d: &str) -> Option<(String, String, String)> {
        Some((l.into(), at.into(), d.into()))
    }
    fn run(after: &str, w: &OutcomeExpect) -> Result<(), NotSerializableReason> {
        outcome_guard(&parse(BASE), &parse(after), w)
    }

    #[test]
    fn identical_parse_and_status_ok() {
        assert_eq!(run(BASE, &want(LeetCodeStatus::Attempted)), Ok(()));
    }

    #[test]
    fn wrong_status_is_status() {
        assert_eq!(
            run(BASE, &want(LeetCodeStatus::Solved)),
            Err(NotSerializableReason::Status)
        );
    }

    #[test]
    fn meta_must_land_exactly() {
        let after = BASE.replace("```python\nx", &format!("{META}```python\nx"));
        let mut w = want(LeetCodeStatus::Attempted);
        w.solved = solved("python", "s", "d");
        assert_eq!(run(&after, &w), Ok(()));
        w.solved = solved("python", "s", "other");
        assert_eq!(run(&after, &w), Err(NotSerializableReason::Meta));
        w.solved = None; // no duration asked: solutions must be unchanged
        assert_eq!(run(&after, &w), Err(NotSerializableReason::Meta));
        w.solved = solved("python", "s", "d");
        assert_eq!(run(BASE, &w), Err(NotSerializableReason::Meta));
    }

    #[test]
    fn attempts_are_old_plus_exactly_the_entry() {
        let e = AttemptEntry {
            at: "a".into(),
            duration: "1s".into(),
            passed: true,
            big_o: None,
            confidence: None,
            code: "y\n".into(),
        };
        let after = format!(
            "{BASE}\n# Attempts\n\n## python\n<!-- attempt: {{\"at\":\"a\",\"duration\":\"1s\",\"passed\":true}} -->\n```python\ny\n```\n"
        );
        let mut w = want(LeetCodeStatus::Attempted);
        w.attempt = Some(("python".into(), e.clone()));
        assert_eq!(run(&after, &w), Ok(()));
        assert_eq!(run(BASE, &w), Err(NotSerializableReason::Attempts));
        w.attempt = None;
        assert_eq!(run(&after, &w), Err(NotSerializableReason::Attempts));
        let mut bad = e;
        bad.at = "z".into();
        w.attempt = Some(("python".into(), bad));
        assert_eq!(run(&after, &w), Err(NotSerializableReason::Attempts));
    }

    #[test]
    fn attempt_under_an_alias_heading_is_ok() {
        let e = AttemptEntry {
            at: "a".into(),
            duration: "1s".into(),
            passed: true,
            big_o: None,
            confidence: None,
            code: "y\n".into(),
        };
        let base = BASE.to_owned()
            + "\n# Attempts\n\n## py\n<!-- attempt: {\"at\":\"o\",\"duration\":\"1s\",\"passed\":false} -->\n```python\nz\n```\n";
        let after = base.replace("<!-- attempt: {\"at\":\"o\"", "<!-- attempt: {\"at\":\"a\",\"duration\":\"1s\",\"passed\":true} -->\n```python\ny\n```\n<!-- attempt: {\"at\":\"o\"");
        let mut w = want(LeetCodeStatus::Attempted);
        w.attempt = Some(("python".into(), e));
        assert_eq!(outcome_guard(&parse(&base), &parse(&after), &w), Ok(()));
    }

    #[test]
    fn any_other_difference_is_other() {
        let after = BASE.replace("\nx\n", "\nz\n");
        assert_eq!(
            run(&after, &want(LeetCodeStatus::Attempted)),
            Err(NotSerializableReason::Other)
        );
        let warned = BASE.replace("title: T", "title: T\ndifficulty: hard");
        assert_eq!(
            run(&warned, &want(LeetCodeStatus::Attempted)),
            Err(NotSerializableReason::Other)
        );
    }
}
