//! The legacy `KEY=value` reader: TS `parseVarLines` (`parser.service.ts:233-242`) verbatim, plus
//! the one deliberate deviation: C0 controls (TAB excepted) and DEL are rejected (spec §9.6).

use crate::error::{VKS_CONTROL_CHAR, VarsError};
use crate::model::ParsedVar;
use crate::parse::text::{js_lines, js_trim};

/// C0 control (TAB excepted) or DEL: the one control rule, shared by the readers and the emitter.
pub(crate) fn is_control(c: char) -> bool {
    (c < ' ' && c != '\t') || c == '\u{7f}'
}

/// 1-based line of the first C0 control (TAB excepted) or DEL, after CRLF splitting so a `\r\n`
/// is never flagged. Shared with the YAML reader so both dialects agree on the rule.
pub(crate) fn control_line(body: &str) -> Option<usize> {
    js_lines(body)
        .position(|l| l.chars().any(is_control))
        .map(|i| i + 1)
}

pub(crate) fn control_error(line: usize) -> VarsError {
    VarsError::new(VKS_CONTROL_CHAR, line)
}

/// Lines containing `=` and not starting (after trim) with `#`; split on the first `=`; both halves
/// trimmed; empty names dropped. Duplicates are kept in order. Whole body scanned for controls first.
pub(crate) fn read_legacy(body: &str) -> Result<Vec<ParsedVar>, VarsError> {
    if let Some(line) = control_line(body) {
        return Err(control_error(line));
    }
    Ok(js_lines(body)
        .filter(|l| l.contains('=') && !js_trim(l).starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (js_trim(k), js_trim(v)))
        .filter(|(k, _)| !k.is_empty())
        .map(|(k, v)| ParsedVar::text(k, v))
        .collect())
}
