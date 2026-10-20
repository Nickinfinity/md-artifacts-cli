//! The one control-character rule (spec §9.1, `vks.control_char`), shared by the reader and the
//! emitter.

use crate::error::{VKS_CONTROL_CHAR, VarsError};
use crate::parse::text::js_lines;

/// C0 control (TAB excepted) or DEL.
pub(crate) fn is_control(c: char) -> bool {
    (c < ' ' && c != '\t') || c == '\u{7f}'
}

/// 1-based line of the first C0 control (TAB excepted) or DEL, after CRLF splitting so a `\r\n`
/// is never flagged.
pub(crate) fn control_line(body: &str) -> Option<usize> {
    js_lines(body)
        .position(|l| l.chars().any(is_control))
        .map(|i| i + 1)
}

pub(crate) fn control_error(line: usize) -> VarsError {
    VarsError::new(VKS_CONTROL_CHAR, line)
}
