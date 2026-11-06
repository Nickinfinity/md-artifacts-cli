//! The `# Setup` / `# Solutions` / `# Attempts` trees: `## <Language>` chunks, `### <Label>`
//! sub-chunks, fences with optional `<!-- meta|attempt: {…} -->` comments (TS sections helpers).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::super::bounds::section_bounds;
use super::super::model::{Attempt, ExerciseSetup, LeetCodeSolution};
use super::super::safe_json::safe_json;
use crate::parse::text::{JS_S, compile, js_trim, js_trim_end};

fn top_heading(name: &str) -> Regex {
    compile(&format!("(?m)^# {name}{JS_S}*$"))
}
static SETUP_RE: LazyLock<Regex> = LazyLock::new(|| top_heading("Setup"));
static SOLUTIONS_RE: LazyLock<Regex> = LazyLock::new(|| top_heading("Solutions"));
static ATTEMPTS_RE: LazyLock<Regex> = LazyLock::new(|| top_heading("Attempts"));
static META_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!(r"(?s)<!-- meta:{JS_S}*(\{{.*?\}}){JS_S}*-->")));
// `\w` is ASCII in JS: spelled out.
static FENCE_LANG: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?s)```[A-Za-z0-9_]+\r?\n(.*?)```"));
static FENCE_W_META: LazyLock<Regex> = LazyLock::new(|| fence_with("meta"));
static FENCE_W_ATTEMPT: LazyLock<Regex> = LazyLock::new(|| fence_with("attempt"));

fn fence_with(tag: &str) -> Regex {
    compile(&format!(
        r"(?s)(?:<!-- {tag}:{JS_S}*(\{{.*?\}}){JS_S}*-->{JS_S}*\r?\n)?```[A-Za-z0-9_]+\r?\n(.*?)```"
    ))
}

/// Body of a `#` section: `##` children stay, the next `#` heading outside a fence ends it.
fn top_section<'a>(body: &'a str, heading: &Regex) -> Option<&'a str> {
    let b = section_bounds(body, heading, &|l: &str| l.starts_with("# "))?;
    body.get(b.heading_end..b.body_end)
}

/// JS `text.split(/(?=^<prefix>)/m)`: the text before the first `prefix` line (possibly empty),
/// then one chunk per `prefix` line. The lookahead regex is a line scanner here (no lookahead in
/// the linear `regex` crate).
pub(super) fn split_before<'a>(text: &'a str, prefix: &str) -> (&'a str, Vec<&'a str>) {
    let mut cuts = Vec::new();
    let mut offset = 0;
    for line in text.split('\n') {
        if line.starts_with(prefix) {
            cuts.push(offset);
        }
        offset += line.len() + 1;
    }
    let lead = text
        .get(..cuts.first().copied().unwrap_or(text.len()))
        .unwrap_or("");
    let ends = cuts.iter().skip(1).copied().chain([text.len()]);
    let chunks = cuts
        .iter()
        .zip(ends)
        .filter_map(|(&a, b)| text.get(a..b))
        .collect();
    (lead, chunks)
}

/// `^<prefix>(.+)\r?\n` on a chunk's first line: the trimmed text and the rest after the newline.
fn heading_of<'a>(chunk: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = chunk.strip_prefix(prefix)?;
    let (line, after) = rest.split_once('\n')?;
    let line = line.strip_suffix('\r').unwrap_or(line);
    let bad = |c: char| matches!(c, '\r' | '\u{2028}' | '\u{2029}');
    (!line.is_empty() && !line.contains(bad)).then_some((js_trim(line), after))
}

fn language_chunks(section: &str) -> Vec<(String, &str)> {
    split_before(section, "## ")
        .1
        .into_iter()
        .filter_map(|c| heading_of(c, "## "))
        .map(|(h, rest)| (h.to_lowercase(), rest))
        .collect()
}

/// `# Setup`: the first fence under each `## <Language>`.
#[allow(dead_code)] // reason: seam, used by T4.5
pub(crate) fn extract_setups(body: &str) -> Vec<ExerciseSetup> {
    let Some(section) = top_section(body, &SETUP_RE) else {
        return Vec::new();
    };
    language_chunks(section)
        .into_iter()
        .filter_map(|(language, rest)| {
            let code = js_trim_end(FENCE_LANG.captures(rest)?.get(1)?.as_str()).to_owned();
            Some(ExerciseSetup { language, code })
        })
        .collect()
}

/// `# Solutions`: per language, direct fences (auto-numbered `Solution #N` when 2+) then `###`
/// labelled ones.
#[allow(dead_code)] // reason: seam, used by T4.5
pub(crate) fn extract_solutions(body: &str) -> Vec<LeetCodeSolution> {
    let Some(section) = top_section(body, &SOLUTIONS_RE) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (language, rest) in language_chunks(section) {
        let (lead, labelled) = split_before(rest, "### ");
        let mut direct: Vec<LeetCodeSolution> = FENCE_W_META
            .captures_iter(lead)
            .map(|c| solution(&language, None, c.get(2), c.get(1)))
            .collect();
        if direct.len() > 1 {
            for (i, s) in direct.iter_mut().enumerate() {
                s.label = Some(format!("Solution #{}", i + 1));
            }
        }
        out.extend(direct);
        for chunk in labelled {
            let Some((label, rest)) = heading_of(chunk, "### ") else {
                continue;
            };
            let Some(fence) = FENCE_LANG.captures(rest) else {
                continue;
            };
            let meta = META_RE.captures(rest);
            out.push(solution(
                &language,
                Some(label.to_owned()),
                fence.get(1),
                meta.as_ref().and_then(|m| m.get(1)),
            ));
        }
    }
    out
}

fn solution(
    language: &str,
    label: Option<String>,
    code: Option<regex::Match<'_>>,
    meta: Option<regex::Match<'_>>,
) -> LeetCodeSolution {
    let obj = meta.and_then(|m| comment_json(m.as_str()));
    let text = |k: &str| {
        obj.as_ref()
            .and_then(|o| o.get(k)?.as_str())
            .map(str::to_owned)
    };
    LeetCodeSolution {
        language: language.to_owned(),
        label,
        code: js_trim_end(code.map_or("", |m| m.as_str())).to_owned(),
        solved_at: text("solved_at"),
        duration: text("duration"),
    }
}

/// An HTML-comment payload: an object, or `None` when absent, malformed or not an object.
fn comment_json(raw: &str) -> Option<serde_json::Map<String, Value>> {
    match safe_json(raw)? {
        Value::Object(o) => Some(o),
        _ => None,
    }
}

/// `# Attempts`: each fence needs an `<!-- attempt: {…} -->` with string `at` and `duration` and
/// boolean `passed`; any other fence is skipped.
#[allow(dead_code)] // reason: seam, used by T4.5
pub(crate) fn extract_attempts(body: &str) -> Vec<Attempt> {
    let Some(section) = top_section(body, &ATTEMPTS_RE) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (language, rest) in language_chunks(section) {
        for c in FENCE_W_ATTEMPT.captures_iter(rest) {
            let Some(o) = c.get(1).and_then(|m| comment_json(m.as_str())) else {
                continue;
            };
            let text = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_owned);
            let (Some(at), Some(duration), Some(passed)) = (
                text("at"),
                text("duration"),
                o.get("passed").and_then(Value::as_bool),
            ) else {
                continue;
            };
            out.push(Attempt {
                language: language.clone(),
                at,
                duration,
                passed,
                big_o: text("bigO"),
                confidence: text("confidence"),
                code: js_trim_end(c.get(2).map_or("", |m| m.as_str())).to_owned(),
            });
        }
    }
    out
}
