//! Growth, the attempts section, and the orphan-heading rule (Q-70, ruling 3).

use super::{Act, Asm, Res, at};
use crate::leetcode::checks::base_ref;
use crate::leetcode::emit::{case_fence, example, file_fence, setup, solution};
use crate::leetcode::{AttemptEntry, BlockKind as K, FileSpec, LeetCodeExercise};

/// Section heading lines in canonical order (spec §3.9); growth that has no home opens its section
/// before the first later one that exists.
const SECS: [&str; 7] = [
    "## Examples",
    "## Tests",
    "## Final Tests",
    "## Files",
    "# Setup",
    "# Solutions",
    "# Attempts",
];

fn tail<T>(v: &[T], old_len: usize) -> &[T] {
    v.get(old_len..).unwrap_or(&[])
}

fn files(v: &Option<Vec<FileSpec>>, old: &Option<Vec<FileSpec>>) -> Vec<FileSpec> {
    let old_len = old.as_ref().map_or(0, Vec::len);
    tail(v.as_deref().unwrap_or(&[]), old_len).to_vec()
}

/// Item texts joined by a blank line; `None` when there are none.
fn joined(texts: Vec<String>) -> Option<String> {
    (!texts.is_empty()).then(|| texts.join("\n"))
}

/// A check's public cases (`## Tests`) or its hidden ones (`## Final Tests`).
pub(super) fn section(
    c: &crate::leetcode::CheckBase,
    public: bool,
) -> &[crate::leetcode::TestCase] {
    let at = c.public_count.min(c.cases.len());
    let r = if public {
        c.cases.get(..at)
    } else {
        c.cases.get(at..)
    };
    r.unwrap_or(&[])
}

/// The check of that name, if the exercise has one.
pub(super) fn named<'a>(
    x: &'a LeetCodeExercise,
    name: &str,
) -> Option<&'a crate::leetcode::CheckBase> {
    x.checks
        .iter()
        .flatten()
        .map(base_ref)
        .find(|b| b.name == name)
}

/// Several checks: a bare fence binds to none of them, so a check's cases are written as
/// `check=<name>` fences, public ones then hidden ones, as `bind_cases` reads them. A check whose
/// old file already held cases in the section keeps its fence (`Asm::cases` re-emits it whole);
/// only a check with none there needs a new one.
fn by_check(x: &LeetCodeExercise, old: &LeetCodeExercise, public: bool) -> Res<Option<String>> {
    let mut out = vec![];
    let bases: Vec<_> = x.checks.iter().flatten().map(base_ref).collect();
    let held = |b: &crate::leetcode::CheckBase| {
        named(old, &b.name).is_some_and(|o| !section(o, public).is_empty())
    };
    for (i, b) in bases.iter().enumerate() {
        // a later check's fence exists: `Asm::lead` puts this one in front of it, in order
        if held(b) || bases.get(i + 1..).unwrap_or(&[]).iter().any(|l| held(l)) {
            continue;
        }
        let cs = section(b, public);
        if !cs.is_empty() {
            out.push(case_fence(cs, Some(&b.name))?);
        }
    }
    Ok(joined(out))
}

