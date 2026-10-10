//! The strict YAML reader for `vks` bodies (spec §9.1–§9.4, §9.7). Designed by T1.1.
//!
//! Line-based recursive descent. Recursion is bounded by [`MAX_DEPTH`], checked when a container
//! is entered; every other limit is checked before the push it bounds. No regex, no `&s[a..b]`.

use super::legacy::{control_error, control_line};
use super::{
    MAX_DEPTH, MAX_LIST_ITEMS, MAX_MAP_KEYS, MAX_NODES, VksList, VksRecord, VksValue, limit_error,
};
use crate::error::{
    Construct, Limit, SyntaxRule, VKS_BAD_KEY, VKS_DUPLICATE_KEY, VKS_INDENT, VKS_INLINE_COMMENT,
    VKS_MIXED_DIALECT, VKS_MIXED_LIST, VKS_SYNTAX, VKS_UNSUPPORTED, VarsError,
};
use crate::model::ParsedVar;
use crate::parse::text::{js_lines, js_trim};

type R<T> = Result<T, VarsError>;

fn syntax(rule: SyntaxRule, line: usize) -> VarsError {
    VarsError::new(VKS_SYNTAX, line).with("rule", rule.as_str())
}

fn unsupported(c: Construct, line: usize) -> VarsError {
    VarsError::new(VKS_UNSUPPORTED, line).with("construct", c.as_str())
}

fn indent_err(line: usize) -> VarsError {
    VarsError::new(VKS_INDENT, line)
}

/// One body line. `text` is what follows the indentation, trailing spaces removed.
struct Line<'a> {
    no: usize,
    raw: &'a str,
    indent: usize,
    tab_indent: bool,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn new(i: usize, raw: &'a str) -> Self {
        let text = raw.trim_start_matches([' ', '\t']);
        let lead = raw.get(..raw.len() - text.len()).unwrap_or("");
        Self {
            no: i + 1,
            raw,
            indent: lead.matches(' ').count(),
            tab_indent: lead.contains('\t'),
            text: text.trim_end_matches(' '),
        }
    }

    fn skippable(&self) -> bool {
        js_trim(self.raw).is_empty() || self.text.starts_with('#')
    }
}

/// An unsupported construct a value may start with.
fn value_construct(s: &str) -> Option<Construct> {
    Some(match s.chars().next()? {
        '{' => Construct::FlowMap,
        '[' => Construct::FlowList,
        '&' => Construct::Anchor,
        '*' => Construct::Alias,
        '!' => Construct::Tag,
        '>' => Construct::Folded,
        '|' => Construct::BlockIndicator,
        _ => return None,
    })
}

/// Checked before the key grammar, so `<<: *x` is never a `bad_key`.
fn line_construct(s: &str) -> Option<Construct> {
    let marker = |m: &str| s == m || s.starts_with(&format!("{m} "));
    if s.starts_with("<<") {
        Some(Construct::MergeKey)
    } else if s.starts_with('%') {
        Some(Construct::Directive)
    } else if marker("?") {
        Some(Construct::ComplexKey)
    } else if marker("---") || marker("...") {
        Some(Construct::DocumentMarker)
    } else {
        value_construct(s)
    }
}

/// Split `key: rest` at the first `:` followed by a space or the end; `rest` starts after the `:`.
fn split_key(text: &str) -> Option<(&str, &str)> {
    text.match_indices(':').find_map(|(i, _)| {
        let after = text.get(i + 1..)?;
        (after.is_empty() || after.starts_with(' ')).then(|| (text.get(..i).unwrap_or(""), after))
    })
}

fn check_key(key: &str, top: bool, line: usize) -> R<()> {
    let mut cs = key.chars();
    let ok = cs.next().is_some_and(|c| c.is_ascii_alphabetic())
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || (top && c == '-'));
    if ok {
        Ok(())
    } else {
        Err(VarsError::new(VKS_BAD_KEY, line).with("key", key))
    }
}

/// A line that is not an entry, item or comment.
fn bad_line(text: &str, line: usize) -> VarsError {
    let mixed = text
        .find('=')
        .and_then(|i| text.get(..i))
        .is_some_and(|p| !p.contains(':'));
    if mixed {
        VarsError::new(VKS_MIXED_DIALECT, line)
    } else {
        syntax(SyntaxRule::ExpectedEntry, line)
    }
}

/// A quoted scalar, spec §9.1: `''` / `\\ \" \n \t` only; ` #…` after the close is a comment error.
fn quoted(v: &str, line: usize) -> R<String> {
    let q = v.chars().next().unwrap_or('"');
    let mut out = String::new();
    let mut it = v.char_indices().skip(1).peekable();
    while let Some((i, c)) = it.next() {
        match c {
            c if c == q && q == '\'' && it.peek().is_some_and(|&(_, n)| n == '\'') => {
                it.next();
                out.push('\'');
            }
            c if c == q => return tail(v.get(i + 1..).unwrap_or(""), line).map(|()| out),
            '\\' if q == '"' => match it.next() {
                Some((_, '\\')) => out.push('\\'),
                Some((_, '"')) => out.push('"'),
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some(_) => return Err(syntax(SyntaxRule::BadEscape, line)),
                None => break,
            },
            c => out.push(c),
        }
    }
    Err(syntax(SyntaxRule::UnclosedQuote, line))
}

