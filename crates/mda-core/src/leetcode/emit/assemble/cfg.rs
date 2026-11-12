//! Config key groups: a changed `ConfigKey` block re-emits canonically (or goes, when the key is
//! back to its default); a newly non-default key joins its group's fence or opens one (Q-81); a
//! fence left with no keys is dropped.

use std::collections::HashSet;

use super::diff::FM_KEYS;
use super::{Act, Asm, Res};
use crate::error::NotSerializableReason as Why;
use crate::leetcode::config::top_level_key;
use crate::leetcode::emit::{
    config_fence, libs_group, packages_group, practice_group, program_group, signature_groups,
    test_group,
};
use crate::leetcode::{Block, BlockKind as K, LeetCodeExercise};

/// Config keys the emitters write, in spec §3.9 order, with their fence group and the model keys
/// whose change means "re-emit".
const KEYS: [(&str, usize, &[&str]); 9] = [
    ("function", 0, &["functionName"]),
    ("functions", 0, &["functions"]),
    ("params", 0, &["params"]),
    ("returns", 0, &["returns"]),
    ("test", 1, &["test", "checks"]),
    ("practice", 1, &["practice"]),
    ("libs", 2, &["libs"]),
    ("packages", 2, &["packages"]),
    ("program", 2, &["program"]),
];

/// The canonical group of `key`; `None` = the default value (or a key the emitters do not own).
fn canon(x: &LeetCodeExercise, key: &str) -> Result<Option<String>, Why> {
    Ok(match key {
        "function" | "functions" | "params" | "returns" => signature_groups(x)?
            .into_iter()
            .find(|g| g.starts_with(&format!("{key}:"))),
        "test" => test_group(x)?,
        "practice" => practice_group(x)?,
        "libs" => libs_group(x)?,
        "packages" => packages_group(x)?,
        "program" => program_group(x)?,
        _ => None,
    })
}

fn key_of(b: &Block) -> Option<&str> {
    top_level_key(b.raw.lines().next().unwrap_or(""))
}

/// One config fence: its opener and closer blocks.
struct Fence {
    open: usize,
    close: usize,
}

impl Asm<'_> {
    pub(super) fn config_key(&mut self, j: usize, b: &Block) -> Res {
        self.check()?;
        let Some(key) = key_of(b) else { return Ok(()) };
        // A frontmatter key written in a fence: the canonical frontmatter carries it now. A key
        // seen before is a duplicate: gone, and never emitted twice.
        if FM_KEYS.contains(&key) || !self.done.insert((format!("cfg:{key}"), 0)) {
            self.set(j, Act::Drop);
            return Ok(());
        }
        match canon(self.x, key)? {
            Some(t) => self.set(j, Act::Replace(t, false)),
            None => self.set(j, Act::Drop),
        }
        Ok(())
    }

    fn fences(&self) -> Vec<Fence> {
        let (mut out, mut open) = (vec![], None);
        for (j, b) in self.blocks.iter().enumerate() {
            match b.kind {
                K::ConfigOpen => open = Some(j),
                K::ConfigClose => {
                    if let Some(o) = open.take() {
                        out.push(Fence { open: o, close: j });
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Canonical groups of the keys that are non-default now but have no live block, by fence group.
    fn missing(&self) -> Res<[Vec<String>; 3]> {
        let live: HashSet<&str> = self
            .blocks
            .iter()
            .zip(&self.acts)
            .filter(|(b, a)| b.kind == K::ConfigKey && !matches!(a, Act::Drop))
            .filter_map(|(b, _)| key_of(b))
            .collect();
        let mut add: [Vec<String>; 3] = Default::default();
        for (key, g, model) in KEYS {
            if live.contains(key) || !model.iter().any(|m| self.diff.key_changed(m)) {
                continue;
            }
            if let (Some(t), Some(slot)) = (canon(self.x, key)?, add.get_mut(g)) {
                slot.push(t);
            }
        }
        Ok(add)
    }

    /// The fence already holding a live key of group `g`.
    fn home(&self, fences: &[Fence], g: usize) -> Option<usize> {
        let group_of = |k: &str| KEYS.iter().find(|e| e.0 == k).map(|e| e.1);
        fences
            .iter()
            .find(|f| {
                (f.open..f.close).any(|k| {
                    self.blocks
                        .get(k)
                        .zip(self.acts.get(k))
                        .is_some_and(|(b, a)| {
                            b.kind == K::ConfigKey
                                && !matches!(a, Act::Drop)
                                && key_of(b).and_then(group_of) == Some(g)
                        })
                })
            })
            .map(|f| f.close)
    }

    /// Missing keys go into their group's fence (at its end) or into a new canonical fence per
    /// group, after the last fence, else before the first heading (Q-81).
    pub(super) fn config(&mut self) -> Res {
        let add = self.missing()?;
        let fences = self.fences();
        let mut fresh = vec![];
        for (g, texts) in add.iter().enumerate().filter(|(_, t)| !t.is_empty()) {
            match self.home(&fences, g) {
                Some(close) => self.put(close, &texts.concat()),
                None => fresh.push(config_fence(texts)),
            }
        }
        if fresh.is_empty() {
            return Ok(());
        }
        match fences.last() {
            Some(f) => self.put(f.close + 1, &format!("\n{}", fresh.join("\n"))),
            None => {
                let heading = self.blocks.iter().position(|b| b.kind == K::Heading);
                let at = heading.unwrap_or(self.blocks.len());
                self.put(at, &format!("{}\n", fresh.join("\n")));
            }
        }
        Ok(())
    }

    /// A fence that lost a key and holds none now (comments aside) goes whole (Q-81).
    pub(super) fn prune_fences(&mut self) {
        for f in self.fences() {
            let inner = f.open + 1..f.close;
            let acts = || inner.clone().filter_map(|k| self.acts.get(k));
            let lost = acts().any(|a| matches!(a, Act::Drop));
            let live = acts().any(|a| !matches!(a, Act::Drop));
            if lost && !live && !self.ins.contains_key(&f.close) {
                self.set(f.open, Act::Drop);
                self.set(f.close, Act::Drop);
            }
        }
    }
}
