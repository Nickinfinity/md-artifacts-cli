//! Body sections (TS `leetcode-sections.helpers.ts`): description, examples, the two case suites,
//! and the `# Setup` / `# Solutions` / `# Attempts` trees (`tree`). Pure, never fails: an absent or
//! malformed section is the empty value. The pre-flight lookahead splits (`(?=^## )`, `(?=^### )`)
//! are line scanners (`tree::split_before`). Heading regexes use `JS_S`, never Rust `\s`.

mod tree;

use std::sync::LazyLock;

use regex::Regex;

use super::bounds::{boundary_outside_fence, section_bounds};
use super::config::{ConfigSpan, extract_config_blocks};
use super::model::{CaseValue, Example, OrderedMap, TestCase};
use super::safe_json::safe_json;
use crate::parse::text::{JS_S, compile, js_trim};

pub(crate) use tree::{extract_attempts, extract_setups, extract_solutions};

static TESTS_RE: LazyLock<Regex> = LazyLock::new(|| compile(&format!("(?m)^## Tests{JS_S}*$")));
static FINAL_TESTS_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!("(?m)^## Final Tests{JS_S}*$")));
static EXAMPLES_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!("(?m)^## Examples{JS_S}*$")));
static EXAMPLE_FENCE: LazyLock<Regex> = LazyLock::new(|| compile(r"(?s)```example\r?\n(.*?)```"));
// `.` is every JS non-terminator; `\s*` may eat newlines, as in TS.
static INPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    compile(&format!(
        r"(?m)^input:{JS_S}*([^\n\r\x{{2028}}\x{{2029}}]+)"
    ))
});
static OUTPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    compile(&format!(
        r"(?m)^output:{JS_S}*([^\n\r\x{{2028}}\x{{2029}}]+)"
    ))
});
static CASE_FENCE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?s)```(json|yaml)([^\n]*)\r?\n(.*?)```"));
static CONFIG_INFO: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!("^{JS_S}*leetcode{JS_S}*$")));
static CHECK_ATTR: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!(r#"check=(?:"([^"]*)"|([^{JS_S}]+))"#)));

/// The two case-section headings, for the `project` parser's `check=` binding (TS `CASE_SECTIONS`).
pub(crate) struct CaseSections {
    pub tests: &'static LazyLock<Regex>,
    pub final_tests: &'static LazyLock<Regex>,
}

pub(crate) static CASE_SECTIONS: CaseSections = CaseSections {
    tests: &TESTS_RE,
    final_tests: &FINAL_TESTS_RE,
};

/// `^#{1,2} ` on a line (a `##` section's boundary).
fn is_h1_or_h2(line: &str) -> bool {
    line.starts_with("# ") || line.starts_with("## ")
}

/// Body of a `##` section: after its heading to the next `#`/`##` heading outside a fence.
fn extract_section<'a>(body: &'a str, heading: &Regex) -> Option<&'a str> {
    let b = section_bounds(body, heading, &is_h1_or_h2)?;
    body.get(b.heading_end..b.body_end)
}

/// `^#+ ` on a line: any Markdown heading.
fn is_heading(line: &str) -> bool {
    line.trim_start_matches('#').starts_with(' ') && line.starts_with('#')
}

/// Prose before the first heading outside a fence, minus every config-fence span, trimmed (TS
/// `extractDescription`).
///
/// # Examples
///
/// ```text
/// extract_description("Prose.\n\n```yaml leetcode\nreturns: int\n```\n\n## Examples") == "Prose."
/// ```
pub(crate) fn extract_description(body: &str) -> String {
    let end = boundary_outside_fence(body, 0, &is_heading);
    let text = body.get(..end).unwrap_or(body);
    js_trim(&without_spans(text, &extract_config_blocks(body).spans)).to_owned()
}

/// `text` with each span's characters removed; a span at or past the end stops the walk, one
/// straddling the end is clamped (TS `withoutSpans`).
fn without_spans(text: &str, spans: &[ConfigSpan]) -> String {
    let (mut out, mut cursor) = (String::new(), 0);
    for s in spans {
        if s.start >= text.len() {
            break;
        }
        out.push_str(text.get(cursor..s.start).unwrap_or(""));
        cursor = s.end.min(text.len());
    }
    out.push_str(text.get(cursor..).unwrap_or(""));
    out
}

