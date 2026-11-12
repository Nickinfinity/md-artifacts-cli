//! The layout producer (W7 T7.1): a decoded LeetCode file → ordered [`Block`]s whose `raw` texts
//! concatenate to the input exactly, each tagged with the model fields it feeds. One line scan with
//! offset accumulation over the existing scanners (frontmatter end, config spans, key grammar); no
//! second grammar. The field mapping is a hint (the whole-file guard arbitrates); the byte
//! coverage is not: every byte lands in exactly one block, parsed or not.

mod keys;
#[cfg(test)]
mod tests;

use keys::{has_path, is_lang, key_fields};

use super::config::{
    ConfigSpan, extract_config_blocks, split_frontmatter, starts_with_ws, top_level_key,
};
use super::sections::{CASE_SECTIONS, extract_attempts, extract_case_fences};
use super::{Block, BlockKind as K, LayoutField as F, MAX_LAYOUT_BLOCKS};
use crate::parse::frontmatter_end;
use crate::parse::text::{js_trim, js_trim_end};

/// Why no layout was built. The block cap is checked before building (`MAX_LAYOUT_BLOCKS`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LayoutRefusal;
/// assert_eq!(LayoutRefusal::TooManyBlocks, LayoutRefusal::TooManyBlocks);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutRefusal {
    TooManyBlocks,
}

type Res<T = ()> = Result<T, LayoutRefusal>;

/// The layout of `src` (decoded text, as every `leetcode::*` entry point). Refuses, before
/// building the block that would exceed it, a file of more than [`MAX_LAYOUT_BLOCKS`] blocks.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{layout, BlockKind, LayoutField};
/// let blocks = layout("---\ntitle: T\n---\n\nProse.\n").unwrap();
/// assert_eq!(blocks[1].kind, BlockKind::FmKey);
/// assert_eq!(blocks[1].fields, [LayoutField::Title]);
/// assert_eq!(blocks.iter().map(|b| b.raw.as_str()).collect::<String>(), "---\ntitle: T\n---\n\nProse.\n");
/// ```
pub fn layout(src: &str) -> Result<Vec<Block>, LayoutRefusal> {
    let mut scan = Scan::new(src);
    let end = frontmatter_end(src);
    if end > 0 {
        // A block over the 64 KiB cap is skipped by the parser: covered, maps nothing.
        scan.frontmatter(end, !split_frontmatter(src).0.is_empty())?;
    }
    scan.body(end)?;
    Ok(scan.out)
}

/// The body section the scan is in; a heading changes it. Only the first `## Tests` etc. counts,
/// as in the parser.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Lead,
    Examples,
    Tests,
    FinalTests,
    Files,
    Setup,
    Solutions,
    Attempts,
    Other,
}

/// A key group being collected: a top-level key (or a run of comment/blank lines) plus its tail.
struct Group {
    start: usize,
    kind: K,
    fields: Vec<F>,
    is_key: bool,
}

struct Scan<'a> {
    src: &'a str,
    out: Vec<Block>,
    prose: Option<usize>,
    sec: Section,
    seen: Vec<Section>,
    /// Inside a valid `## <Language>` chunk of `# Setup` / `# Solutions` / `# Attempts`.
    lang_ok: bool,
    /// The chunk's one fence (setup, `###` solution) is already taken.
    taken: bool,
    sub: Sub,
    /// `(out index, start offset)` of the comment block awaiting its fence.
    pending: Option<(usize, usize)>,
    n: Counts,
}

/// Where in a `# Solutions` language chunk: before any `###` (direct fences), in one, or in a
/// `###` the parser drops (empty label).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    Direct,
    Labelled,
    Dropped,
}

/// Next item index per list field.
#[derive(Default)]
struct Counts {
    examples: usize,
    tests: usize,
    finals: usize,
    setups: usize,
    solutions: usize,
    attempts: usize,
    files: usize,
    solution_files: usize,
}

/// The line starting at `pos`, newline included; `""` at the end.
fn line_at(src: &str, pos: usize) -> &str {
    src.get(pos..)
        .and_then(|r| r.split_inclusive('\n').next())
        .unwrap_or("")
}