fn tail(rest: &str, line: usize) -> R<()> {
    let t = rest.trim_start_matches(' ');
    if t.is_empty() {
        Ok(())
    } else if t.starts_with('#') && rest.starts_with(' ') {
        Err(VarsError::new(VKS_INLINE_COMMENT, line))
    } else {
        Err(syntax(SyntaxRule::ExpectedEntry, line))
    }
}

/// A scalar already known not to be `|`/`|-`; `None` is the empty list `[]`.
fn scalar(v: &str, line: usize) -> R<Option<String>> {
    if v == "[]" {
        return Ok(None);
    }
    if let Some(c) = value_construct(v) {
        return Err(unsupported(c, line));
    }
    if v.starts_with(['\'', '"']) {
        return quoted(v, line).map(Some);
    }
    if v.starts_with('#') || v.contains(" #") {
        return Err(VarsError::new(VKS_INLINE_COMMENT, line));
    }
    if v.starts_with([']', '}', '%', '@', '`', ',', '?'])
        || v.starts_with("- ")
        || v.contains(": ")
        || v.ends_with(':')
    {
        return Err(syntax(SyntaxRule::BadPlain, line));
    }
    Ok(Some(v.to_owned()))
}

fn is_dash(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

/// Classify a dash line: the text after the dash, and whether it opens a compact record.
/// Everything the item grammar refuses is refused here, before the line is rewritten or read.
fn item_kind(text: &str, no: usize) -> R<(&str, bool)> {
    let after = text.get(2..).unwrap_or("");
    if text == "-" || after.starts_with(' ') {
        return Err(syntax(SyntaxRule::ExpectedEntry, no));
    }
    if is_dash(after) {
        return Err(unsupported(Construct::NestedList, no));
    }
    if after.starts_with('|') {
        return Err(unsupported(Construct::BlockInList, no));
    }
    if after.starts_with('#') {
        // Would be re-read as a comment line and its content silently dropped.
        return Err(VarsError::new(VKS_INLINE_COMMENT, no));
    }
    if let Some(c) = line_construct(after) {
        return Err(unsupported(c, no));
    }
    Ok((
        after,
        !after.starts_with(['\'', '"']) && split_key(after).is_some(),
    ))
}

struct Parser<'a> {
    lines: Vec<Line<'a>>,
    pos: usize,
    nodes: usize,
}

impl<'a> Parser<'a> {
    /// Advance to the next content line and return its validated indent.
    fn peek(&mut self) -> R<Option<usize>> {
        while self.lines.get(self.pos).is_some_and(Line::skippable) {
            self.pos += 1;
        }
        let Some(l) = self.lines.get(self.pos) else {
            return Ok(None);
        };
        if l.tab_indent || l.indent % 2 != 0 {
            return Err(indent_err(l.no));
        }
        Ok(Some(l.indent))
    }

    fn cur(&self) -> (usize, &'a str) {
        self.lines.get(self.pos).map_or((0, ""), |l| (l.no, l.text))
    }

    /// Count one `VksValue` node, before it exists.
    fn node(&mut self, line: usize) -> R<()> {
        if self.nodes >= MAX_NODES {
            return Err(limit_error(Limit::Nodes, MAX_NODES, line));
        }
        self.nodes += 1;
        Ok(())
    }

    fn str_node(&mut self, s: String, line: usize) -> R<VksValue> {
        self.node(line)?;
        Ok(VksValue::Str(s))
    }

    /// After a scalar at indent `n`, a deeper line is a second value for the same key.
    fn no_children(&mut self, n: usize) -> R<()> {
        match self.peek()? {
            Some(ind) if ind > n => Err(syntax(SyntaxRule::ValueAndChildren, self.cur().0)),
            _ => Ok(()),
        }
    }

    /// `map(n)`: entries of one map at exactly indent `n`; `d` is the map's container depth.
    fn map(&mut self, n: usize, d: usize) -> R<VksRecord> {
        let mut rec: Vec<(String, VksValue)> = Vec::new();
        while let Some(ind) = self.peek()? {
            let (no, text) = self.cur();
            if ind < n {
                break;
            }
            if ind > n {
                return Err(indent_err(no));
            }
            if text == "-" {
                return Err(syntax(SyntaxRule::ExpectedEntry, no));
            }
            if text.starts_with("- ") {
                return Err(if n == 0 {
                    syntax(SyntaxRule::DashAtColumn0, no)
                } else {
                    indent_err(no)
                });
            }
            if let Some(c) = line_construct(text) {
                return Err(unsupported(c, no));
            }
            let (key, rest) = split_key(text).ok_or_else(|| bad_line(text, no))?;
            check_key(key, n == 0 && d == 0, no)?;
            if rec.iter().any(|(k, _)| k == key) {
                return Err(VarsError::new(VKS_DUPLICATE_KEY, no).with("key", key));
            }
            if rec.len() >= MAX_MAP_KEYS {
                return Err(limit_error(Limit::MapKeys, MAX_MAP_KEYS, no));
            }
            let value = self.value(n, d, rest, no)?;
            rec.push((key.to_owned(), value));
        }
        Ok(VksRecord(rec))
    }

