//! `program:` config block (TS `program-config.helpers.ts`, W4 T4.4): the delivery channel of a
//! `program` test. `entry` is a shape guard, never containment (the point of use owns that).

use super::{
    ProgramChannel, ProgramConfig,
    config::{block_lines, kv_line},
    truncate_param,
};
use crate::parse::text::js_trim;
use crate::render::Warning;
use unicode_normalization::UnicodeNormalization;

#[cfg(test)]
mod tests;

/// A warning literal from `(key, value)` params, each value truncated (Q-2).
pub(super) fn warning(code: &'static str, params: &[(&str, &str)]) -> Warning {
    Warning {
        code,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), truncate_param(v)))
            .collect(),
    }
}

/// Why a vault-authored relative path can never become a contained one (TS `unsafeEntryReason` /
/// `unsafeDirReason`): one of the four spec tokens. TS `path.isAbsolute` is POSIX; the engine also
/// refuses `C:\`, `\\` and a leading `\` (Q-32, `deviates_from_ts`). Shape only, not containment.
pub(super) fn unsafe_path_reason(p: &str) -> Option<&'static str> {
    if p.is_empty() {
        return Some("empty");
    }
    let mut cs = p.chars();
    let drive = matches!(
        (cs.next(), cs.next(), cs.next()),
        (Some(d), Some(':'), Some('\\' | '/')) if d.is_ascii_alphabetic()
    );
    if p.starts_with(['/', '\\']) || drive {
        return Some("absolute");
    }
    let mut segs = p.split(['/', '\\']);
    if segs.clone().any(|s| s == "..") {
        return Some("parent_segment");
    }
    // NFKC then lowercase: the fold `resolveContained` uses (`ſ` -> `s`).
    if segs.any(|s| s.nfkc().collect::<String>().to_lowercase() == "node_modules") {
        return Some("reserved_segment");
    }
    None
}

/// Parse the `program:` block out of merged config lines; `None` when there is none.
#[allow(dead_code)] // reason: seam, used by T4.5
pub(crate) fn parse_program(lines: &[&str], warnings: &mut Vec<Warning>) -> Option<ProgramConfig> {
    let start = lines.iter().position(|l| js_trim(l) == "program:")?;
    let mut channel = ProgramChannel::Argv;
    let mut saw_channel = false;
    let mut entry = None;
    let mut flags: Vec<String> = vec![];
    for raw in block_lines(lines, start) {
        let t = js_trim(raw);
        if let Some(f) = t.strip_prefix("- ") {
            flags.push(js_trim(f).to_owned());
            continue;
        }
        let Some((key, val)) = kv_line(t) else {
            continue;
        };
        let val = js_trim(val);
        match key {
            "channel" => {
                saw_channel = true;
                channel = parse_channel(val, warnings);
            }
            "entry" => entry = parse_entry(val, warnings),
            "flags" => flags.extend(inline_list(val)),
            _ => {} // any other field, `__proto__` included, is simply unrecognised
        }
    }
    if !saw_channel {
        warnings.push(warning("leetcode.program.no_channel", &[]));
    }
    Some(ProgramConfig {
        channel,
        entry,
        flags: (!flags.is_empty()).then_some(flags),
    })
}

fn parse_channel(val: &str, warnings: &mut Vec<Warning>) -> ProgramChannel {
    match val {
        "argv" => ProgramChannel::Argv,
        "flags" => ProgramChannel::Flags,
        "stdin" => ProgramChannel::Stdin,
        _ => {
            warnings.push(warning(
                "leetcode.program.unknown_channel",
                &[("channel", val)],
            ));
            ProgramChannel::Argv
        }
    }
}

fn parse_entry(val: &str, warnings: &mut Vec<Warning>) -> Option<String> {
    match unsafe_path_reason(val) {
        Some(reason) => {
            warnings.push(warning(
                "leetcode.program.entry_rejected",
                &[("entry", val), ("reason", reason)],
            ));
            None
        }
        None => Some(val.to_owned()),
    }
}

/// `flags: [a, 'b']` -> `["a", "b"]`; anything not starting `[` -> `[]` (TS `parseInlineFlags`:
/// one leading `[`, one trailing `]`, each part trimmed and stripped of one edge quote each side).
fn inline_list(val: &str) -> Vec<String> {
    let Some(inner) = val.strip_prefix('[') else {
        return vec![];
    };
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    inner
        .split(',')
        .map(|part| {
            let t = js_trim(part);
            let t = t.strip_prefix(['\'', '"']).unwrap_or(t);
            t.strip_suffix(['\'', '"']).unwrap_or(t).to_owned()
        })
        .filter(|p| !p.is_empty())
        .collect()
}
