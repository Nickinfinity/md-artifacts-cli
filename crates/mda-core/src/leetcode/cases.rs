//! The YAML case subset of `## Tests` / `## Final Tests` (TS `services/yaml-cases.helpers.ts`, parse
//! half; the emitter is W6's): **JSON's type system in YAML's syntax**. A quoted scalar is always a
//! string; an unquoted one is typed by JSON's rules only (`true`/`false`/`null`/`~`/strict number),
//! so YAML 1.1 forms (`yes`, `0051`, `1:1`, `.inf`) stay strings. A mistyped expected would verify
//! green while teaching the wrong answer, so ambiguity is a string and shape errors are `Null`.
//!
//! Rust-only cap (`deviates_from_ts`, Q-37): flow and block nesting [`MAX_NESTING`], counted
//! separately; over it gives `Null` (the fence's cases are dropped), as TS's stack overflow does.

use std::sync::LazyLock;

use regex::Regex;

use super::model::{CaseValue, OrderedMap};
use crate::parse::text::{compile, js_lines, js_trim, js_trim_end};

/// Deepest flow nesting, and separately deepest block nesting, before the fence is malformed.
pub(crate) const MAX_NESTING: usize = 64;

/// 2^53: above it JS integers stop being exact, so they stay floats (W4 B #10).
pub(crate) const MAX_EXACT_INT: f64 = 9_007_199_254_740_992.0;

/// Strict JSON number syntax, the only numeric form an unquoted scalar may take.
static JSON_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$"));

fn is_ws(c: char) -> bool {
    js_trim(c.encode_utf8(&mut [0; 4])).is_empty()
}

/// Index of the first char at or after `from` satisfying `f`.
fn find_from(t: &[char], from: usize, f: impl Fn(char) -> bool) -> Option<usize> {
    t.get(from..)?.iter().position(|&c| f(c)).map(|p| p + from)
}

/// `t[a..b]` as a string (empty when out of range).
fn slice_str(t: &[char], a: usize, b: usize) -> String {
    t.get(a..b).unwrap_or_default().iter().collect()
}

/// Keys that would reach through a JS prototype are never data.
pub(crate) fn is_safe_key(k: &str) -> bool {
    !matches!(k, "__proto__" | "constructor" | "prototype")
}

/// TS `typePlainScalar`. An integral float within 2^53 is `Int` (JS `Number("5.0")` is `5`).
fn type_plain(raw: &str) -> CaseValue {
    match raw {
        "true" => return CaseValue::Bool(true),
        "false" => return CaseValue::Bool(false),
        "null" | "~" => return CaseValue::Null,
        _ => {}
    }
    if !JSON_NUMBER.is_match(raw) {
        return CaseValue::Str(raw.to_owned());
    }
    match raw.parse::<f64>() {
        // 2^53: above it JS integers stop being exact, so they stay floats (W4 B #10).
        Ok(f) if f.is_finite() && f.fract() == 0.0 && f.abs() <= MAX_EXACT_INT => {
            CaseValue::Int(f as i64)
        }
        Ok(f) => CaseValue::Float(f),
        Err(_) => CaseValue::Str(raw.to_owned()),
    }
}

/// A quoted scalar starting at `t[i]`: the value and the index after the closing quote.
fn read_quoted(t: &[char], i: usize) -> Option<(String, usize)> {
    let quote = *t.get(i)?;
    let mut out = String::new();
    let mut j = i + 1;
    while let Some(&ch) = t.get(j) {
        if ch == '\\' && quote == '"' && j + 1 < t.len() {
            out.push(match t.get(j + 1).copied().unwrap_or('\\') {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                c => c,
            });
            j += 2;
        } else if ch == '\'' && quote == '\'' && t.get(j + 1) == Some(&'\'') {
            out.push('\'');
            j += 2;
        } else if ch == quote {
            return Some((out, j + 1));
        } else {
            out.push(ch);
            j += 1;
        }
    }
    None
}

/// Strips a trailing `# comment`, honouring quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut prev: Option<char> = None;
    let mut it = line.char_indices();
    while let Some((i, ch)) = it.next() {
        match quote {
            Some(q) => {
                if ch == '\\' && q == '"' {
                    it.next();
                } else if ch == q {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '#' && prev.is_none_or(is_ws) => return line.get(..i).unwrap_or(line),
            None => {}
        }
        prev = Some(ch);
    }
    line
}

/// `[a, b]` / `{k: v}` / quoted / bare readers over a char slice.
struct Flow<'a> {
    t: &'a [char],
    /// Set when [`MAX_NESTING`] was exceeded: the whole fence is then refused, not degraded.
    too_deep: bool,
}