type Lists = Vec<(&'static str, String)>;

impl Asm<'_> {
    /// New items per section (everything past the old list's length), canonical text.
    fn new_items(&self) -> Res<[Lists; 6]> {
        let (x, o) = (self.x, self.old);
        let many = x.checks.as_ref().is_some_and(|c| c.len() > 1);
        let cases = |new: &[crate::leetcode::TestCase], old_len: usize, public: bool| {
            if many {
                return by_check(x, o, public);
            }
            let t = tail(new, old_len);
            (!t.is_empty())
                .then(|| case_fence(t, None))
                .transpose()
                .map_err(Into::into)
        };
        let each = |k: &str, v: Vec<Result<String, _>>| {
            v.into_iter()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|r: crate::error::NotSerializableReason| r.at(k))
        };
        let ex = each(
            "examples",
            tail(&x.examples, o.examples.len())
                .iter()
                .map(example)
                .collect(),
        )?;
        let su = each(
            "setups",
            tail(&x.setups, o.setups.len()).iter().map(setup).collect(),
        )?;
        let so = each(
            "solutions",
            tail(&x.solutions, o.solutions.len())
                .iter()
                .map(solution)
                .collect(),
        )?;
        let fi = each(
            "files",
            files(&x.files, &o.files).iter().map(file_fence).collect(),
        )?;
        let sf = each(
            "solutionFiles",
            files(&x.solution_files, &o.solution_files)
                .iter()
                .map(file_fence)
                .collect(),
        )?;
        let pick = |l: Vec<(&'static str, Option<String>)>| -> Lists {
            l.into_iter().filter_map(|(k, t)| Some((k, t?))).collect()
        };
        Ok([
            pick(vec![("examples", joined(ex))]),
            pick(vec![(
                "tests",
                at(cases(&x.tests, o.tests.len(), true), "tests")?,
            )]),
            pick(vec![(
                "finalTests",
                at(
                    cases(&x.final_tests, o.final_tests.len(), false),
                    "finalTests",
                )?,
            )]),
            pick(vec![("files", joined(fi))]),
            pick(vec![("setups", joined(su))]),
            pick(vec![
                ("solutions", joined(so)),
                ("solutionFiles", joined(sf)),
            ]),
        ])
    }

    pub(super) fn growth(&mut self) -> Res {
        for (s, lists) in self.new_items()?.into_iter().enumerate() {
            let mut fresh = vec![];
            for (key, text) in lists {
                let anchor = self.last.get(key).or_else(|| {
                    (key == "solutionFiles")
                        .then(|| self.last.get("solutions"))
                        .flatten()
                });
                match anchor.copied() {
                    Some(a) => self.put(a + 1, &format!("\n{text}")),
                    None => fresh.push(text),
                }
            }
            let Some(name) = SECS.get(s).filter(|_| !fresh.is_empty()) else {
                continue;
            };
            let body = fresh.join("\n");
            match self.head(name) {
                Some(h) => self.put(h + 1, &format!("\n{body}")),
                None => {
                    let at = SECS
                        .iter()
                        .skip(s + 1)
                        .find_map(|n| self.head(n))
                        .unwrap_or(self.blocks.len());
                    self.put(at, &format!("\n{name}\n\n{body}"));
                }
            }
        }
        Ok(())
    }

    /// The `# Attempts` section re-emitted whole through `prepend_attempt` (ruling 3): attempts
    /// shift on prepend, so any change counts every attempt as changed.
    pub(super) fn attempts(&mut self) -> Res {
        if !self.diff.key_changed("attempts") {
            return Ok(());
        }
        let section = at(self.attempts_text(), "attempts")?;
        let blocks = self.blocks;
        let Some(start) = self.head("# Attempts") else {
            if let Some(t) = section {
                self.put(blocks.len(), &format!("\n{t}"));
            }
            return Ok(());
        };
        let end = blocks
            .iter()
            .enumerate()
            .skip(start + 1)
            .find(|(_, b)| b.kind == K::Heading && b.raw.starts_with("# "))
            .map_or(blocks.len(), |(i, _)| i);
        for j in start..end {
            let config = blocks.get(j).is_some_and(|b| {
                matches!(
                    b.kind,
                    K::ConfigOpen | K::ConfigKey | K::ConfigComment | K::ConfigClose
                )
            });
            if !config {
                self.set(j, Act::Drop);
            }
        }
        if let Some(t) = section {
            self.set(start, Act::Replace(t, false));
        }
        Ok(())
    }

    /// Chunks in first-appearance order; inside one, entries newest first: the last attempt of a
    /// language makes its chunk, each earlier one is prepended to it.
    fn attempts_text(&self) -> Res<Option<String>> {
        let mut langs: Vec<&str> = vec![];
        for a in &self.x.attempts {
            if !langs.contains(&a.language.as_str()) {
                langs.push(&a.language);
            }
        }
        let mut sec = String::new();
        for l in langs {
            for a in self.x.attempts.iter().rev().filter(|a| a.language == l) {
                let e = AttemptEntry {
                    at: a.at.clone(),
                    duration: a.duration.clone(),
                    passed: a.passed,
                    big_o: a.big_o.clone(),
                    confidence: a.confidence.clone(),
                    code: a.code.clone(),
                };
                sec = crate::leetcode::prepend_attempt(&sec, &a.language, &e)?;
            }
        }
        Ok((!sec.is_empty()).then_some(sec))
    }

    /// A `##`/`###` heading whose fences all went (dropped, or re-emitted under a heading of their
    /// own) and none stayed guards nothing: it goes with them.
    pub(super) fn orphans(&mut self) {
        // (heading block, a fence under it went, a fence under it stayed)
        type Span = Option<(usize, bool, bool)>;
        let (mut h2, mut h3): (Span, Span) = (None, None);
        let mut doomed = vec![];
        let close = |s: Span, doomed: &mut Vec<usize>| {
            if let Some((i, true, false)) = s {
                doomed.push(i);
            }
        };
        for (j, b) in self.blocks.iter().enumerate() {
            match b.kind {
                K::Heading => {
                    let level = b.raw.bytes().take_while(|&c| c == b'#').count();
                    if level <= 3 {
                        close(h3.take(), &mut doomed);
                    }
                    if level <= 2 {
                        close(h2.take(), &mut doomed);
                    }
                    let kept = matches!(self.acts.get(j), Some(Act::Keep));
                    match level {
                        2 if kept => h2 = Some((j, false, false)),
                        3 if kept => h3 = Some((j, false, false)),
                        _ => {}
                    }
                }
                K::CodeFence | K::Comment | K::FileFence | K::CaseFence => {
                    let gone = matches!(self.acts.get(j), Some(Act::Drop | Act::Replace(_, true)));
                    for s in [h2.as_mut(), h3.as_mut()].into_iter().flatten() {
                        s.1 |= gone;
                        s.2 |= !gone;
                    }
                }
                _ => {}
            }
        }
        close(h3, &mut doomed);
        close(h2, &mut doomed);
        for j in doomed {
            self.set(j, Act::Drop);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::super::assemble;
    use crate::leetcode::{LeetCodeExercise, layout, parse, write_guard};

    const MULTI: &str = "---
artifactType: LeetCode
leetcodeType: package
title: Case
difficulty: easy
status: unsolved
tags: [leetcode]
---

```yaml leetcode
test:
  checks:
    - name: a
      kind: build
      argv: [\"make\"]
    - name: b
      kind: build
      argv: [\"make\", \"b\"]
```

## Tests

```json check=a
[{\"input\": {}, \"expected\": 1}]
```

```json check=b
[{\"input\": {}, \"expected\": 2}]
```

## Final Tests

```json check=b
[{\"input\": {}, \"expected\": 3}]
```
";

    /// Adds a case to check `i`, at the end of its public run or of its hidden run, and mirrors it
    /// into the flat `tests` / `finalTests` at the position that keeps them in check order.
    fn add(v: &mut Value, i: usize, public: bool, expected: u32) {
        let case = json!({"input": {}, "expected": expected});
        let cs = v["checks"].as_array().unwrap().clone();
        fn n(c: &Value, hidden: bool) -> u64 {
            let all = c["cases"].as_array().unwrap().len() as u64;
            let p = c["publicCount"].as_u64().unwrap();
            if hidden { all - p } else { p }
        }
        let before: u64 = cs[..i].iter().map(|c| n(c, !public)).sum();
        let at = (before + n(&cs[i], !public)) as usize;
        let list = if public { "tests" } else { "finalTests" };
        v[list].as_array_mut().unwrap().insert(at, case.clone());
        let b = &mut v["checks"][i];
        let in_check = if public {
            n(&cs[i], false)
        } else {
            n(&cs[i], false) + n(&cs[i], true)
        } as usize;
        b["cases"].as_array_mut().unwrap().insert(in_check, case);
        if public {
            b["publicCount"] = json!(n(&cs[i], false) + 1);
        }
    }

    fn grown(edits: &[(usize, bool, u32)]) -> LeetCodeExercise {
        let mut v: Value = serde_json::to_value(parse(MULTI)).unwrap();
        for &(i, public, e) in edits {
            add(&mut v, i, public, e);
        }
        serde_json::from_value(v).unwrap()
    }

    fn counts(out: &str) -> Vec<usize> {
        let v = serde_json::to_value(parse(out)).unwrap();
        (0..2)
            .map(|i| v["checks"][i]["cases"].as_array().map_or(0, Vec::len))
            .collect()
    }

    #[test]
    fn new_cases_in_a_multi_check_project_bind_to_their_check() {
        let x = parse(MULTI);
        assert!(x.warnings.is_empty(), "{:?}", x.warnings);
        let blocks = layout(MULTI).unwrap();
        // (edits, cases per check afterwards)
        type Edits = &'static [(usize, bool, u32)];
        let table: [(Edits, [usize; 2]); 7] = [
            (&[(1, true, 9)], [1, 3]),
            (&[(1, false, 9)], [1, 3]),
            (&[(0, true, 9)], [2, 2]),
            (&[(0, false, 9)], [2, 2]),
            (&[(0, true, 8), (1, true, 9)], [2, 3]),
            (&[(0, false, 8), (1, false, 9), (0, true, 7)], [3, 3]),
            (&[(0, true, 8), (0, true, 9), (1, false, 7)], [3, 3]),
        ];
        for (edits, want) in table {
            let y = grown(edits);
            let out = assemble(&y, Some(&blocks)).unwrap_or_else(|e| panic!("{edits:?}: {e:?}"));
            assert_eq!(write_guard(&y, &out), Ok(()), "{edits:?}\n{out}");
            assert_eq!(counts(&out), want, "{edits:?}\n{out}");
        }
    }

    #[test]
    fn renaming_a_check_is_refused_never_mis_bound() {
        let blocks = layout(MULTI).unwrap();
        let mut v: Value = serde_json::to_value(parse(MULTI)).unwrap();
        v["checks"][1]["name"] = json!("c");
        let y: LeetCodeExercise = serde_json::from_value(v).unwrap();
        // the old `check=b` fences name nothing now: the guard catches the mismatch
        let r = assemble(&y, Some(&blocks));
        assert!(r.map_or(true, |out| write_guard(&y, &out).is_err()));
    }
}
