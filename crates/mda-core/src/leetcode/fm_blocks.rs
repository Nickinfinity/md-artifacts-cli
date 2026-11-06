//! The five indented sub-blocks of a LeetCode frontmatter/config text (TS
//! `leetcode-frontmatter-blocks.helpers.ts`): `test:`, `practice:`, `functions:`, `tags:`,
//! `params:`. Each takes the text's lines (`js_lines`) and the header index and returns the parsed
//! value plus the index of the first line it did not consume.

use super::config::{kv_line, starts_with_ws};
use super::kind::parse_test_type;
use super::{OrderedMap, ParamDef, PracticeConfig, PracticeOptionId, TestConfig, resolve_lang_id};
use crate::migrate::frontmatter::unquote;
use crate::parse::text::js_trim;

const MIN_TEST_TIMEOUT_MS: i64 = 100;
const MAX_SUITE_TIMEOUT_MS: i64 = 60_000;

/// A parsed `test:` block plus the `type:` scalar as written (TS `test.rawType`, which the type
/// derivation needs before an unknown value collapses to `call`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TestBlock {
    pub test: TestConfig,
    pub raw_type: Option<String>,
}

/// JS `Number.parseInt(s, 10)` on an already trimmed value: optional sign, then the digit prefix;
/// `None` is NaN. Saturates instead of overflowing (JS yields 1e21+, every caller clamps or caps).
pub(crate) fn js_parse_int(s: &str) -> Option<i64> {
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .map_or(rest, |n| &rest[..n]);
    if digits.is_empty() {
        return None;
    }
    let n = digits
        .chars()
        .try_fold(0i64, |acc, c| {
            acc.checked_mul(10)?.checked_add(i64::from(c.to_digit(10)?))
        })
        .unwrap_or(i64::MAX);
    Some(if neg { -n } else { n })
}

/// Consecutive indented lines of the block at `start`, each non-blank one trimmed and handed to
/// `on_line`; a line that does not start with whitespace (an empty one too) ends it.
fn scan_indented(lines: &[&str], start: usize, mut on_line: impl FnMut(&str)) -> usize {
    let mut i = start + 1;
    while let Some(line) = lines.get(i).filter(|l| starts_with_ws(l)) {
        let trimmed = js_trim(line);
        if !trimmed.is_empty() {
            on_line(trimmed);
        }
        i += 1;
    }
    i
}

/// `key: value` of one (trimmed) line with the value trimmed.
fn kv(line: &str) -> Option<(&str, &str)> {
    kv_line(line).map(|(k, v)| (k, js_trim(v)))
}

/// TS `parseTestBlock`. An unknown `type:` collapses to `call`; `timeoutMs` clamps to `[100, 60000]`.
pub(crate) fn parse_test_block(lines: &[&str], start: usize) -> (TestBlock, usize) {
    let mut b = TestBlock::default();
    let next = scan_indented(lines, start, |line| match kv(line) {
        Some(("type", v)) => {
            b.raw_type = Some(v.to_owned());
            b.test.r#type = parse_test_type(v).unwrap_or(TestConfig::default().r#type);
        }
        Some(("timeoutMs", v)) => {
            b.test.timeout_ms = js_parse_int(v).map_or(TestConfig::default().timeout_ms, |n| {
                // clamped into u32 range, so the cast cannot truncate
                n.clamp(MIN_TEST_TIMEOUT_MS, MAX_SUITE_TIMEOUT_MS) as u32
            });
        }
        _ => {}
    });
    (b, next)
}

fn option_id(s: &str) -> Option<PracticeOptionId> {
    match s {
        "noCompletion" => Some(PracticeOptionId::NoCompletion),
        "noAiAgents" => Some(PracticeOptionId::NoAiAgents),
        "noSnippets" => Some(PracticeOptionId::NoSnippets),
        "noParameterHints" => Some(PracticeOptionId::NoParameterHints),
        _ => None,
    }
}

/// Append `raw` (quotes off each end) when it is a known id not yet collected.
fn push_option(out: &mut Vec<PracticeOptionId>, raw: &str) {
    if let Some(id) = option_id(unquote(raw))
        && !out.contains(&id)
    {
        out.push(id);
    }
}

/// `val` without one leading `[` and one trailing `]` (TS `replace(/^\[|\]$/g, '')`).
fn strip_brackets(val: &str) -> &str {
    let v = val.strip_prefix('[').unwrap_or(val);
    v.strip_suffix(']').unwrap_or(v)
}