/// A line without its terminator (`\n`, then a `\r` before it).
fn text(line: &str) -> &str {
    let l = line.strip_suffix('\n').unwrap_or(line);
    l.strip_suffix('\r').unwrap_or(l)
}

fn bump(n: &mut usize) -> usize {
    *n += 1;
    *n - 1
}

/// How many cases a case fence holds, by the parser's own reader.
fn case_count(fence: &str) -> usize {
    let wrapped = format!("## Tests\n{fence}");
    extract_case_fences(&wrapped, CASE_SECTIONS.tests)
        .first()
        .map_or(0, |f| f.cases.len())
}

impl<'a> Scan<'a> {
    fn new(src: &'a str) -> Self {
        Scan {
            src,
            out: vec![],
            prose: None,
            sec: Section::Lead,
            seen: vec![],
            lang_ok: false,
            taken: false,
            sub: Sub::Direct,
            pending: None,
            n: Counts::default(),
        }
    }

    /// The only place a block is built, so the cap holds before the 20 001st exists.
    fn push(&mut self, start: usize, end: usize, kind: K, fields: Vec<F>) -> Res {
        if start >= end {
            return Ok(());
        }
        if self.out.len() >= MAX_LAYOUT_BLOCKS {
            return Err(LayoutRefusal::TooManyBlocks);
        }
        let raw = self.src.get(start..end).unwrap_or("").to_owned();
        self.out.push(Block { raw, kind, fields });
        Ok(())
    }

    fn flush_prose(&mut self, upto: usize) -> Res {
        let Some(start) = self.prose.take() else {
            return Ok(());
        };
        let fields = if self.sec == Section::Lead {
            vec![F::Description]
        } else {
            vec![]
        };
        self.push(start, upto, K::Prose, fields)
    }

    /// `---` · key groups · `---` over `[0, end)`.
    fn frontmatter(&mut self, end: usize, mapped: bool) -> Res {
        let first = line_at(self.src, 0).len().min(end);
        self.push(0, first, K::FmOpen, vec![])?;
        let head = self.src.get(..end).unwrap_or("");
        let head = head.strip_suffix('\n').unwrap_or(head);
        let close = head.rfind('\n').map_or(0, |i| i + 1).max(first);
        self.key_groups(first, close, (K::FmKey, K::FmKey), mapped)?;
        self.push(close, end, K::FmClose, vec![])
    }

    /// Split `[from, to)` into key groups (a key line plus its indented/blank tail) and runs of
    /// everything else (comments, stray lines), one block each.
    fn key_groups(&mut self, from: usize, to: usize, kinds: (K, K), mapped: bool) -> Res {
        let (mut pos, mut cur): (usize, Option<Group>) = (from, None);
        while pos < to {
            let line = line_at(self.src, pos);
            if line.is_empty() {
                break;
            }
            let t = text(line);
            let extends = cur.as_ref().is_some_and(|g| {
                top_level_key(t).is_none()
                    && (!g.is_key || starts_with_ws(t) || js_trim(t).is_empty())
            });
            if !extends {
                if let Some(g) = cur.take() {
                    self.push(g.start, pos, g.kind, g.fields)?;
                }
                let is_key = top_level_key(t).is_some();
                let fields = if is_key && mapped {
                    key_fields(t, kinds.0 == K::ConfigKey)
                } else {
                    vec![]
                };
                let kind = if is_key { kinds.0 } else { kinds.1 };
                cur = Some(Group {
                    start: pos,
                    kind,
                    fields,
                    is_key,
                });
            }
            pos += line.len();
        }
        match cur {
            Some(g) => self.push(g.start, pos.min(to), g.kind, g.fields),
            None => Ok(()),
        }
    }

