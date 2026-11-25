//! The small tables the layout scan consults: which fields a `key:` feeds, and the fence-info
//! predicates. Hints only; the whole-file guard arbitrates.

use crate::leetcode::LayoutField;
use crate::leetcode::LayoutField as F;
use crate::leetcode::config::{BODY_SET_KEYS, top_level_key};
use crate::leetcode::project::attr;

/// The §10.1 fields a top-level `key:` feeds, in frontmatter or in a config fence. `test:` also
/// feeds `checks` (spec §10.5).
pub(super) fn key_fields(line: &str, in_config: bool) -> Vec<LayoutField> {
    let Some(key) = top_level_key(line) else {
        return vec![];
    };
    // `without_body_set_keys` drops these from frontmatter before parsing: they feed nothing there.
    if !in_config && BODY_SET_KEYS.contains(&key) {
        return vec![];
    }
    match key {
        "title" => vec![F::Title],
        "leetcodeType" => vec![F::LeetcodeType],
        "difficulty" => vec![F::Difficulty],
        "status" => vec![F::Status],
        "algorithm" => vec![F::Algorithm],
        "tags" => vec![F::Tags],
        "function" => vec![F::FunctionName],
        "functions" => vec![F::Functions],
        "params" => vec![F::Params],
        "returns" => vec![F::Returns],
        "test" => vec![F::Test, F::Checks],
        "practice" => vec![F::Practice],
        "libs" => vec![F::Libs],
        "checks" => vec![F::Checks],
        "packages" => vec![F::Packages],
        "program" => vec![F::Program],
        _ => vec![],
    }
}

/// An info-string that is one bare `\w+` language (the setup/solution/attempt fence grammar).
pub(super) fn is_lang(info: &str) -> bool {
    !info.is_empty() && info.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// An info-string carrying a non-empty `path=` attribute (a `## Files` entry or overlay), by the
/// project parser's own reader.
pub(super) fn has_path(info: &str) -> bool {
    attr(info, "path").is_some_and(|p| !p.is_empty())
}
