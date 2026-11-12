//! Growth, the attempts section, and the orphan-heading rule (Q-70, ruling 3).

use super::{Act, Asm, Res};
use crate::leetcode::emit::{case_fence, example, file_fence, setup, solution};
use crate::leetcode::{AttemptEntry, BlockKind as K, FileSpec, LeetCodeExercise, ProjectCheck};

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

/// Several checks: a bare fence binds to none of them, so a new `## Tests` section is written
/// check by check (`check=<name>`), public cases then hidden ones, as `bind_cases` reads them.
fn by_check(x: &LeetCodeExercise, public: bool) -> Res<Option<String>> {
    let mut out = vec![];
    for c in x.checks.as_deref().unwrap_or(&[]) {
        let b = match c {
            ProjectCheck::Call(c) => &c.base,
            ProjectCheck::Build(c) => &c.base,
            ProjectCheck::DomAssert(c) => &c.base,
            ProjectCheck::CssAssert(c) => &c.base,
            ProjectCheck::Http(c) => &c.base,
        };
        let at = b.public_count.min(b.cases.len());
        let cases = if public {
            b.cases.get(..at)
        } else {
            b.cases.get(at..)
        };
        if let Some(cs) = cases.filter(|c| !c.is_empty()) {
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
            if many && old_len == 0 {
                return by_check(x, public);
            }
            let t = tail(new, old_len);
            (!t.is_empty()).then(|| case_fence(t, None)).transpose()
        };
        let each = |v: Vec<Result<String, _>>| v.into_iter().collect::<Result<Vec<_>, _>>();
        let ex = each(
            tail(&x.examples, o.examples.len())
                .iter()
                .map(example)
                .collect(),
        )?;
        let su = each(tail(&x.setups, o.setups.len()).iter().map(setup).collect())?;
        let so = each(
            tail(&x.solutions, o.solutions.len())
                .iter()
                .map(solution)
                .collect(),
        )?;
        let fi = each(files(&x.files, &o.files).iter().map(file_fence).collect())?;
        let sf = each(
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
            pick(vec![("tests", cases(&x.tests, o.tests.len(), true)?)]),
            pick(vec![(
                "finalTests",
                cases(&x.final_tests, o.final_tests.len(), false)?,
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
        let section = self.attempts_text()?;
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