/// Every ` ```example ` block under `## Examples` with both an `input:` and an `output:` line.
///
/// # Examples
///
/// ```text
/// extract_examples("## Examples\n```example\ninput: x = 1\noutput: 1\n```")
/// ```
pub(crate) fn extract_examples(body: &str) -> Vec<Example> {
    let Some(section) = extract_section(body, &EXAMPLES_RE) else {
        return Vec::new();
    };
    let cap = |re: &Regex, t: &str| {
        re.captures(t)
            .and_then(|c| c.get(1))
            .map(|m| js_trim(m.as_str()).to_owned())
    };
    EXAMPLE_FENCE
        .captures_iter(section)
        .filter_map(|c| {
            let t = c.get(1)?.as_str();
            Some(Example {
                input: cap(&INPUT_RE, t)?,
                output: cap(&OUTPUT_RE, t)?,
            })
        })
        .collect()
}

/// One json/yaml fence of a case section, with the `check=<name>` it names (TS `CaseFence`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CaseFence {
    /// `check=<name>` (quoted or bare); `None` on a bare fence.
    pub check: Option<String>,
    /// Every element of the fence's list, as written (an `http` case is not `input`/`expected`).
    pub raw: Vec<CaseValue>,
    /// The `input`/`expected` view of `raw`: object elements only, `Null` for a missing `expected`.
    pub cases: Vec<TestCase>,
}

/// Every ` ```json ` / ` ```yaml ` fence under a case section, in document order (TS
/// `extractCaseFences`). A ` ```yaml leetcode ` config fence is skipped; a malformed fence is a
/// fence with no cases.
pub(crate) fn extract_case_fences(body: &str, heading: &Regex) -> Vec<CaseFence> {
    let Some(section) = extract_section(body, heading) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for c in CASE_FENCE.captures_iter(section) {
        let (lang, info, text) = (&c[1], &c[2], &c[3]);
        if lang == "yaml" && CONFIG_INFO.is_match(info) {
            continue;
        }
        let parsed = if lang == "yaml" {
            super::cases::parse_yaml_cases(text)
        } else {
            safe_json(text).map_or(CaseValue::Null, |v| from_json(&v))
        };
        let raw = match parsed {
            CaseValue::List(v) => v,
            _ => Vec::new(),
        };
        let cases = raw.iter().filter_map(as_test_case).collect();
        out.push(CaseFence {
            check: check_attr(info),
            raw,
            cases,
        });
    }
    out
}

fn check_attr(info: &str) -> Option<String> {
    let c = CHECK_ATTR.captures(info)?;
    c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_owned())
}

fn as_test_case(v: &CaseValue) -> Option<TestCase> {
    let CaseValue::Object(m) = v else { return None };
    let get = |k: &str| m.0.iter().find(|(key, _)| key == k).map(|(_, v)| v);
    Some(TestCase {
        input: match get("input") {
            Some(CaseValue::Object(i)) => i.clone(),
            _ => OrderedMap(Vec::new()),
        },
        expected: get("expected").cloned().unwrap_or(CaseValue::Null),
    })
}

/// Cases of the `## Tests` section (all fences).
pub(crate) fn extract_tests(body: &str) -> Vec<TestCase> {
    cases_of(body, &TESTS_RE)
}

/// Cases of the `## Final Tests` section (the hidden grading suite; no legacy fallback here).
pub(crate) fn extract_final_tests(body: &str) -> Vec<TestCase> {
    cases_of(body, &FINAL_TESTS_RE)
}

fn cases_of(body: &str, heading: &Regex) -> Vec<TestCase> {
    extract_case_fences(body, heading)
        .into_iter()
        .flat_map(|f| f.cases)
        .collect()
}

/// JSON `Value` → `CaseValue`. A number is `Int` only when integral within 2^53 (JS `Number`),
/// else `Float`; objects keep JS order and duplicate-key semantics via `from_pairs`.
fn from_json(v: &serde_json::Value) -> CaseValue {
    use serde_json::Value as J;
    match v {
        J::Null => CaseValue::Null,
        J::Bool(b) => CaseValue::Bool(*b),
        J::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_992.0 => {
                CaseValue::Int(f as i64)
            }
            Some(f) => CaseValue::Float(f),
            None => CaseValue::Null,
        },
        J::String(s) => CaseValue::Str(s.clone()),
        J::Array(a) => CaseValue::List(a.iter().map(from_json).collect()),
        J::Object(o) => CaseValue::Object(OrderedMap::from_pairs(
            o.iter().map(|(k, v)| (k.clone(), from_json(v))),
        )),
    }
}

#[cfg(test)]
mod tests;