impl Flow<'_> {
    fn skip(&self, mut k: usize, extra: &[char]) -> usize {
        while self
            .t
            .get(k)
            .is_some_and(|&c| is_ws(c) || extra.contains(&c))
        {
            k += 1;
        }
        k
    }

    fn value(&mut self, i: usize, depth: usize) -> Option<(CaseValue, usize)> {
        let k = self.skip(i, &[]);
        match *self.t.get(k)? {
            '[' => self.seq(k, depth + 1),
            '{' => self.map(k, depth + 1),
            '"' | '\'' => read_quoted(self.t, k).map(|(s, n)| (CaseValue::Str(s), n)),
            _ => {
                let end =
                    find_from(self.t, k, |c| matches!(c, ',' | ']' | '}')).unwrap_or(self.t.len());
                let raw = slice_str(self.t, k, end);
                let raw = js_trim(&raw);
                (!raw.is_empty()).then(|| (type_plain(raw), end))
            }
        }
    }

    fn deep(&mut self, depth: usize) -> bool {
        self.too_deep |= depth > MAX_NESTING;
        self.too_deep
    }

    fn seq(&mut self, i: usize, depth: usize) -> Option<(CaseValue, usize)> {
        if self.deep(depth) {
            return None;
        }
        let mut out = Vec::new();
        let mut k = i + 1;
        loop {
            k = self.skip(k, &[',']);
            match self.t.get(k)? {
                ']' => return Some((CaseValue::List(out), k + 1)),
                _ => {
                    let (v, n) = self.value(k, depth)?;
                    out.push(v);
                    k = n;
                }
            }
        }
    }

    fn key(&self, i: usize) -> Option<(String, usize)> {
        if matches!(self.t.get(i), Some('"' | '\'')) {
            return read_quoted(self.t, i);
        }
        let colon = find_from(self.t, i, |c| c == ':')?;
        let key = slice_str(self.t, i, colon);
        Some((js_trim(&key).to_owned(), colon))
    }

    fn map(&mut self, i: usize, depth: usize) -> Option<(CaseValue, usize)> {
        if self.deep(depth) {
            return None;
        }
        let mut out: Vec<(String, CaseValue)> = Vec::new();
        let mut k = i + 1;
        loop {
            k = self.skip(k, &[',']);
            if *self.t.get(k)? == '}' {
                return Some((CaseValue::Object(OrderedMap::from_pairs(out)), k + 1));
            }
            let (key, after) = self.key(k)?;
            let colon = find_from(self.t, after, |c| c == ':')?;
            let (v, n) = self.value(colon + 1, depth)?;
            if is_safe_key(&key) {
                out.push((key, v));
            }
            k = n;
        }
    }
}

/// TS `scalarOrFlow`: a flow collection or quoted string as written after `key:` / `- `, else a
/// plain scalar typed whole (never cut at a comma).
fn scalar_or_flow(text: &str, too_deep: &mut bool) -> CaseValue {
    let chars: Vec<char> = text.chars().collect();
    let start = chars.iter().take_while(|&&c| is_ws(c)).count();
    let mut flow = Flow {
        t: &chars,
        too_deep: *too_deep,
    };
    let got = match chars.get(start) {
        Some('[' | '{') => flow.value(0, 0),
        Some('"' | '\'') => read_quoted(&chars, start).map(|(s, n)| (CaseValue::Str(s), n)),
        _ => None,
    };
    *too_deep = flow.too_deep;
    got.map_or_else(|| type_plain(js_trim(text)), |(v, _)| v)
}

struct Line {
    indent: usize,
    text: String,
}

/// Non-blank, comment-stripped lines with their indent (spaces only).
fn to_lines(raw: &str) -> Vec<Line> {
    js_lines(raw)
        .filter_map(|l| {
            let text = js_trim_end(strip_comment(l));
            let trimmed = js_trim(text);
            (!trimmed.is_empty()).then(|| Line {
                indent: text.chars().take_while(|&c| c == ' ').count(),
                text: trimmed.to_owned(),
            })
        })
        .collect()
}

