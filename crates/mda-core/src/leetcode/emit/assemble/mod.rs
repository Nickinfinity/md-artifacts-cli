//! The layout-driven assembler (W7 T7.5a). Every layout block gets an action: kept raw, dropped, or
//! replaced by canonical text from the R1 emitters; new content is inserted at an index. A block is
//! kept iff none of its fields differ between the parse of the layout's text and the request
//! (Q-69/Q-70). Without a layout the file is built from nothing, so every non-default value is "new".
//! The caller runs `write_guard` on the result; nothing here trusts a block's kind or raw text.

mod cfg;
#[cfg(test)]
mod corpus;
mod diff;
mod grow;
mod items;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::NotSerializableReason as Why;
use crate::leetcode::{Block, BlockKind as K, LeetCodeExercise, MAX_LAYOUT_BLOCKS, parse};
use diff::Diff;

type Res<T = ()> = Result<T, Why>;

/// Layout text this big is refused before it is parsed (the op caps lower; this is the backstop).
const MAX_RAW_BYTES: usize = 16 * 1024 * 1024;
/// Output over this multiple of (layout text + request JSON) is refused.
const MAX_GROWTH: usize = 8;

/// What happens to one layout block.
enum Act {
    Keep,
    Drop,
    /// New text; `true` when it opens with its own `## <lang>` heading (so the old heading, if it
    /// now guards nothing, is an orphan).
    Replace(String, bool),
}

struct Asm<'a> {
    blocks: &'a [Block],
    x: &'a LeetCodeExercise,
    old: &'a LeetCodeExercise,
    diff: &'a Diff,
    acts: Vec<Act>,
    /// Text inserted before block `k` (`blocks.len()` = at the end), in the order it was added.
    ins: BTreeMap<usize, String>,
    /// Item runs already emitted: `(list key, index)`, so a run of blocks is replaced once (ruling 2).
    done: HashSet<(String, usize)>,
    /// The last block carrying each list key: where growth appends.
    last: HashMap<String, usize>,
    desc_seen: bool,
    /// No layout at all: the file is built from nothing, frontmatter included.
    fresh: bool,
    /// Bytes emitted so far and the most allowed: checked before each emit, so a hostile layout
    /// stops the work, not just the result.
    used: usize,
    cap: usize,
}

impl Asm<'_> {
    /// Refuse as soon as the emitted text is over budget.
    pub fn check(&self) -> Res {
        if self.used > self.cap {
            return Err(Why::Other);
        }
        Ok(())
    }

    pub fn set(&mut self, j: usize, act: Act) {
        if let Act::Replace(t, _) = &act {
            self.used = self.used.saturating_add(t.len());
        }
        if let Some(a) = self.acts.get_mut(j) {
            *a = act;
        }
    }

    pub fn put(&mut self, at: usize, text: &str) {
        self.used = self.used.saturating_add(text.len());
        self.ins.entry(at).or_default().push_str(text);
    }

    /// The first heading block whose line is `name`.
    pub fn head(&self, name: &str) -> Option<usize> {
        self.blocks
            .iter()
            .position(|b| b.kind == K::Heading && b.raw.trim_end() == name)
    }

    /// Frontmatter is whole (ruling 4): any frontmatter key changed → one canonical block run.
    fn frontmatter(&mut self) -> Res {
        if !(self.fresh || self.diff.fm()) {
            return Ok(());
        }
        let canon = super::frontmatter(self.x)?;
        // Keys the model does not carry (`aliases`, `created`, Dataview fields) are the author's:
        // they stay, raw, after the canonical keys.
        let mut text = canon.strip_suffix("---\n").unwrap_or(&canon).to_owned();
        let custom = |b: &&Block| {
            let key = crate::leetcode::config::top_level_key(b.raw.lines().next().unwrap_or(""));
            b.kind == K::FmKey
                && b.fields.is_empty()
                && !key.is_some_and(|k| crate::leetcode::config::CANONICAL_ORDER.contains(&k))
        };
        for b in self.blocks.iter().filter(custom) {
            if b.raw.lines().any(|l| l == "---") {
                continue; // would close the block early
            }
            text.push_str(&b.raw);
            if !b.raw.ends_with('\n') {
                text.push('\n');
            }
        }
        text.push_str("---\n");
        let blocks = self.blocks;
        let fm = |b: &Block| matches!(b.kind, K::FmOpen | K::FmKey | K::FmClose);
        let at = blocks.iter().position(fm).unwrap_or(0);
        for (j, b) in blocks.iter().enumerate() {
            if fm(b) {
                self.set(j, Act::Drop);
            }
        }
        self.put(at, &text);
        Ok(())
    }

    fn finish(&self) -> Res<String> {
        let mut out = String::new();
        for (j, b) in self.blocks.iter().enumerate() {
            out.push_str(self.ins.get(&j).map_or("", String::as_str));
            match self.acts.get(j) {
                Some(Act::Drop) => {}
                Some(Act::Replace(t, _)) => out.push_str(t),
                _ => out.push_str(&b.raw),
            }
        }
        out.push_str(self.ins.get(&self.blocks.len()).map_or("", String::as_str));
        if out.len() > self.cap {
            return Err(Why::Other);
        }
        Ok(out)
    }
}

pub(super) fn run(x: &LeetCodeExercise, layout: Option<&[Block]>) -> Res<String> {
    let blocks = layout.unwrap_or(&[]);
    let raw_len = blocks
        .iter()
        .fold(0usize, |n, b| n.saturating_add(b.raw.len()));
    if blocks.len() > MAX_LAYOUT_BLOCKS || raw_len > MAX_RAW_BYTES {
        return Err(Why::Other);
    }
    let raw: String = blocks.iter().map(|b| b.raw.as_str()).collect();
    let old = parse(&raw);
    let diff = Diff::new(&old, x)?;
    let mut a = Asm {
        blocks,
        x,
        old: &old,
        diff: &diff,
        acts: blocks.iter().map(|_| Act::Keep).collect(),
        ins: BTreeMap::new(),
        done: HashSet::new(),
        last: HashMap::new(),
        desc_seen: false,
        fresh: layout.is_none(),
        used: 0,
        cap: MAX_GROWTH
            .saturating_mul(raw_len.saturating_add(diff.new_len))
            .saturating_add(4096),
    };
    // Order = canonical order at a shared insertion index: frontmatter, description, config
    // fences, sections, attempts.
    a.frontmatter()?;
    a.walk()?;
    a.description()?;
    a.config()?;
    a.growth()?;
    a.attempts()?;
    a.check()?;
    a.prune_fences();
    a.orphans();
    a.finish()
}
