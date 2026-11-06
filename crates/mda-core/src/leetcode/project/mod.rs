//! The `project` half of an artifact (TS `project-parser.helpers.ts`, W4 T4.3; spec §9.1, §9.1b,
//! §9.4): the `## Files` tree, `# Solutions` overlays, `libs:` (in `decl`), and the unknown-key
//! warning. `checks:` and the `check=` binding are `checks`. Paths are not validated here: the
//! writer asserts containment at the point of use. Every `\s`/`\S` is the JS set (`JS_S`).

mod decl;
#[cfg(test)]
pub(crate) mod tests;

use std::sync::LazyLock;

use regex::Regex;

use super::{
    FileRole, FileSpec, LibSpec, PackageSpec, ProjectCheck,
    bounds::section_bounds,
    checks::{bind_cases, parse_checks},
    config::{BODY_SET_KEYS, RETAINED_FM_KEYS, is_ws, top_level_key},
    packages::parse_packages,
    program::warning,
    resolve_lang_id,
};
use crate::error::{LEETCODE_CONFIG_UNKNOWN_KEY, LEETCODE_FILES_UNKNOWN_ROLE};
use crate::parse::text::{JS_S, compile};
use crate::render::Warning;

pub(crate) use decl::parse_lib_declarations;
pub(in crate::leetcode) use decl::split_key_value;

static FILES_RE: LazyLock<Regex> = LazyLock::new(|| compile(&format!("(?m)^## Files{JS_S}*$")));
static SOLUTIONS_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!("(?m)^# Solutions{JS_S}*$")));
static FENCE_RE: LazyLock<Regex> = LazyLock::new(|| compile(r"(?s)```([^\n]*)\r?\n(.*?)```"));

/// What [`parse_project_artifact`] reads out of an artifact (TS `ProjectSections`).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ProjectSections {
    /// `## Files` entries, in document order.
    pub files: Vec<FileSpec>,
    /// `libs:` lists, allowlist-filtered.
    pub libs: LibSpec,
    /// `checks:` entries with their bound cases.
    pub checks: Vec<ProjectCheck>,
    /// `packages:` entries; `[]` when absent and when the block was refused.
    pub packages: Vec<PackageSpec>,
    /// `# Solutions` fences carrying `path=`: reference overlays for `## Files`.
    pub solution_files: Vec<FileSpec>,
    pub warnings: Vec<Warning>,
    /// Check kinds declared that nothing implements, deduplicated, first-seen order.
    pub unimplemented_kinds: Vec<String>,
}

/// Parse the `project` grammar of `config_raw` (the merged config text: retained frontmatter plus
/// every config fence) and `body`. Every step degrades with a warning. Warning order is the TS
/// call order: near-miss keys, checks, case binding, packages, `## Files`, `libs:`, `# Solutions`.
pub(crate) fn parse_project_artifact(config_raw: &str, body: &str) -> ProjectSections {
    let lines: Vec<&str> = config_raw
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut warnings = vec![];
    let mut unimplemented_kinds = vec![];
    warn_near_miss_keys(&lines, &mut warnings);
    let mut checks = parse_checks(&lines, &mut warnings, &mut unimplemented_kinds);
    bind_cases(&mut checks, body, &mut warnings);
    let packages = parse_packages(&lines, &mut warnings);
    let files = parse_files(body, &FILES_RE, &is_h1_or_h2, &mut warnings);
    let libs = parse_lib_declarations(&lines, &mut warnings);
    let solution_files = parse_files(body, &SOLUTIONS_RE, &is_h1, &mut warnings);
    ProjectSections {
        files,
        libs,
        checks,
        packages,
        solution_files,
        warnings,
        unimplemented_kinds,
    }
}

/// `^#{1,2} ` on a line.
fn is_h1_or_h2(line: &str) -> bool {
    line.starts_with("# ") || line.starts_with("## ")
}

/// `^# ` on a line: `# Solutions` is a `#`-level section, its `##` headings stay inside it.
fn is_h1(line: &str) -> bool {
    line.starts_with("# ")
}

/// Every fence of a section carrying `path=`. A fence without one is prose, skipped silently.
fn parse_files(
    body: &str,
    heading: &Regex,
    boundary: &dyn Fn(&str) -> bool,
    warnings: &mut Vec<Warning>,
) -> Vec<FileSpec> {
    let Some(section) =
        section_bounds(body, heading, boundary).and_then(|b| body.get(b.heading_end..b.body_end))
    else {
        return vec![];
    };
    let mut out = vec![];
    for c in FENCE_RE.captures_iter(section) {
        let (info, content) = (&c[1], &c[2]);
        let Some(path) = attr(info, "path").filter(|p| !p.is_empty()) else {
            continue;
        };
        let lang = info.split(is_ws).next().unwrap_or("");
        out.push(FileSpec {
            path: path.to_owned(),
            language: resolve_lang_id(lang),
            role: parse_role(attr(info, "role"), warnings),
            content: content.to_owned(),
        });
    }
    out
}

/// `key=value` (quoted or bare) from an info-string: TS `new RegExp(`${key}=(?:"([^"]*)"|(\S+))`)`
/// as a scanner (the key is a constant, never vault text; unanchored, so `xpath=` matches `path=`).
fn attr<'a>(info: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=");
    info.match_indices(&needle).find_map(|(i, _)| {
        let rest = info.get(i + needle.len()..)?;
        if let Some(inner) = rest.strip_prefix('"').and_then(|r| r.split_once('"')) {
            return Some(inner.0);
        }
        let end = rest.find(is_ws).unwrap_or(rest.len());
        rest.get(..end).filter(|v| !v.is_empty())
    })
}

/// An absent role means `editable`; an unrecognised one is a typo worth naming.
fn parse_role(raw: Option<&str>, warnings: &mut Vec<Warning>) -> FileRole {
    match raw {
        None | Some("editable") => FileRole::Editable,
        Some("readonly") => FileRole::Readonly,
        Some("hidden") => FileRole::Hidden,
        Some(other) => {
            warnings.push(warning(LEETCODE_FILES_UNKNOWN_ROLE, &[("role", other)]));
            FileRole::Editable
        }
    }
}

/// Warn when a column-0 key is within two edits of a known one (`servcies:`). The **first** known
/// key in array order wins, not the closest; short keys match almost anything (spec §10.4 W23).
fn warn_near_miss_keys(lines: &[&str], warnings: &mut Vec<Warning>) {
    let known: Vec<&str> = RETAINED_FM_KEYS
        .iter()
        .chain(BODY_SET_KEYS)
        .copied()
        .collect();
    for line in lines {
        let Some(key) = top_level_key(line) else {
            continue;
        };
        if known.contains(&key) {
            continue;
        }
        if let Some(near) = known.iter().find(|k| edit_distance(key, k) <= 2) {
            warnings.push(warning(
                LEETCODE_CONFIG_UNKNOWN_KEY,
                &[("key", key), ("suggestion", near)],
            ));
        }
    }
}

/// Levenshtein distance over bytes (both sides are ASCII `\w` keys), one row; `3` stands for
/// "more than two" when the lengths already differ by more.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len().abs_diff(b.len()) > 2 {
        return 3;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let (mut diag, mut left) = (i, i + 1);
        for (cb, cell) in b.iter().zip(row.iter_mut().skip(1)) {
            let up = *cell;
            left = (diag + usize::from(ca != cb)).min(up + 1).min(left + 1);
            *cell = left;
            diag = up;
        }
    }
    if b.is_empty() {
        a.len()
    } else {
        row.last().copied().unwrap_or(0)
    }
}