/// TS `parsePracticeBlock`. No `options` key keeps the defaults; `options: []` means none.
pub(crate) fn parse_practice_block(lines: &[&str], start: usize) -> (PracticeConfig, usize) {
    let mut p = PracticeConfig::default();
    let mut saw_options = false;
    let mut collected = vec![];
    let next = scan_indented(lines, start, |line| {
        if let Some(item) = line.strip_prefix("- ") {
            push_option(&mut collected, js_trim(item));
            return;
        }
        match kv(line) {
            Some(("timeLimit", v)) => {
                // ponytail: JS keeps any integer; the wire is u32 — saturate, widen if a client needs more
                p.time_limit_minutes = match js_parse_int(v) {
                    Some(n) if n >= 0 => u32::try_from(n).unwrap_or(u32::MAX),
                    _ => 0,
                };
            }
            Some(("locked", v)) => p.locked = v == "true",
            Some(("options", v)) => {
                saw_options = true;
                if v.starts_with('[') {
                    for raw in strip_brackets(v).split(',') {
                        push_option(&mut collected, js_trim(raw));
                    }
                }
            }
            _ => {}
        }
    });
    if saw_options {
        p.options = collected;
    }
    (p, next)
}

/// TS `parseFunctionsBlock`: `<language>: <name>` lines, the language through `resolve_lang_id`.
/// Built with `from_pairs` (a hostile block is O(n)); an empty name is skipped.
pub(crate) fn parse_functions_block(lines: &[&str], start: usize) -> (OrderedMap<String>, usize) {
    let mut pairs = vec![];
    let next = scan_indented(lines, start, |line| {
        if let Some((k, v)) = kv(line).filter(|(_, v)| !v.is_empty()) {
            pairs.push((resolve_lang_id(k), v.to_owned()));
        }
    });
    (OrderedMap::from_pairs(pairs), next)
}

/// TS `parseTagsBlock`: inline `[a, b]` or an indented `- a` list; any string is a tag.
pub(crate) fn parse_tags_block(lines: &[&str], start: usize, val: &str) -> (Vec<String>, usize) {
    if val.starts_with('[') {
        let tags = strip_brackets(val)
            .split(',')
            .map(|t| unquote(js_trim(t)))
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect();
        // an all-blank inner (`[ ]`) trims to "" in TS before the split and yields no tags
        return (tags, start + 1);
    }
    let mut tags = vec![];
    let next = scan_indented(lines, start, |line| {
        if let Some(item) = line.strip_prefix("- ") {
            tags.push(unquote(js_trim(item)).to_owned());
        }
    });
    (tags, next)
}

/// A param under construction (TS `Partial<ParamDef>`).
#[derive(Default)]
struct Pending {
    name: String,
    r#type: String,
}

impl Pending {
    fn assign(&mut self, raw: &str) {
        match kv(raw) {
            Some(("name", v)) => self.name = v.to_owned(),
            Some(("type", v)) => self.r#type = v.to_owned(),
            _ => {}
        }
    }
    /// Push when both fields are non-empty (TS truthiness).
    fn flush(&mut self, out: &mut Vec<ParamDef>) {
        let done = std::mem::take(self);
        if !done.name.is_empty() && !done.r#type.is_empty() {
            out.push(ParamDef {
                name: done.name,
                r#type: done.r#type,
            });
        }
    }
}

