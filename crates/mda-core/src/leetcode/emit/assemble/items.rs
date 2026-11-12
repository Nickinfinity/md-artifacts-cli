//! Per-block decisions for changed fields: list items (one emit per index, ruling 2), case fences
//! (Q-82), the description, and the shrink rule (a tail item's blocks go).

use super::{Act, Asm, Diff, Res};
use crate::leetcode::emit::{case_fence, example, file_fence, setup, solution};
use crate::leetcode::project::attr;
use crate::leetcode::{Block, BlockKind as K, LayoutField, LeetCodeExercise};
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
            if !b.fields.iter().any(|f| self.diff.changed(*f)) {
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
        let (text, headed, keep_head) = item_text(self.x, self.old, key, i)?;
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
        let list = if key == "tests" {
            &self.x.tests
        } else {
            &self.x.final_tests
        };
        let kept = list.get(a..=z).or_else(|| list.get(a..)).unwrap_or(&[]);
        if kept.is_empty() {
            self.set(j, Act::Drop);
            return Ok(());
        }
        let info = b.raw.lines().next().and_then(|l| l.strip_prefix("```"));
        let check = info.and_then(|i| attr(i, "check"));
        let text = case_fence(kept, check)?;
        self.set(j, Act::Replace(text, false));
        Ok(())
    }

    /// The first block carrying `description` gets the new text; later prose of it goes, blank
    /// runs stay.
    fn description_block(&mut self, j: usize, b: &Block) -> Res {
        self.check()?;
        if self.done.insert(("description".to_owned(), 0)) {
            let t = super::super::description(self.x)?;
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
        let t = super::super::description(self.x)?;
        let at = self
            .blocks
            .iter()
            .rposition(|b| matches!(b.kind, K::FmOpen | K::FmKey | K::FmClose))
            .map_or(0, |i| i + 1);
        self.put(at, &format!("\n{t}\n"));
        Ok(())
    }
}