    /// One config fence: opener · key groups · closer; the closer is the line holding `span_end`.
    fn config_fence(&mut self, pos: usize, span_end: usize) -> Res<usize> {
        let open = pos + line_at(self.src, pos).len();
        self.push(pos, open, K::ConfigOpen, vec![])?;
        let mut close = open;
        loop {
            let l = line_at(self.src, close);
            if l.is_empty() || close + l.len() >= span_end {
                break;
            }
            close += l.len();
        }
        self.key_groups(open, close, (K::ConfigKey, K::ConfigComment), true)?;
        let end = close + line_at(self.src, close).len();
        self.push(close, end, K::ConfigClose, vec![])?;
        Ok(end)
    }

    fn body(&mut self, base: usize) -> Res {
        let body = self.src.get(base..).unwrap_or("");
        let mut spans = extract_config_blocks(body).spans.into_iter().peekable();
        let mut pos = base;
        loop {
            let line = line_at(self.src, pos);
            if line.is_empty() {
                break;
            }
            let t = text(line);
            while spans.next_if(|s| base + s.start < pos).is_some() {}
            if let Some(ConfigSpan { end, .. }) = spans.next_if(|s| base + s.start == pos) {
                self.flush_prose(pos)?;
                pos = self.config_fence(pos, base + end)?;
            } else if t.starts_with("```") {
                self.flush_prose(pos)?;
                pos = self.fence(pos, t)?;
            } else if t.starts_with('#') && t.trim_start_matches('#').starts_with(' ') {
                self.flush_prose(pos)?;
                self.heading(line);
                self.push(pos, pos + line.len(), K::Heading, vec![])?;
                pos += line.len();
            } else if let Some(next) = self.comment(pos, t)? {
                pos = next;
            } else {
                self.prose.get_or_insert(pos);
                pos += line.len();
            }
        }
        self.flush_prose(pos)
    }

    /// An HTML comment line (to its `-->`) in `# Solutions` / `# Attempts` feeds the item the next
    /// fence will be (`meta` / `attempt` payload).
    fn comment(&mut self, pos: usize, t: &str) -> Res<Option<usize>> {
        if !matches!(self.sec, Section::Solutions | Section::Attempts)
            || !t.trim_start().starts_with("<!--")
        {
            return Ok(None);
        }
        self.flush_prose(pos)?;
        let mut p = pos;
        loop {
            let l = line_at(self.src, p);
            p += l.len();
            if l.is_empty() || l.contains("-->") {
                break;
            }
        }
        // Fields are set when the fence that follows takes an index (it may not).
        self.pending = Some((self.out.len(), pos));
        self.push(pos, p, K::Comment, vec![])?;
        Ok(Some(p))
    }

    /// First occurrence of a section heading only (the parser reads the first match).
    fn first(&mut self, s: Section) -> bool {
        let fresh = !self.seen.contains(&s);
        if fresh {
            self.seen.push(s);
        }
        fresh
    }

    fn heading(&mut self, line: &str) {
        let t = text(line);
        let hashes = t.bytes().take_while(|&b| b == b'#').count();
        let named = match js_trim_end(t) {
            "# Setup" => Some(Section::Setup),
            "# Solutions" => Some(Section::Solutions),
            "# Attempts" => Some(Section::Attempts),
            "## Examples" => Some(Section::Examples),
            "## Tests" => Some(Section::Tests),
            "## Final Tests" => Some(Section::FinalTests),
            "## Files" => Some(Section::Files),
            _ => None,
        };
        let langs = matches!(
            self.sec,
            Section::Setup | Section::Solutions | Section::Attempts
        );
        self.sec = match named {
            Some(s) if self.first(s) => s,
            Some(_) => Section::Other,
            None if hashes == 2 && langs => self.sec,
            None if hashes >= 3 && self.sec != Section::Lead => self.sec,
            None => Section::Other,
        };
        if named.is_some() || (hashes == 1 && !langs) || self.sec == Section::Other {
            (self.lang_ok, self.taken, self.sub) = (false, false, Sub::Direct);
        } else if hashes == 2 && langs {
            // A `## <Language>` chunk counts only with a non-empty, newline-ended label.
            (self.lang_ok, self.taken, self.sub) = (label_ok(line, "## "), false, Sub::Direct);
        } else if hashes == 3 && self.sec == Section::Solutions {
            self.sub = if label_ok(line, "### ") {
                Sub::Labelled
            } else {
                Sub::Dropped
            };
            self.taken = false;
        }
    }

