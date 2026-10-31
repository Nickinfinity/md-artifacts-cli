//! The evaluator: walks tokens and text gaps, appending through the budget (T3.1; directives T3.3).

use super::Warning;
use super::budget::{Budget, Warnings};
use super::loops::{BlockSpan, JoinShape, block_mode, join_values, match_markers, warn};
use super::resolve::{Item, Resolved, Scope, Table};
use super::scan::{Kind, Tok};
use crate::error::{
    LimitExceeded, RENDER_EACH_NOT_LIST, RENDER_JOIN_EMPTY, RENDER_JOIN_RECORD, RENDER_UNKNOWN_VAR,
};
use crate::vks::VksList;

struct Eval<'a, 'v, 'm> {
    code: &'a str,
    toks: &'a [Tok<'a>],
    partner: Vec<Option<usize>>,
    spans: Vec<Option<BlockSpan<'a>>>,
    table: &'m Table<'v>,
    budget: &'m mut Budget,
    warnings: &'m mut Warnings,
}

/// Append `code` to `budget` with every token resolved and every `each`/`join` expanded. One
/// pass: inserted text is never scanned again. Markers are matched over the whole token list
/// first, so expansion recursion is at most one level per matched pair (<= `MAX_LOOP_DEPTH`).
pub(super) fn eval<'a>(
    code: &'a str,
    toks: &'a [Tok<'a>],
    table: &Table<'_>,
    budget: &mut Budget,
    warnings: &mut Warnings,
) -> Result<(), LimitExceeded> {
    let partner = match_markers(toks, warnings)?;
    let spans = toks
        .iter()
        .zip(&partner)
        .map(|(t, p)| match (&t.kind, p.and_then(|e| toks.get(e))) {
            (Kind::Each { .. }, Some(end)) => block_mode(code, t, end),
            _ => None,
        })
        .collect();
    let mut ev = Eval {
        code,
        toks,
        partner,
        spans,
        table,
        budget,
        warnings,
    };
    ev.range(0, toks.len(), 0, code.len(), &mut Vec::new())
}

impl<'a, 'v> Eval<'a, 'v, '_> {
    /// Tokens `from..to` with the text between `pos` and `stop`.
    fn range(
        &mut self,
        from: usize,
        to: usize,
        mut pos: usize,
        stop: usize,
        scopes: &mut Vec<Scope<'a, 'v>>,
    ) -> Result<(), LimitExceeded> {
        let mut i = from;
        while let Some(t) = self.toks.get(i).filter(|_| i < to) {
            if let Kind::Each { path, sep } = &t.kind
                && let Some(end) = self.partner.get(i).copied().flatten()
                && let Some(list) = self.loop_list(t, path, scopes)
            {
                let span = self.spans.get(i).copied().flatten();
                self.text(pos, span.map_or(t.start, |s| s.cut_from))?;
                self.budget.step()?;
                let end_tok = self.toks.get(end);
                pos = span.map_or(end_tok.map_or(t.end, |e| e.end), |s| s.after);
                let sep = sep.as_deref().unwrap_or("");
                self.run_loop(
                    Loop {
                        each: i,
                        end,
                        path,
                        sep,
                        list,
                        span,
                    },
                    scopes,
                )?;
                i = end + 1;
                continue;
            }
            self.text(pos, t.start)?;
            self.budget.step()?;
            self.token(t, scopes)?;
            pos = t.end;
            i += 1;
        }
        self.text(pos, stop)
    }

    /// The list an `each` iterates, or `None` (token stays literal, warning added).
    fn loop_list(
        &mut self,
        t: &Tok<'_>,
        path: &str,
        scopes: &[Scope<'a, 'v>],
    ) -> Option<&'v VksList> {
        let code = match self.table.node(scopes, path) {
            Ok((Item::List(l), _)) => return Some(l),
            Ok(_) => RENDER_EACH_NOT_LIST,
            Err(c) => c,
        };
        warn(self.warnings, code, t, path);
        None
    }

    fn run_loop(
        &mut self,
        l: Loop<'a, 'v, '_>,
        scopes: &mut Vec<Scope<'a, 'v>>,
    ) -> Result<(), LimitExceeded> {
        let (Some(each), Some(end)) = (self.toks.get(l.each), self.toks.get(l.end)) else {
            return Ok(());
        };
        let (from, to, nl) = match l.span {
            Some(s) => (s.body_from, s.body_to, s.nl),
            None => (each.end, end.start, ""),
        };
        let n = Item::list_len(l.list);
        for k in 0..n {
            self.budget.iteration()?;
            if k > 0 {
                self.budget.push(l.sep)?;
                self.budget.push(nl)?;
            }
            let Some(item) = Item::list_item(l.list, k) else {
                break;
            };
            scopes.push((l.path.split('.').collect(), item));
            let r = self.range(l.each + 1, l.end, from, to, scopes);
            scopes.pop();
            r?;
        }
        if n > 0 {
            self.budget.push(nl)?;
        }
        Ok(())
    }

    fn token(&mut self, t: &Tok<'a>, scopes: &[Scope<'a, 'v>]) -> Result<(), LimitExceeded> {
        let literal = self.code.get(t.start..t.end).unwrap_or("");
        match &t.kind {
            Kind::Var { path, .. } => match self.table.resolve(scopes, path) {
                Resolved::Text(s) => self.budget.push(s),
                Resolved::Literal(wcode) => {
                    let key = if path.contains('.') { "path" } else { "name" };
                    let w = Warning::new(wcode, t.line).with(key, format!("VK-{path}"));
                    self.warnings.add(w);
                    self.budget.push(literal)
                }
            },
            Kind::Join { path, sep } => self.join(t, path, sep.as_deref(), scopes),
            // A marker that reaches here failed to pair or to loop; it stays as written.
            Kind::Each { .. } | Kind::End { .. } => self.budget.push(literal),
        }
    }

    fn join(
        &mut self,
        t: &Tok<'a>,
        path: &str,
        sep: Option<&str>,
        scopes: &[Scope<'a, 'v>],
    ) -> Result<(), LimitExceeded> {
        let literal = self.code.get(t.start..t.end).unwrap_or("");
        let mut found = Vec::new();
        let failure = match self.table.locate(scopes, path) {
            None => Some(RENDER_UNKNOWN_VAR),
            Some((item, segs, _)) => match join_values(item, &segs, self.budget, &mut found)? {
                JoinShape::Record => Some(RENDER_JOIN_RECORD),
                JoinShape::Strings if found.is_empty() => Some(RENDER_JOIN_EMPTY),
                JoinShape::Strings => None,
            },
        };
        if let Some(code) = failure {
            warn(self.warnings, code, t, path);
            return self.budget.push(literal);
        }
        for (k, s) in found.iter().enumerate() {
            if k > 0 {
                self.budget.push(sep.unwrap_or(", "))?;
            }
            self.budget.push(s)?;
        }
        Ok(())
    }

    fn text(&mut self, from: usize, to: usize) -> Result<(), LimitExceeded> {
        let gap = self.code.get(from..to).unwrap_or("");
        if gap.is_empty() {
            return Ok(());
        }
        self.budget.step()?;
        self.budget.push(gap)
    }
}

/// One matched, resolved `each` ready to expand.
struct Loop<'a, 'v, 'm> {
    each: usize,
    end: usize,
    path: &'a str,
    sep: &'m str,
    list: &'v VksList,
    span: Option<BlockSpan<'a>>,
}
