//! One left-to-right pass of `TOKEN_RE` into typed tokens with a running line counter (T3.1).

use crate::parse::tokens::TOKEN_RE;

/// One matched token: byte span in the scanned code, 1-based line, and what it is.
#[derive(Debug)]
pub(super) struct Tok<'a> {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub kind: Kind<'a>,
}

#[derive(Debug)]
pub(super) enum Kind<'a> {
    /// `<VK-x>`, `</VK-x>`, `<VK-x></VK-x>` (`pair`) or `<VK-x.y>`; `path` has no `VK-`.
    Var {
        path: &'a str,
        pair: bool,
    },
    Each {
        path: &'a str,
        sep: Option<String>,
    },
    End {
        path: &'a str,
    },
    Join {
        path: &'a str,
        sep: Option<String>,
    },
}

/// Tokenize `code`. The regex already proved every match well-formed, so the reads below only
/// split text; nothing here can fail. Matched text never holds `\n`, so lines are counted in the
/// gaps, once, left to right.
pub(super) fn scan(code: &str) -> Vec<Tok<'_>> {
    let ms: Vec<_> = TOKEN_RE.find_iter(code).collect();
    let mut toks = Vec::with_capacity(ms.len());
    let (mut line, mut last, mut i) = (1, 0, 0);
    while let Some(m) = ms.get(i) {
        let gap = code.get(last..m.start()).unwrap_or("");
        line += gap.bytes().filter(|b| *b == b'\n').count();
        let mut kind = kind(m.as_str());
        let mut end = m.end();
        if let Kind::Var { path, pair } = &mut kind
            && !path.contains('.')
            && let Some(next) = ms.get(i + 1)
            && next.start() == m.end()
            && closing_name(next.as_str()) == Some(path)
            && m.as_str().starts_with("<VK-")
        {
            *pair = true;
            end = next.end();
            i += 1;
        }
        toks.push(Tok {
            start: m.start(),
            end,
            line,
            kind,
        });
        last = end;
        i += 1;
    }
    toks
}

fn closing_name(text: &str) -> Option<&str> {
    text.strip_prefix("</VK-")?.strip_suffix('>')
}

fn kind(text: &str) -> Kind<'_> {
    let inner = text
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .unwrap_or("");
    if let Some(p) = inner.strip_prefix("/VK-") {
        return Kind::Var {
            path: p,
            pair: false,
        };
    }
    if let Some(r) = inner.strip_prefix("VK-each:") {
        let (path, sep) = path_sep(r);
        return Kind::Each { path, sep };
    }
    if let Some(r) = inner.strip_prefix("VK-join:") {
        let (path, sep) = path_sep(r);
        return Kind::Join { path, sep };
    }
    if let Some(r) = inner.strip_prefix("VK-end:") {
        return Kind::End { path: r };
    }
    Kind::Var {
        path: inner.strip_prefix("VK-").unwrap_or(inner),
        pair: false,
    }
}

/// `users.tags:", "` -> (`users.tags`, `, `). A path never holds `:`, so the first one starts the
/// quoted separator.
fn path_sep(rest: &str) -> (&str, Option<String>) {
    match rest.split_once(':') {
        Some((path, q)) => {
            // One quote each side only: a trailing `\"` escape must keep its quote.
            let inner = q
                .strip_prefix('"')
                .and_then(|q| q.strip_suffix('"'))
                .unwrap_or(q);
            (path, Some(unescape(inner)))
        }
        None => (rest, None),
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(e) => out.push(e), // `\\` and `\"`; the regex admits no other escape
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_pairs_and_separators() {
        let t = scan("a\r\n<VK-x></VK-x>\n<VK-each:u.v:\"\\n,\\\"\">");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].line, 2);
        assert!(matches!(
            t[0].kind,
            Kind::Var {
                path: "x",
                pair: true
            }
        ));
        assert_eq!(
            &"a\r\n<VK-x></VK-x>\n"[t[0].start..t[0].end],
            "<VK-x></VK-x>"
        );
        assert_eq!(t[1].line, 3);
        assert!(matches!(&t[1].kind, Kind::Each { path: "u.v", sep: Some(s) } if s == "\n,\""));
    }
}
