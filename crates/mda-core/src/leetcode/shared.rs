//! Helpers every W4 parser task needs (H4.0a): warning-param truncation and language ids.

use crate::language::normalize_lang_id;
use crate::parse::text::js_trim;

/// Longest warning param, in **chars** (Q-2: raw vault text, truncated on a char boundary).
pub const MAX_PARAM_CHARS: usize = 200;

/// A warning param: the first [`MAX_PARAM_CHARS`] chars of `s`.
///
/// # Examples
///
/// ```
/// let s = "é".repeat(201);
/// assert_eq!(mda_core::leetcode::truncate_param(&s).chars().count(), 200);
/// assert_eq!(mda_core::leetcode::truncate_param("ab"), "ab");
/// ```
pub fn truncate_param(s: &str) -> String {
    s.chars().take(MAX_PARAM_CHARS).collect()
}

/// The LeetCode extension's `LANG_ALIAS` rows that differ from `language::LANG_ALIAS` (Q-29): its
/// table maps `python3` and `objective-c`, and has no `objc`/`objcpp` rows.
const LEETCODE_ALIAS: &[(&str, &str)] = &[
    ("python3", "python"),
    ("objective-c", "objc"),
    ("objc", "objc"),
    ("objcpp", "objcpp"),
];

/// TS `resolveLangId` (`language-map.service.ts:31`): JS trim, lowercase, empty → `plaintext`, then
/// the LeetCode alias table (the overlay, then the shared rows).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::resolve_lang_id;
/// assert_eq!(resolve_lang_id(" JS "), "javascript");
/// assert_eq!(resolve_lang_id(""), "plaintext");
/// assert_eq!(resolve_lang_id("python3"), "python");
/// assert_eq!(resolve_lang_id("objc"), "objc");
/// assert_eq!(resolve_lang_id("__proto__"), "__proto__");
/// ```
pub fn resolve_lang_id(fence_lang: &str) -> String {
    let key = js_trim(fence_lang).to_lowercase();
    if key.is_empty() {
        return "plaintext".to_owned();
    }
    match LEETCODE_ALIAS.iter().find(|(k, _)| *k == key) {
        Some((_, v)) => (*v).to_owned(),
        None => normalize_lang_id(&key),
    }
}