    /// A plain fence from its opener to the next line opening with three backticks (or the end).
    fn fence(&mut self, pos: usize, opener: &str) -> Res<usize> {
        let mut p = pos + line_at(self.src, pos).len();
        loop {
            let l = line_at(self.src, p);
            p += l.len();
            if l.is_empty() || text(l).starts_with("```") {
                break;
            }
        }
        let raw = self.src.get(pos..p).unwrap_or("");
        let (kind, fields) = self.classify(opener.get(3..).unwrap_or(""), raw, p);
        if let Some((i, _)) = self.pending.take()
            && let Some(c) = self.out.get_mut(i)
        {
            c.fields.clone_from(&fields); // the comment feeds the item its fence became
        }
        self.push(pos, p, kind, fields)?;
        Ok(p)
    }

    fn classify(&mut self, info: &str, raw: &str, end: usize) -> (K, Vec<F>) {
        let attempt = self.sec == Section::Attempts && self.attempt_at(end);
        let n = &mut self.n;
        let (path, lang) = (has_path(info), is_lang(info));
        let has = |k: &str| raw.lines().any(|l| l.starts_with(k));
        match self.sec {
            Section::Examples if info == "example" => {
                let ok = has("input:") && has("output:");
                (
                    K::CaseFence,
                    if ok {
                        vec![F::Examples(bump(&mut n.examples))]
                    } else {
                        vec![]
                    },
                )
            }
            Section::Tests | Section::FinalTests
                if info.starts_with("json") || info.starts_with("yaml") =>
            {
                let tests = self.sec == Section::Tests;
                let ctr = if tests { &mut n.tests } else { &mut n.finals };
                let from = *ctr;
                *ctr += case_count(raw);
                let mk = |i| if tests { F::Tests(i) } else { F::FinalTests(i) };
                (K::CaseFence, (from..*ctr).map(mk).collect())
            }
            Section::Setup if lang && self.lang_ok && !self.taken => {
                self.taken = true;
                (K::CodeFence, vec![F::Setups(bump(&mut n.setups))])
            }
            Section::Solutions if path => (
                K::FileFence,
                vec![F::SolutionFiles(bump(&mut n.solution_files))],
            ),
            Section::Solutions if lang && self.lang_ok => match (self.sub, self.taken) {
                (Sub::Direct, _) | (Sub::Labelled, false) => {
                    self.taken = true;
                    (K::CodeFence, vec![F::Solutions(bump(&mut n.solutions))])
                }
                _ => (K::CodeFence, vec![]),
            },
            Section::Attempts if lang && self.lang_ok && attempt => {
                (K::CodeFence, vec![F::Attempts(bump(&mut self.n.attempts))])
            }
            Section::Files if path => (K::FileFence, vec![F::Files(bump(&mut n.files))]),
            Section::Lead => (K::CodeFence, vec![F::Description]),
            _ => (K::CodeFence, vec![]),
        }
    }

    /// Does the pending comment plus the fence ending at `end` read as an attempt, by the parser?
    fn attempt_at(&self, end: usize) -> bool {
        let Some((_, start)) = self.pending else {
            return false;
        };
        let wrapped = format!(
            "# Attempts\n## x\n{}",
            self.src.get(start..end).unwrap_or("")
        );
        extract_attempts(&wrapped).len() == 1
    }
}

/// `heading_of`: the text after `prefix` is non-empty, free of `\r`/U+2028/U+2029, newline-ended.
fn label_ok(line: &str, prefix: &str) -> bool {
    let Some((l, _)) = line.strip_prefix(prefix).and_then(|r| r.split_once('\n')) else {
        return false;
    };
    let l = l.strip_suffix('\r').unwrap_or(l);
    !l.is_empty() && !l.contains(['\r', '\u{2028}', '\u{2029}'])
}