    /// The value of the entry on the current line: inline, block scalar, or children.
    fn value(&mut self, n: usize, d: usize, rest: &str, no: usize) -> R<VksValue> {
        let v = rest.trim_start_matches(' ');
        self.pos += 1;
        if v.is_empty() {
            return match self.peek()? {
                Some(ind) if ind > n && ind == n + 2 => self.child(n + 2, d + 1),
                Some(ind) if ind > n => Err(indent_err(self.cur().0)),
                _ => self.str_node(String::new(), no),
            };
        }
        if v == "|" || v == "|-" {
            return self.block(n, v == "|", no);
        }
        let out = match scalar(v, no)? {
            Some(s) => self.str_node(s, no)?,
            None => {
                self.node(no)?;
                VksValue::List(VksList::Strings(Vec::new()))
            }
        };
        self.no_children(n)?;
        Ok(out)
    }

    /// Enter a container at indent `n`, depth `d`: a sequence when it starts with a dash.
    fn child(&mut self, n: usize, d: usize) -> R<VksValue> {
        let (no, text) = self.cur();
        if d > MAX_DEPTH {
            return Err(limit_error(Limit::Depth, MAX_DEPTH, no));
        }
        self.node(no)?;
        if is_dash(text) {
            Ok(VksValue::List(self.seq(n, d)?))
        } else {
            Ok(VksValue::Record(self.map(n, d)?))
        }
    }

    fn seq(&mut self, n: usize, d: usize) -> R<VksList> {
        let (mut strs, mut recs): (Vec<String>, Vec<VksRecord>) = (Vec::new(), Vec::new());
        while let Some(ind) = self.peek()? {
            let (no, text) = self.cur();
            if ind < n {
                break;
            }
            if ind > n {
                return Err(indent_err(no));
            }
            if !is_dash(text) {
                return Err(bad_line(text, no));
            }
            if strs.len() + recs.len() >= MAX_LIST_ITEMS {
                return Err(limit_error(Limit::ListItems, MAX_LIST_ITEMS, no));
            }
            let (after, is_rec) = item_kind(text, no)?;
            if (is_rec && !strs.is_empty()) || (!is_rec && !recs.is_empty()) {
                return Err(VarsError::new(VKS_MIXED_LIST, no));
            }
            if is_rec {
                if d + 1 > MAX_DEPTH {
                    return Err(limit_error(Limit::Depth, MAX_DEPTH, no));
                }
                self.node(no)?;
                // The compact record's first field sits on the dash line: re-read it two deeper.
                if let Some(l) = self.lines.get_mut(self.pos) {
                    l.indent = n + 2;
                    l.text = after;
                }
                recs.push(self.map(n + 2, d + 1)?);
            } else {
                let s = scalar(after, no)?.ok_or_else(|| unsupported(Construct::FlowList, no))?;
                self.node(no)?;
                self.pos += 1;
                self.no_children(n)?;
                strs.push(s);
            }
        }
        Ok(if recs.is_empty() {
            VksList::Strings(strs)
        } else {
            VksList::Records(recs)
        })
    }

    /// `|` / `|-` block scalar for a key at indent `n`: lines indented ≥ n+2 or blank, `#` included.
    fn block(&mut self, n: usize, keep: bool, no: usize) -> R<VksValue> {
        let want = n + 2;
        let mut out: Vec<&str> = Vec::new();
        while let Some(l) = self.lines.get(self.pos) {
            if js_trim(l.raw).is_empty() {
                out.push("");
            } else if l.raw.len() - l.raw.trim_start_matches(' ').len() >= want {
                out.push(l.raw.get(want..).unwrap_or(""));
            } else {
                break;
            }
            self.pos += 1;
        }
        while out.last().is_some_and(|s| s.is_empty()) {
            out.pop();
        }
        let mut s = out.join("\n");
        if keep && !s.is_empty() {
            s.push('\n');
        }
        self.str_node(s, no)
    }
}

/// Read a body classified as YAML. The first violation in line order wins, control characters
/// included: they are located first, then compared with the parse error's line.
pub(crate) fn read_yaml(body: &str) -> R<Vec<ParsedVar>> {
    let lines = js_lines(body)
        .enumerate()
        .map(|(i, l)| Line::new(i, l))
        .collect();
    let mut p = Parser {
        lines,
        pos: 0,
        nodes: 0,
    };
    let parsed = p.map(0, 0);
    match (parsed, control_line(body)) {
        (Err(e), Some(c))
            if c <= e
                .params
                .get("line")
                .and_then(|l| l.parse().ok())
                .unwrap_or(0) =>
        {
            Err(control_error(c))
        }
        (Ok(_), Some(c)) => Err(control_error(c)),
        (Err(e), _) => Err(e),
        (Ok(rec), None) => Ok(rec
            .0
            .into_iter()
            .map(|(name, value)| ParsedVar { name, value })
            .collect()),
    }
}
