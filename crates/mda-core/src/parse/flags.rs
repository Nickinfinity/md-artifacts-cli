//! Flagged plain-markdown payload regions (spec §7; `flags.service.ts:36-280`).
//! The only module that spells the marker; `tests/flags.rs` guards that.

use std::sync::LazyLock;

use regex::Regex;

use super::fence::is_vars_label;
use super::text::{compile, js_lines, js_trim};
use crate::model::RegionLayout;

// Line-local, so `\A…\z` (JS `^…$` without `m`). `[A-Za-z0-9_]`-style classes are not needed here.
static START_FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\A[ \t]*%%[ \t]*oa:start(?:[ \t]([^%\n]*))?%%[ \t]*\z"));
static END_FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\A[ \t]*%%[ \t]*oa:end[ \t]*%%[ \t]*\z"));
static FENCE_RE: LazyLock<Regex> = LazyLock::new(|| compile(r"\A {0,3}(`{3,}|~{3,})"));

/// One flagged region: its name (`""` when unnamed) and its trimmed content.
///
/// # Examples
///
/// ```
/// use mda_core::parse::flags::FlaggedRegion;
/// let r = FlaggedRegion { name: String::new(), content: "x".into() };
/// assert_eq!(r.content, "x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlaggedRegion {
    pub name: String,
    pub content: String,
}

/// `***` visual rule: 3+ asterisks and nothing else but surrounding whitespace.
fn is_visual_rule(line: &str) -> bool {
    let t = js_trim(line);
    t.len() >= 3 && t.bytes().all(|b| b == b'*')
}

/// The fence run a line opens, if any.
fn fence_opener(line: &str) -> Option<&str> {
    FENCE_RE.captures(line)?.get(1).map(|m| m.as_str())
}

/// Same char, run at least as long; info after the run is tolerated.
fn closes_fence(marker: &str, line: &str) -> bool {
    let run = fence_opener(line).unwrap_or("");
    run.chars().next() == marker.chars().next() && !run.is_empty() && run.len() >= marker.len()
}

/// Blank lines trimmed at both ends only (indentation of the first line survives).
fn region_content(lines: &[&str]) -> String {
    let blank = |l: &&str| js_trim(l).is_empty();
    let start = lines.iter().position(|l| !blank(l)).unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !blank(l))
        .map_or(start, |i| i + 1);
    lines.get(start..end).unwrap_or(&[]).join("\n")
}

/// The start flag line the emitter writes (W7c): `%%oa:start%%`, or with ` <name>` when named.
///
/// # Examples
///
/// ```
/// use mda_core::parse::flags::start_flag;
/// assert_eq!(start_flag(""), "%%oa:start%%");
/// assert_eq!(start_flag("Dev"), "%%oa:start Dev%%");
/// ```
pub fn start_flag(name: &str) -> String {
    if name.is_empty() {
        "%%oa:start%%".to_owned()
    } else {
        format!("%%oa:start {name}%%")
    }
}

/// The end flag line the emitter writes (W7c).
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::parse::flags::END_FLAG, "%%oa:end%%");
/// ```
pub const END_FLAG: &str = "%%oa:end%%";

/// `s` with whole blank lines trimmed at both ends, lines joined by `\n`: the rule region content
/// and notes are read with, so the emitter writes exactly what a re-parse yields.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::parse::flags::trim_blank_lines("\n \n  a\r\nb \n\n"), "  a\nb ");
/// ```
pub fn trim_blank_lines(s: &str) -> String {
    region_content(&js_lines(s).collect::<Vec<_>>())
}

/// Every flagged region of `body` (frontmatter already stripped), in document order.
///
/// Fences are tracked inside and outside regions, so a documented flag inside a sample fence is
/// payload. An unterminated region runs to EOF; a second start inside an open region is content.
///
/// # Examples
///
/// ```
/// use mda_core::parse::flags::extract_flagged_regions;
/// let r = extract_flagged_regions("%%oa:start Dev%%\nrun it\n%%oa:end%%");
/// assert_eq!((r[0].name.as_str(), r[0].content.as_str()), ("Dev", "run it"));
/// ```
pub fn extract_flagged_regions(body: &str) -> Vec<FlaggedRegion> {
    scan_flags(body).regions
}