/// `key: rest`, or `None` when the line is not a mapping entry (`a:b` is a scalar).
fn split_key(text: &str) -> Option<(String, String)> {
    if text.starts_with(['"', '\'']) {
        let t: Vec<char> = text.chars().collect();
        let (key, next) = read_quoted(&t, 0)?;
        return (t.get(next) == Some(&':')).then(|| {
            let rest = slice_str(&t, next + 1, t.len());
            (key, js_trim(&rest).to_owned())
        });
    }
    let colon = text.find(':').filter(|&c| c > 0)?;
    let rest = text.get(colon + 1..)?;
    if rest.chars().next().is_some_and(|c| !is_ws(c)) {
        return None;
    }
    Some((
        js_trim(text.get(..colon)?).to_owned(),
        js_trim(rest).to_owned(),
    ))
}

struct Parser {
    lines: Vec<Line>,
    i: usize,
    block: usize,
    too_deep: bool,
}

impl Parser {
    fn node(&mut self, min_indent: usize) -> CaseValue {
        let Some(line) = self.lines.get(self.i) else {
            return CaseValue::Null;
        };
        if line.indent < min_indent {
            return CaseValue::Null;
        }
        let (indent, is_seq) = (line.indent, line.text == "-" || line.text.starts_with("- "));
        self.block += 1;
        if self.block > MAX_NESTING {
            self.too_deep = true;
        }
        let v = if self.too_deep {
            CaseValue::Null
        } else if is_seq {
            self.seq(indent)
        } else {
            self.map(indent)
        };
        self.block -= 1;
        v
    }

    fn seq(&mut self, indent: usize) -> CaseValue {
        let mut out = Vec::new();
        while let Some(line) = self.lines.get(self.i) {
            if line.indent != indent || !(line.text == "-" || line.text.starts_with("- ")) {
                break;
            }
            let inline = if line.text == "-" {
                ""
            } else {
                js_trim(line.text.get(2..).unwrap_or_default())
            }
            .to_owned();
            if inline.is_empty() {
                self.i += 1;
                out.push(self.node(indent + 1));
            } else if split_key(&inline).is_some() {
                // `- k: v` opens a mapping whose other keys sit at the key's own column.
                if let Some(slot) = self.lines.get_mut(self.i) {
                    *slot = Line {
                        indent: indent + 2,
                        text: inline,
                    };
                }
                out.push(self.map(indent + 2));
            } else {
                self.i += 1;
                out.push(scalar_or_flow(&inline, &mut self.too_deep));
            }
        }
        CaseValue::List(out)
    }

    fn map(&mut self, indent: usize) -> CaseValue {
        let mut out: Vec<(String, CaseValue)> = Vec::new();
        while let Some(line) = self.lines.get(self.i) {
            if line.indent != indent {
                break;
            }
            let Some((key, rest)) = split_key(&line.text) else {
                break;
            };
            self.i += 1;
            let v = if rest.is_empty() {
                self.node(indent + 1)
            } else {
                scalar_or_flow(&rest, &mut self.too_deep)
            };
            if is_safe_key(&key) {
                out.push((key, v));
            }
        }
        CaseValue::Object(OrderedMap::from_pairs(out))
    }
}

/// Parses a ` ```yaml ` case fence into the value `JSON.parse` would give. Never fails: anything
/// not understood (bad indentation, unconsumed lines, over a cap) is `CaseValue::Null`, so a bad
/// fence costs its own cases and nothing else; an empty fence is an empty list.
///
/// # Examples
///
/// ```text
/// let v = parse_yaml_cases("- input:\n    x: 1\n  expected: 2");
/// assert_eq!(serde_json::to_string(&v).unwrap(), r#"[{"input":{"x":1},"expected":2}]"#);
/// ```
pub(crate) fn parse_yaml_cases(raw: &str) -> CaseValue {
    let lines = to_lines(raw);
    let Some(first) = lines.first() else {
        return CaseValue::List(Vec::new());
    };
    let indent = first.indent;
    let mut p = Parser {
        lines,
        i: 0,
        block: 0,
        too_deep: false,
    };
    let v = p.node(indent);
    // Trailing unconsumed lines: the shape was not understood; refusing beats a truncated suite.
    if p.too_deep || p.i != p.lines.len() {
        CaseValue::Null
    } else {
        v
    }
}

#[cfg(test)]
mod tests;
