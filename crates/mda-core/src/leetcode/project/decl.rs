//! `libs:` declarations (TS `parseLibDeclarations`, `readListMap`, `inlineList`,
//! `splitOutsideQuotes`): each language's specs validated against **its own** registry grammar
//! before anything can reach an install subprocess.

use super::super::{
    LibSpec,
    config::{block_lines, unquote_pair},
    libs::{LibEcosystem, ecosystem_for, parse_spec},
    program::warning,
    resolve_lang_id,
    shared::truncate_param,
};
use crate::error::{LEETCODE_LIBS_NO_REGISTRY, LEETCODE_LIBS_REJECTED};
use crate::parse::text::js_trim;
use crate::render::Warning;

/// `^([^:]+):(.*)$` over a trimmed line: `(key, rest)`. `.` and `$` refuse a line terminator.
pub(in crate::leetcode) fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once(':')?;
    (!key.is_empty() && !rest.contains(['\n', '\r', '\u{2028}', '\u{2029}'])).then_some((key, rest))
}

fn ecosystem_name(e: LibEcosystem) -> &'static str {
    match e {
        LibEcosystem::Pnpm => "pnpm",
        LibEcosystem::Pip => "pip",
        LibEcosystem::Cargo => "cargo",
        LibEcosystem::Maven => "maven",
    }
}

/// Keys that would reach through an object's prototype are never used as targets.
fn is_safe_key(key: &str) -> bool {
    !matches!(key, "__proto__" | "constructor" | "prototype")
}

/// Parse `libs:` into per-language lists; `{}` when absent. The language key runs through
/// `resolve_lang_id`; an unsafe one is skipped silently, one without a registry warns (W12), refused
/// specs warn once per language (W13).
pub(crate) fn parse_lib_declarations(lines: &[&str], warnings: &mut Vec<Warning>) -> LibSpec {
    let Some(start) = lines.iter().position(|l| js_trim(l) == "libs:") else {
        return LibSpec::default();
    };
    let mut pairs: Vec<(String, Vec<String>)> = vec![];
    for (key, values) in read_list_map(lines, start) {
        let language = resolve_lang_id(&key);
        if !is_safe_key(&language) {
            continue;
        }
        let Some(eco) = ecosystem_for(&language) else {
            warnings.push(warning(
                LEETCODE_LIBS_NO_REGISTRY,
                &[("language", &language)],
            ));
            continue;
        };
        let (accepted, refused): (Vec<String>, Vec<String>) =
            values.into_iter().partition(|v| parse_spec(eco, v).is_ok());
        if !refused.is_empty() {
            let specs = refused
                .iter()
                .map(|s| truncate_param(s))
                .collect::<Vec<_>>()
                .join(", ");
            warnings.push(warning(
                LEETCODE_LIBS_REJECTED,
                &[("specs", &specs), ("ecosystem", ecosystem_name(eco))],
            ));
        }
        if !accepted.is_empty() {
            pairs.push((language, accepted));
        }
    }
    LibSpec::from_pairs(pairs)
}

/// A `key:` to list block: each nested `key:` maps to its `- item` lines or an inline `[a, b]`.
fn read_list_map(lines: &[&str], start: usize) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = vec![];
    for line in block_lines(lines, start) {
        let t = js_trim(line);
        if let Some(item) = t.strip_prefix("- ") {
            if let Some(cur) = out.last_mut() {
                cur.1.push(unquote_pair(item).to_owned());
            }
            continue;
        }
        if let Some((key, rest)) = split_key_value(t) {
            out.push((js_trim(key).to_owned(), inline_list(rest)));
        }
    }
    out
}

/// `[a, b]` to `["a", "b"]`; anything else (a list-header line) to `[]`.
fn inline_list(raw: &str) -> Vec<String> {
    let Some(inner) = js_trim(raw).strip_prefix('[') else {
        return vec![];
    };
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    split_outside_quotes(inner)
        .into_iter()
        .map(|p| unquote_pair(p).to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Split on commas outside a quoted run (`numpy>=2,<3` inside quotes stays whole).
fn split_outside_quotes(body: &str) -> Vec<&str> {
    let (mut parts, mut from, mut quote) = (vec![], 0, None::<char>);
    for (i, c) in body.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ',' => {
                parts.push(body.get(from..i).unwrap_or(""));
                from = i + 1;
            }
            None => {}
        }
    }
    parts.push(body.get(from..).unwrap_or(""));
    parts
}