/// What [`scan_flags`] finds: the regions and their layout (W7c, spec §7.6).
///
/// # Examples
///
/// ```
/// let s = mda_core::parse::flags::scan_flags("");
/// assert!(s.regions.is_empty() && s.layouts.is_empty() && s.notes.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlagScan {
    pub regions: Vec<FlaggedRegion>,
    /// One per region: name, notes before it, `***` wrapping.
    pub layouts: Vec<RegionLayout>,
    /// Text after the last region, minus the first `vks` fence outside the regions and the
    /// `vars:` label just above it (the defaults section, which the emitter rebuilds).
    pub notes: String,
}

/// An open region: its name, payload lines, and whether its first / latest non-blank line is chrome.
struct Open<'a> {
    name: String,
    lines: Vec<&'a str>,
    first_rule: Option<bool>,
    last_rule: bool,
}

impl<'a> Open<'a> {
    fn saw(&mut self, line: &str, chrome: bool) {
        if !js_trim(line).is_empty() {
            self.first_rule.get_or_insert(chrome);
            self.last_rule = chrome;
        }
    }
}

/// The scan behind [`extract_flagged_regions`], keeping what lies between the regions.
///
/// # Examples
///
/// ```
/// use mda_core::parse::flags::scan_flags;
/// let s = scan_flags("note\n%%oa:start%%\n***\nx\n***\n%%oa:end%%\nafter\nvars:\n```vks\nVK-a: 1\n```\n");
/// assert_eq!(s.regions[0].content, "x");
/// assert_eq!((s.layouts[0].notes.as_str(), s.layouts[0].rule), ("note", true));
/// assert_eq!(s.notes, "after");
/// ```
pub fn scan_flags(body: &str) -> FlagScan {
    let mut out = FlagScan::default();
    let mut fence: Option<&str> = None;
    let mut open: Option<Open> = None;
    let mut gap: Vec<&str> = Vec::new();
    // `Some(true)` while skipping the defaults fence; `Some(false)` once it is skipped.
    let mut defaults: Option<bool> = None;
    for line in js_lines(body) {
        if let Some(m) = fence {
            if closes_fence(m, line) {
                fence = None;
                if defaults == Some(true) {
                    defaults = Some(false);
                    continue;
                }
            }
        } else if let Some(m) = fence_opener(line) {
            fence = Some(m);
            if open.is_none() && defaults.is_none() && js_trim(line) == "```vks" {
                defaults = Some(true);
                drop_vars_label(&mut gap);
            }
        } else {
            let start = START_FLAG_RE.captures(line);
            if let (Some(c), None) = (&start, &open) {
                let name = c.get(1).map_or("", |m| js_trim(m.as_str()));
                out.layouts.push(RegionLayout {
                    name: name.to_owned(),
                    notes: region_content(&gap),
                    rule: false,
                });
                gap.clear();
                open = Some(Open {
                    name: name.to_owned(),
                    lines: Vec::new(),
                    first_rule: None,
                    last_rule: false,
                });
                continue;
            }
            if open.is_some() && END_FLAG_RE.is_match(line) {
                close(&mut open, &mut out);
                continue;
            }
            // `***` is chrome only inside a region; fenced lines never reach here.
            if let Some(o) = open.as_mut().filter(|_| is_visual_rule(line)) {
                o.saw(line, true);
                continue;
            }
        }
        match open.as_mut() {
            Some(o) => {
                o.saw(line, false);
                o.lines.push(line);
            }
            None if defaults != Some(true) => gap.push(line),
            None => {}
        }
    }
    close(&mut open, &mut out);
    out.notes = region_content(&gap);
    out
}

fn close(open: &mut Option<Open>, out: &mut FlagScan) {
    if let Some(o) = open.take() {
        if let Some(l) = out.layouts.last_mut() {
            // ponytail: only the wrapping `***` pair is recorded; a rule line mid-region is chrome
            // that a rewrite drops (spec §7.6 ceilings) — record line positions if anyone uses them.
            l.rule = o.first_rule == Some(true) && o.last_rule;
        }
        out.regions.push(FlaggedRegion {
            name: o.name,
            content: region_content(&o.lines),
        });
    }
}

/// Drop the `vars:` label (and the blank lines after it) that introduces the defaults fence.
fn drop_vars_label(gap: &mut Vec<&str>) {
    let keep = gap.len();
    let mut end = keep;
    while end > 0 && gap.get(end - 1).is_some_and(|l| js_trim(l).is_empty()) {
        end -= 1;
    }
    if end > 0 && gap.get(end - 1).is_some_and(|l| is_vars_label(l)) {
        gap.truncate(end - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statics_compile() {
        LazyLock::force(&START_FLAG_RE);
        LazyLock::force(&END_FLAG_RE);
        LazyLock::force(&FENCE_RE);
    }
}
