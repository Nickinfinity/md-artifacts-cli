//! `checks:` and the `check=<name>` case binding (TS `project-parser.helpers.ts`: `parseChecks`,
//! `buildCheck`, `bindCases`; spec §9.2). Only the fields a kind owns are read, from a fixed
//! allowlist (`__proto__` never lands); a check whose kind is unknown or whose required fields are
//! missing is dropped with a warning. A kind the vocabulary declares but nothing runs (`program`,
//! `class`, `in-place`) is *reserved*: dropped, and recorded in `unimplementedKinds` so a grader can
//! refuse the artifact as a whole.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use super::{
    BuildCheck, CheckBase, CssAssertCheck, DomAssertCheck, FunctionCheck, HttpCheck, ProjectCheck,
    config::{block_lines, is_ws, unquote_pair},
    kind::TEST_TYPES,
    program::warning,
    safe_json::safe_json,
    sections::{CASE_SECTIONS, extract_case_fences},
};
use crate::error::{
    LEETCODE_CHECKS_BUILD_NEEDS_ARGV, LEETCODE_CHECKS_CALL_NEEDS_FUNCTION,
    LEETCODE_CHECKS_FENCE_UNBOUND, LEETCODE_CHECKS_FENCE_UNKNOWN_CHECK,
    LEETCODE_CHECKS_HTTP_NEEDS_PACKAGE, LEETCODE_CHECKS_NEEDS_FILE, LEETCODE_CHECKS_NOT_A_LIST,
    LEETCODE_CHECKS_RESERVED_KIND, LEETCODE_CHECKS_UNKNOWN_KIND,
};
use crate::parse::text::js_trim;
use crate::render::Warning;

/// The five kinds a check may declare (the `ProjectCheck` discriminants).
#[derive(Clone, Copy)]
enum Kind {
    Call,
    Build,
    DomAssert,
    CssAssert,
    Http,
}

enum Class {
    Valid(Kind),
    /// In the one test-type vocabulary but not executable as a check.
    Reserved,
    Unknown,
}

fn classify(kind: &str) -> Class {
    match kind {
        "call" => Class::Valid(Kind::Call),
        "build" => Class::Valid(Kind::Build),
        "dom-assert" => Class::Valid(Kind::DomAssert),
        "css-assert" => Class::Valid(Kind::CssAssert),
        "http" => Class::Valid(Kind::Http),
        k if TEST_TYPES.iter().any(|t| t.name == k) => Class::Reserved,
        _ => Class::Unknown,
    }
}

/// The fields of one `- ` entry (TS `CHECK_FIELDS`); a repeated field keeps its last value.
#[derive(Default)]
struct Fields<'a> {
    name: Option<&'a str>,
    kind: Option<&'a str>,
    file: Option<&'a str>,
    function: Option<&'a str>,
    argv: Option<&'a str>,
    dir: Option<&'a str>,
    package: Option<&'a str>,
}

impl<'a> Fields<'a> {
    fn set(&mut self, field: &str, value: &'a str) {
        let slot = match field {
            "name" => &mut self.name,
            "kind" => &mut self.kind,
            "file" => &mut self.file,
            "function" => &mut self.function,
            "argv" => &mut self.argv,
            "dir" => &mut self.dir,
            "package" => &mut self.package,
            _ => return,
        };
        *slot = Some(value);
    }
}

/// Parse the `checks:` list of merged config lines into checks with no cases yet.
pub(crate) fn parse_checks(
    lines: &[&str],
    warnings: &mut Vec<Warning>,
    unimplemented: &mut Vec<String>,
) -> Vec<ProjectCheck> {
    let Some(start) = lines
        .iter()
        .position(|l| l.trim_start_matches(is_ws).starts_with("checks:"))
    else {
        return vec![];
    };
    // `/^\s*checks:(.*)$/`: `.` refuses a line terminator, so such a line has no header match.
    let rest = lines
        .get(start)
        .and_then(|l| l.trim_start_matches(is_ws).strip_prefix("checks:"))
        .filter(|r| !r.contains(['\r', '\u{2028}', '\u{2029}']));
    if let Some(got) = rest.map(js_trim).filter(|g| !g.is_empty()) {
        warnings.push(warning(LEETCODE_CHECKS_NOT_A_LIST, &[("got", got)]));
        return vec![];
    }
    let mut out = vec![];
    for entry in split_entries(&block_lines(lines, start)) {
        out.extend(build_check(&entry, warnings, unimplemented));
    }
    out
}

/// Group a list block's lines into one field set per `- ` entry.
fn split_entries<'a>(lines: &[&'a str]) -> Vec<Fields<'a>> {
    let mut entries: Vec<Fields<'a>> = vec![];
    for line in lines {
        let t = js_trim(line);
        let new = t.strip_prefix("- ");
        if new.is_some() {
            entries.push(Fields::default());
        }
        let kv = super::project::split_key_value(new.unwrap_or(t));
        if let (Some((key, rest)), Some(entry)) = (kv, entries.last_mut()) {
            entry.set(js_trim(key), js_trim(rest));
        }
    }
    entries
}

fn base(name: &str) -> CheckBase {
    CheckBase {
        name: name.to_owned(),
        cases: vec![],
        public_count: 0,
    }
}

/// A dropped check: the warning names the check plus any extra params.
fn dropped(
    warnings: &mut Vec<Warning>,
    code: &'static str,
    name: &str,
    extra: &[(&str, &str)],
) -> Option<ProjectCheck> {
    let mut p = vec![("name", name)];
    p.extend_from_slice(extra);
    warnings.push(warning(code, &p));
    None
}