/// TS `parseParamsBlock`: an inline value (`[]`) means no params; else `- name:`/`type:` pairs.
pub(crate) fn parse_params_block(
    lines: &[&str],
    start: usize,
    val: &str,
) -> (Vec<ParamDef>, usize) {
    let mut params = vec![];
    if !val.is_empty() {
        return (params, start + 1);
    }
    let mut cur = Pending::default();
    let next = scan_indented(lines, start, |line| match line.strip_prefix("- ") {
        Some(item) => {
            cur.flush(&mut params);
            cur.assign(js_trim(item));
        }
        None => cur.assign(line),
    });
    cur.flush(&mut params);
    (params, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leetcode::PracticeOptionId::*;
    use crate::leetcode::TestTypeId;
    use crate::parse::text::js_lines;

    fn ls(s: &str) -> Vec<&str> {
        js_lines(s).collect()
    }
    fn tags(src: &str, val: &str) -> (Vec<String>, usize) {
        parse_tags_block(&ls(src), 0, val)
    }
    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn test_block_type_and_clamped_timeout() {
        let (b, next) =
            parse_test_block(&ls("test:\n  type: program\n  timeoutMs: 2000\nnext:"), 0);
        assert_eq!(b.test.r#type, TestTypeId::Program);
        assert_eq!(b.test.timeout_ms, 2000);
        assert_eq!(b.raw_type.as_deref(), Some("program"));
        assert_eq!(next, 3);
        let low = parse_test_block(&ls("test:\n  timeoutMs: 5"), 0).0;
        assert_eq!(low.test.timeout_ms, 100);
        let high = parse_test_block(&ls("test:\n  timeoutMs: 999999999999999999999"), 0).0;
        assert_eq!(high.test.timeout_ms, 60_000);
        let junk = parse_test_block(&ls("test:\n  timeoutMs: soon"), 0).0;
        assert_eq!(junk.test.timeout_ms, 5000);
        let prefix = parse_test_block(&ls("test:\n  timeoutMs: 250ms"), 0).0;
        assert_eq!(prefix.test.timeout_ms, 250);
    }

    #[test]
    fn test_block_unknown_type_collapses_but_raw_is_kept() {
        let b = parse_test_block(&ls("test:\n  type: project"), 0).0;
        assert_eq!(b.test.r#type, TestTypeId::Call);
        assert_eq!(b.raw_type.as_deref(), Some("project"));
        let none = parse_test_block(&ls("test:\n  timeoutMs: 300"), 0).0;
        assert_eq!(none.raw_type, None);
        let reserved = parse_test_block(&ls("test:\n  type: in-place"), 0).0;
        assert_eq!(reserved.test.r#type, TestTypeId::InPlace);
    }

    #[test]
    fn practice_block_options_limit_locked() {
        let p = parse_practice_block(
            &ls("practice:\n  timeLimit: 30\n  locked: true\n  options: [noAiAgents, 'noSnippets', bogus, noAiAgents]"),
            0,
        )
        .0;
        assert_eq!(p.time_limit_minutes, 30);
        assert!(p.locked);
        assert_eq!(p.options, vec![NoAiAgents, NoSnippets]);
    }

    #[test]
    fn practice_block_list_form_empty_and_defaults() {
        let list = parse_practice_block(
            &ls("practice:\n  options:\n    - noParameterHints\n    - \"noCompletion\""),
            0,
        )
        .0;
        assert_eq!(list.options, vec![NoParameterHints, NoCompletion]);
        let empty = parse_practice_block(&ls("practice:\n  options: []"), 0).0;
        assert!(empty.options.is_empty());
        let kept = parse_practice_block(&ls("practice:\n  timeLimit: -3"), 0).0;
        assert_eq!(kept, PracticeConfig::default());
    }

    #[test]
    fn functions_block_aliases_and_skips_empty() {
        let (f, next) = parse_functions_block(
            &ls("functions:\n  py: ab_check\n  rust: \n  JS: abCheck\nz: 1"),
            0,
        );
        assert_eq!(
            f.0,
            vec![
                ("python".into(), "ab_check".into()),
                ("javascript".into(), "abCheck".into())
            ]
        );
        assert_eq!(next, 4);
    }

    #[test]
    fn functions_block_duplicate_keeps_first_position_last_value_and_proto_is_data() {
        let f = parse_functions_block(
            &ls("functions:\n  python: a\n  rust: b\n  py: c\n  __proto__: x"),
            0,
        )
        .0;
        assert_eq!(
            f.0,
            vec![
                ("python".into(), "c".into()),
                ("rust".into(), "b".into()),
                ("__proto__".into(), "x".into()),
            ]
        );
    }

    #[test]
    fn tags_inline_and_block() {
        assert_eq!(
            tags(
                "tags: [leetcode, arrays, 'hash-map']",
                "[leetcode, arrays, 'hash-map']"
            ),
            (strs(&["leetcode", "arrays", "hash-map"]), 1)
        );
        assert_eq!(tags("tags: []", "[]").0, Vec::<String>::new());
        assert_eq!(tags("tags: [a,, b ,]", "[a,, b ,]").0, strs(&["a", "b"]));
        assert_eq!(
            tags("tags:\n  - a\n  - \"b c\"\nx: 1", "").0,
            strs(&["a", "b c"])
        );
        assert_eq!(tags("tags:\n  - a\nx: 1", "").1, 2);
    }

    #[test]
    fn params_inline_value_is_empty_and_block_form_needs_name_and_type() {
        let (p, next) = parse_params_block(&ls("params: []"), 0, "[]");
        assert!(p.is_empty());
        assert_eq!(next, 1);
        let src = "params:\n  - name: nums\n    type: int[]\n  - name: only\n  - type: lonely\n  - name: t\n    type: int\nreturns: x";
        let (p, next) = parse_params_block(&ls(src), 0, "");
        let got: Vec<_> = p
            .iter()
            .map(|x| (x.name.as_str(), x.r#type.as_str()))
            .collect();
        assert_eq!(got, vec![("nums", "int[]"), ("t", "int")]);
        assert_eq!(next, 7);
    }

    #[test]
    fn blank_line_ends_a_block_but_whitespace_only_does_not() {
        assert_eq!(
            parse_params_block(&ls("params:\n  - name: a\n\n    type: int"), 0, "").1,
            2
        );
        assert_eq!(
            parse_params_block(&ls("params:\n  - name: a\n  \n    type: int"), 0, "")
                .0
                .len(),
            1
        );
    }
}
