//! Per-block decisions for changed fields: list items (one emit per index, ruling 2), case fences
//! (Q-82), the description, and the shrink rule (a tail item's blocks go).

use super::grow::{named, section};
use super::{Act, Asm, Diff, Res, at};
use crate::leetcode::checks::base_ref;
use crate::leetcode::emit::{case_fence, example, file_fence, setup, solution};
use crate::leetcode::project::attr;
use crate::leetcode::{Block, BlockKind as K, CheckBase, LayoutField, LeetCodeExercise};
use crate::parse::text::js_trim;

/// A language-keyed item keeps its raw `## <lang>` heading while its language is unchanged
/// (ruling 1): the emitted text then loses its own heading line.
fn same_lang(a: Option<&String>, b: Option<&String>) -> bool {
    a.is_some() && a == b
}

fn without_first_line(t: String) -> String {
    t.split_once('\n').map_or(t.clone(), |(_, r)| r.to_owned())
}

/// `(text, text opens with a heading, heading kept raw)` of list item `i`; `None` text = shrunk away.
type Item = (Option<String>, bool, bool);

#[cfg(test)]
thread_local! {
    /// Canonical emits run: the hostile tests assert work done, not wall time.
    pub(super) static EMITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn item_text(x: &LeetCodeExercise, old: &LeetCodeExercise, key: &str, i: usize) -> Res<Item> {
    #[cfg(test)]
    EMITS.with(|c| c.set(c.get() + 1));
    let file = |v: &Option<Vec<crate::leetcode::FileSpec>>| {
        v.as_ref()
            .and_then(|v| v.get(i))
            .map(file_fence)
            .transpose()
    };
    Ok(match key {
        "examples" => (x.examples.get(i).map(example).transpose()?, false, false),
        "setups" => (
            x.setups.get(i).map(setup).transpose()?,
            true,
            same_lang(
                old.setups.get(i).map(|s| &s.language),
                x.setups.get(i).map(|s| &s.language),
            ),
        ),
        "solutions" => (
            x.solutions.get(i).map(solution).transpose()?,
            true,
            same_lang(
                old.solutions.get(i).map(|s| &s.language),
                x.solutions.get(i).map(|s| &s.language),
            ),
        ),
        "files" => (file(&x.files)?, false, false),
        "solutionFiles" => (file(&x.solution_files)?, false, false),
        _ => (None, false, false),
    })
}

impl Asm<'_> {
    /// One pass over the layout: remember where each list ends, decide every changed block.
    pub(super) fn walk(&mut self) -> Res {
        let blocks = self.blocks;
        for (j, b) in blocks.iter().enumerate() {
            for f in &b.fields {
                self.desc_seen |= matches!(f, LayoutField::Description);
                self.last.insert(Diff::key(*f).0, j);
            }
            if matches!(b.kind, K::FmOpen | K::FmKey | K::FmClose) {
                continue; // whole: `frontmatter()`
            }
            if !b.fields.iter().any(|f| self.diff.changed(*f)) && !self.bound_changed(b) {
                continue;
            }
            match b.kind {
                K::ConfigKey => self.config_key(j, b)?,
                K::Heading | K::ConfigOpen | K::ConfigComment | K::ConfigClose => {}
                _ => self.item(j, b)?,
            }
        }
        Ok(())
    }

    /// A check-bound case fence whose check's cases in that section differ, though the flat
    /// indices it maps to did not (another check grew in front of it).
    fn bound_changed(&self, b: &Block) -> bool {
        let public = match b.fields.first().map(|f| Diff::key(*f).0).as_deref() {
            Some("tests") => true,
            Some("finalTests") => false,
            _ => return false,
        };
        let info = b.raw.lines().next().and_then(|l| l.strip_prefix("```"));
        let Some(n) = info.and_then(|i| attr(i, "check")) else {
            return false;
        };
        match (named(self.x, n), named(self.old, n)) {
            (Some(x), Some(o)) => section(x, public) != section(o, public),
            _ => false,
        }
    }

    /// Decide a changed block that is not frontmatter or a config key.
    pub(super) fn item(&mut self, j: usize, b: &Block) -> Res {
        let Some(&f) = b.fields.first() else {
            return Ok(());
        };
        let (key, idx) = Diff::key(f);
        match (key.as_str(), idx) {
            ("description", _) => self.description_block(j, b),
            ("tests" | "finalTests", _) => self.cases(j, b, &key),
            ("attempts", _) => Ok(()), // the whole section: `attempts()`
            (k, Some(i)) => self.listed(j, k, i),
            _ => Ok(()),
        }
    }

    fn listed(&mut self, j: usize, key: &str, i: usize) -> Res {
        self.check()?;
        if !self.done.insert((key.to_owned(), i)) {
            self.set(j, Act::Drop);
            return Ok(());
        }
        let (text, headed, keep_head) = at(item_text(self.x, self.old, key, i), key)?;
        if key == "solutions" {
            self.unlabel(j);
        }
        let act = match text {
            None => Act::Drop,
            Some(t) if headed && keep_head => Act::Replace(without_first_line(t), false),
            Some(t) => Act::Replace(t, headed),
        };
        self.set(j, act);
        Ok(())
    }

    /// The item's `### label` heading (at most two blocks back, over blank runs) is re-emitted
    /// inside the item text, so the old one goes.
    fn unlabel(&mut self, j: usize) {
        for k in (j.saturating_sub(2)..j).rev() {
            let Some(b) = self.blocks.get(k) else { return };
            if b.kind == K::Heading && b.raw.starts_with("### ") {
                self.set(k, Act::Drop);
                return;
            }
            if !(b.kind == K::Prose && b.fields.is_empty() && js_trim(&b.raw).is_empty()) {
                return;
            }
        }
    }

    /// A case fence maps to a run of case indices (Q-82): re-emitted whole from the surviving
    /// cases, keeping its `check=` binding; dropped when none survive.
    fn cases(&mut self, j: usize, b: &Block, key: &str) -> Res {
        let idx: Vec<usize> = b.fields.iter().filter_map(|f| Diff::key(*f).1).collect();
        let (Some(&a), Some(&z)) = (idx.iter().min(), idx.iter().max()) else {
            return Ok(());
        };
        self.check()?;
        if !self.done.insert((format!("{key}#{a}"), z)) {
            self.set(j, Act::Drop); // the same cases again: one fence is enough
            return Ok(());
        }
        let info = b.raw.lines().next().and_then(|l| l.strip_prefix("```"));
        let check = info.and_then(|i| attr(i, "check"));
        // A check-bound fence holds its check's cases (flat indices shift when another check
        // grows); the check's first fence of the section takes them all, later ones go.
        let bound = check.and_then(|n| named(self.x, n));
        let list = if key == "tests" {
            &self.x.tests
        } else {
            &self.x.final_tests
        };
        let kept = match bound {
            Some(c) => {
                if !self.done.insert((format!("{key}@{}", c.name), 0)) {
                    self.set(j, Act::Drop);
                    return Ok(());
                }
                section(c, key == "tests")
            }
            None => list.get(a..=z).or_else(|| list.get(a..)).unwrap_or(&[]),
        };
        let mut texts = match bound {
            Some(c) => self.lead(key, c)?,
            None => vec![],
        };
        if !kept.is_empty() {
            texts.push(case_fence(kept, check).map_err(|r| r.at(key))?);
        }
        if texts.is_empty() {
            self.set(j, Act::Drop);
        } else {
            self.set(j, Act::Replace(texts.join("\n"), false));
        }
        Ok(())
    }

    /// Fences of the checks before `c` that had no cases in this section of the old file: they
    /// go in front of `c`'s fence, so the file keeps the checks' order (the flat `tests` order).
    fn lead(&mut self, key: &str, c: &CheckBase) -> Res<Vec<String>> {
        let public = key == "tests";
        let mut out = vec![];
        for d in self.x.checks.iter().flatten().map(base_ref) {
            if d.name == c.name {
                break;
            }
            let cs = section(d, public);
            let had = named(self.old, &d.name).is_some_and(|o| !section(o, public).is_empty());
            if had || cs.is_empty() || !self.done.insert((format!("{key}@{}", d.name), 0)) {
                continue;
            }
            out.push(case_fence(cs, Some(&d.name)).map_err(|r| r.at(key))?);
        }
        Ok(out)
    }

    /// The first block carrying `description` gets the new text; later prose of it goes, blank
    /// runs stay.
    fn description_block(&mut self, j: usize, b: &Block) -> Res {
        self.check()?;
        if self.done.insert(("description".to_owned(), 0)) {
            let t = super::super::description(self.x).map_err(|r| r.at("description"))?;
            self.set(j, Act::Replace(format!("\n{t}\n"), false));
        } else if !(b.kind == K::Prose && js_trim(&b.raw).is_empty()) {
            self.set(j, Act::Drop);
        }
        Ok(())
    }

    /// No block carried the description: it goes right after the frontmatter.
    pub(super) fn description(&mut self) -> Res {
        if self.desc_seen || self.x.description.is_empty() || !self.diff.key_changed("description")
        {
            return Ok(());
        }
        let t = super::super::description(self.x).map_err(|r| r.at("description"))?;
        let at = self
            .blocks
            .iter()
            .rposition(|b| matches!(b.kind, K::FmOpen | K::FmKey | K::FmClose))
            .map_or(0, |i| i + 1);
        self.put(at, &format!("\n{t}\n"));
        Ok(())
    }
}
