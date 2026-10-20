//! Flag-delimited and bare-body payloads (spec §7.6, W7c): the layouts `artifact.read` reports,
//! written back so `create(read(f))` keeps the notes, flag names and `***` chrome.

use std::collections::HashSet;

use super::{SerializeError, frontmatter, is_empty_str, lang_for, vars_part};
use crate::model::{ArtifactModel, ModelBlock, ParsedVar, RegionLayout};
use crate::parse::PAYLOAD_LANG;
use crate::parse::flags::{END_FLAG, start_flag, trim_blank_lines};
use crate::parse::text::{js_trim, js_trim_end};

/// The frontmatter `language:` of a one-block flagged or bare payload: none when it is the
/// parser's default, so the re-parse yields the same model.
fn payload_lang(m: &ArtifactModel, b: &ModelBlock) -> String {
    let lang = lang_for(m.artifact_type, &b.language);
    if lang == PAYLOAD_LANG {
        String::new()
    } else {
        lang
    }
}

/// Region payload as the reader yields it: blank lines trimmed at both ends, no trailing space.
pub(super) fn flag_code(code: &str) -> String {
    js_trim_end(&trim_blank_lines(code)).to_owned()
}

/// `[notes]` + one region per block, then the trailing notes, then the file defaults (§7.3).
pub(super) fn flags(m: &ArtifactModel) -> Result<String, SerializeError> {
    let one = [ModelBlock::default()];
    let blocks: &[ModelBlock] = if m.blocks.is_empty() { &one } else { &m.blocks };
    let multi = blocks.len() > 1;
    let lang = match blocks.first() {
        Some(b) if !multi => payload_lang(m, b),
        _ => String::new(), // §2.1: a multi-block file has no frontmatter `language:`
    };
    let mut out = frontmatter(m, &lang);
    let none = RegionLayout::default();
    for (i, b) in blocks.iter().enumerate() {
        let r = m.layout.regions.get(i).unwrap_or(&none);
        out.push('\n');
        push_notes(&mut out, &r.notes, "\n\n");
        let name = if multi { &b.heading } else { &r.name };
        out.push_str(&region(js_trim(name), r.rule, &b.code));
    }
    let trailing = trim_blank_lines(&m.layout.notes);
    if !trailing.is_empty() {
        out.push('\n');
        push_notes(&mut out, &trailing, "\n");
    }
    Ok(out + &vars_part(&file_defaults(blocks))?)
}

fn push_notes(out: &mut String, notes: &str, after: &str) {
    let notes = trim_blank_lines(notes);
    if !notes.is_empty() {
        out.push_str(&notes);
        out.push_str(after);
    }
}

fn region(name: &str, rule: bool, code: &str) -> String {
    let rule = if rule { "***\n" } else { "" };
    let (start, code) = (start_flag(name), flag_code(code));
    format!("{start}\n{rule}{code}\n{rule}{END_FLAG}\n")
}

/// The one defaults fence of a flagged file: every valued var of every block, first value wins.
/// The reader gives every block every default (§7.3), so a model whose blocks disagree is refused
/// by the guard, not written.
fn file_defaults(blocks: &[ModelBlock]) -> Vec<ParsedVar> {
    let mut seen = HashSet::new();
    blocks
        .iter()
        .flat_map(|b| &b.vars)
        .filter(|v| !is_empty_str(v) && seen.insert(v.name.as_str()))
        .cloned()
        .collect()
}

/// The whole body as the payload (§7.5 rule 3). `None` for an empty body, which reads back as the
/// empty fenced shape, so the caller emits that instead.
pub(super) fn bare(m: &ArtifactModel) -> Result<Option<String>, SerializeError> {
    let empty = ModelBlock::default();
    let b = m.blocks.first().unwrap_or(&empty);
    let code = js_trim(&b.code);
    if code.is_empty() {
        return Ok(None);
    }
    let head = frontmatter(m, &payload_lang(m, b));
    Ok(Some(format!("{head}\n{code}\n{}", vars_part(&b.vars)?)))
}