/// One check from its fields, or `None` when it cannot be trusted.
fn build_check(
    f: &Fields<'_>,
    warnings: &mut Vec<Warning>,
    unimplemented: &mut Vec<String>,
) -> Option<ProjectCheck> {
    let name = unquote_pair(f.name.unwrap_or(""));
    let kind = f.kind.unwrap_or("");
    if name.is_empty() {
        return None;
    }
    let k = match classify(kind) {
        Class::Valid(k) => k,
        Class::Reserved => {
            if !unimplemented.iter().any(|u| u == kind) {
                unimplemented.push(kind.to_owned());
            }
            return dropped(
                warnings,
                LEETCODE_CHECKS_RESERVED_KIND,
                name,
                &[("kind", kind)],
            );
        }
        Class::Unknown => {
            return dropped(
                warnings,
                LEETCODE_CHECKS_UNKNOWN_KIND,
                name,
                &[("kind", kind)],
            );
        }
    };
    let package = unquote_pair(f.package.unwrap_or(""));
    match k {
        Kind::Http if package.is_empty() => {
            dropped(warnings, LEETCODE_CHECKS_HTTP_NEEDS_PACKAGE, name, &[])
        }
        Kind::Http => Some(ProjectCheck::Http(HttpCheck {
            base: base(name),
            package: package.to_owned(),
        })),
        Kind::Build => {
            let Some(argv) = string_array(f.argv.unwrap_or("")) else {
                return dropped(warnings, LEETCODE_CHECKS_BUILD_NEEDS_ARGV, name, &[]);
            };
            let dir = f
                .dir
                .filter(|d| !d.is_empty())
                .map(|d| unquote_pair(d).to_owned());
            Some(ProjectCheck::Build(BuildCheck {
                base: base(name),
                argv,
                dir,
            }))
        }
        _ => file_check(k, f, name, package, warnings),
    }
}

/// The file-based kinds (`call`, `dom-assert`, `css-assert`): all need `file:`.
fn file_check(
    k: Kind,
    f: &Fields<'_>,
    name: &str,
    package: &str,
    warnings: &mut Vec<Warning>,
) -> Option<ProjectCheck> {
    let file = unquote_pair(f.file.unwrap_or(""));
    if file.is_empty() {
        return dropped(warnings, LEETCODE_CHECKS_NEEDS_FILE, name, &[]);
    }
    let package = (!package.is_empty()).then(|| package.to_owned());
    match k {
        Kind::Call => {
            let function = unquote_pair(f.function.unwrap_or(""));
            if function.is_empty() {
                return dropped(warnings, LEETCODE_CHECKS_CALL_NEEDS_FUNCTION, name, &[]);
            }
            Some(ProjectCheck::Call(FunctionCheck {
                base: base(name),
                file: file.to_owned(),
                function: function.to_owned(),
            }))
        }
        Kind::DomAssert => Some(ProjectCheck::DomAssert(DomAssertCheck {
            base: base(name),
            file: file.to_owned(),
            package,
        })),
        _ => Some(ProjectCheck::CssAssert(CssAssertCheck {
            base: base(name),
            file: file.to_owned(),
            package,
        })),
    }
}

/// A JSON array of strings, else `None`.
fn string_array(raw: &str) -> Option<Vec<String>> {
    let serde_json::Value::Array(items) = safe_json(raw)? else {
        return None;
    };
    items
        .into_iter()
        .map(|v| match v {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        })
        .collect()
}

fn base_mut(c: &mut ProjectCheck) -> &mut CheckBase {
    match c {
        ProjectCheck::Call(x) => &mut x.base,
        ProjectCheck::Build(x) => &mut x.base,
        ProjectCheck::DomAssert(x) => &mut x.base,
        ProjectCheck::CssAssert(x) => &mut x.base,
        ProjectCheck::Http(x) => &mut x.base,
    }
}

/// Attach each ` ```json check=<name> ` fence's cases to the check it names. A bare fence binds
/// to the sole check, and warns when there are several (guessing would grade the wrong thing).
/// Public fences bind first, so `cases` keep public-then-final order and `public_count` (counted
/// on `cases`, never `raw`) is the boundary. Names resolve through a first-index map: linear.
pub(crate) fn bind_cases(checks: &mut [ProjectCheck], body: &str, warnings: &mut Vec<Warning>) {
    if checks.is_empty() {
        return;
    }
    let public = extract_case_fences(body, CASE_SECTIONS.tests)
        .into_iter()
        .map(|f| (f, true));
    let hidden = extract_case_fences(body, CASE_SECTIONS.final_tests)
        .into_iter()
        .map(|f| (f, false));
    let mut index: HashMap<String, usize> = HashMap::new();
    for (i, c) in checks.iter_mut().enumerate() {
        index.entry(base_mut(c).name.clone()).or_insert(i);
    }
    let sole = (checks.len() == 1).then_some(0);
    for (fence, is_public) in public.chain(hidden) {
        let target = match &fence.check {
            Some(n) => match index.get(n) {
                Some(&i) => i,
                None => {
                    warnings.push(warning(LEETCODE_CHECKS_FENCE_UNKNOWN_CHECK, &[("name", n)]));
                    continue;
                }
            },
            None => match sole {
                Some(i) => i,
                None => {
                    warnings.push(warning(LEETCODE_CHECKS_FENCE_UNBOUND, &[]));
                    continue;
                }
            },
        };
        let Some(b) = checks.get_mut(target).map(base_mut) else {
            continue;
        };
        if is_public {
            b.public_count += fence.cases.len();
        }
        b.cases.extend(fence.cases);
    }
}
