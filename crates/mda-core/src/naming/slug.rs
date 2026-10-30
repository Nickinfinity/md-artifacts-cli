//! The slug rule (spec §14.1): the one place a title becomes a file or directory name.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Input cap in chars: bounds normalisation work (the stem is cut to 252 bytes anyway).
const MAX_SLUG_INPUT_CHARS: usize = 1024;

/// A file-system-safe slug: the first 1024 chars in NFC, lower-cased, NFC again; letters and
/// digits of any script kept (plus a combining mark right after a kept char), every other run
/// collapsed to one `-`, edge dashes trimmed. Empty when nothing is left.
///
/// ponytail: ZWNJ/ZWJ are dropped (Persian/Indic words split with `-`); `to_lowercase` is
/// locale-free (no Turkish special case).
///
/// # Examples
///
/// ```
/// use mda_core::naming::slugify;
/// assert_eq!(slugify("Button Group"), "button-group");
/// assert_eq!(slugify("日本語 メモ"), "日本語-メモ");
/// ```
pub fn slugify(s: &str) -> String {
    let capped: String = s.chars().take(MAX_SLUG_INPUT_CHARS).nfc().collect();
    let lower: String = capped.to_lowercase().nfc().collect();
    let mut out = String::new();
    for c in lower.chars() {
        let kept = !out.is_empty() && !out.ends_with('-');
        if c.is_alphanumeric() || (kept && is_combining_mark(c)) {
            out.push(c);
        } else if kept {
            out.push('-');
        }
    }
    out.truncate(out.trim_end_matches('-').len());
    out
}
