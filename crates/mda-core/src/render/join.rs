//! Joining a rendered Command into one line (spec §4.3, ER-7): the techniques, the line scanner that
//! refuses what it cannot join safely, and the multi-line value check. Never guesses: a line the
//! scanner cannot prove joinable is refused with a reason.

use crate::error::LimitExceeded;
use crate::model::ParsedVar;
use crate::parse::text::{js_lines, js_trim};
use crate::parse::tokens::detect_vars;
use crate::varset::leaves;

use super::Values;
use super::budget::Budget;
use super::resolve::Table;

/// How lines are joined: `and` ` && `, `seq` ` ; `, `continuation` ` \` + LF, `pwsh` ` ; `, `cmd` ` & `.
///
/// # Examples
///
/// ```
/// use mda_core::render::JoinTechnique;
/// let t: JoinTechnique = serde_json::from_str(r#""continuation""#).unwrap();
/// assert_eq!(t, JoinTechnique::Continuation);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinTechnique {
    And,
    Seq,
    Continuation,
    Pwsh,
    Cmd,
}

/// Why a line cannot be joined (the `reason` param of `render.unjoinable`).
///
/// # Examples
///
/// ```
/// use mda_core::render::UnjoinReason;
/// assert_eq!(UnjoinReason::TrailingOperator.as_str(), "trailing_operator");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnjoinReason {
    Heredoc,
    OpenQuote,
    Comment,
    TrailingOperator,
    BlockKeyword,
    MultilineValue,
}

impl UnjoinReason {
    /// The protocol spelling (snake_case).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Heredoc => "heredoc",
            Self::OpenQuote => "open_quote",
            Self::Comment => "comment",
            Self::TrailingOperator => "trailing_operator",
            Self::BlockKeyword => "block_keyword",
            Self::MultilineValue => "multiline_value",
        }
    }
}

/// A refused join: the 1-based rendered-output line (`0` for a multi-line value), the reason, and
/// the variable for `MultilineValue`.
///
/// # Examples
///
/// ```
/// use mda_core::render::{UnjoinReason, Unjoinable};
/// let u = Unjoinable { line: 2, reason: UnjoinReason::Comment, name: None };
/// assert_eq!(u.reason.as_str(), "comment");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unjoinable {
    pub line: usize,
    pub reason: UnjoinReason,
    pub name: Option<String>,
}

/// Why [`join_lines`] produced nothing.
///
/// # Examples
///
/// ```
/// use mda_core::render::{JoinError, UnjoinReason, Unjoinable};
/// let e = JoinError::Unjoinable(Unjoinable { line: 1, reason: UnjoinReason::Heredoc, name: None });
/// assert!(matches!(e, JoinError::Unjoinable(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinError {
    Unjoinable(Unjoinable),
    Limit(LimitExceeded),
}

/// Join the non-blank, trimmed lines of a rendered `output` with technique `t`, refusing any line
/// the scanner cannot prove joinable.
///
/// # Examples
///
/// ```
/// use mda_core::render::{JoinTechnique, join_lines};
/// assert_eq!(join_lines("a\n\n b \nc", JoinTechnique::And).unwrap(), "a && b && c");
/// ```
pub fn join_lines(output: &str, t: JoinTechnique) -> Result<String, JoinError> {
    let sep = match t {
        JoinTechnique::And => " && ",
        JoinTechnique::Seq | JoinTechnique::Pwsh => " ; ",
        JoinTechnique::Continuation => " \\\n",
        JoinTechnique::Cmd => " & ",
    };
    let mut buf = Budget::default();
    let mut first = true;
    for (i, raw) in js_lines(output).enumerate() {
        let line = js_trim(raw);
        if line.is_empty() {
            continue;
        }
        if let Some(reason) = scan_line(line) {
            return Err(JoinError::Unjoinable(Unjoinable {
                line: i + 1,
                reason,
                name: None,
            }));
        }
        if !first {
            buf.push(sep).map_err(JoinError::Limit)?;
        }
        buf.push(line).map_err(JoinError::Limit)?;
        first = false;
    }
    Ok(buf.out)
}

/// Words that open or close a shell/PowerShell/cmd block (matched case-insensitively).
const BLOCK_WORDS: [&str; 17] = [
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
    "select", "function", "foreach", "try", "catch",
];
const BLOCK_WORDS_2: [&str; 2] = ["finally", "switch"];

/// The first reason `line` (trimmed, non-empty) cannot be joined, in spec order.
// ponytail: a line scanner, not a shell parser; over-refuses pwsh backtick escapes, cmd `'`, `'it''s'`, bash `\"`; cmd quotes are per line (odd `"` count, `^` escape) — parse per shell if a client needs them joined. — parse per shell if a client needs them joined.
fn scan_line(line: &str) -> Option<UnjoinReason> {
    let (heredoc, comment, open) = scan_quotes(line);
    let open = open || cmd_quote_open(line);
    if heredoc || line.ends_with("@'") || line.ends_with("@\"") {
        return Some(UnjoinReason::Heredoc);
    }
    if comment || line.starts_with(':') || rem_at(line) {
        return Some(UnjoinReason::Comment);
    }
    if open {
        return Some(UnjoinReason::OpenQuote);
    }
    if line.ends_with(['\\', '`', '^', '&', '|', ';']) {
        return Some(UnjoinReason::TrailingOperator);
    }
    let run: String = line
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let braces = ['{', '}', '(', ')'];
    let word = BLOCK_WORDS.contains(&run.as_str()) || BLOCK_WORDS_2.contains(&run.as_str());
    (word || line.starts_with(braces) || line.ends_with(braces))
        .then_some(UnjoinReason::BlockKeyword)
}

/// `rem` as a command word: the line's first word, or right after `&`, `&&`, `|`, `||`, `(`.
fn is_rem(s: &str) -> bool {
    let w = s.split_whitespace().next().unwrap_or("");
    // cmd also ends the word at `.`, `/` and `:` (`rem.x`, `rem/x`, `rem:x` are comments).
    w.get(..3).is_some_and(|h| h.eq_ignore_ascii_case("rem"))
        && w.get(3..)
            .is_some_and(|r| r.is_empty() || r.starts_with(['.', '/', ':']))
}

fn rem_at(line: &str) -> bool {
    if is_rem(line) {
        return true;
    }
    line.char_indices()
        .filter(|(_, c)| matches!(c, '&' | '|' | '('))
        .any(|(i, _)| {
            let rest = line.get(i..).unwrap_or("");
            is_rem(rest.trim_start_matches(['&', '|', '(']).trim_start())
        })
}

/// One pass over `line` with POSIX quote rules: `(unquoted <<, unquoted comment, ends in a quote)`.
/// Stops at the first unquoted comment.
fn scan_quotes(line: &str) -> (bool, bool, bool) {
    let mut q: Option<char> = None;
    let mut prev: Option<char> = None;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match (q, c) {
            (Some('\''), '\'') | (Some('"'), '"') | (Some('`'), '`') => q = None,
            (Some('\''), _) => {}
            (Some(_), '\\') => {
                it.next();
            }
            (Some(_), _) => {}
            (None, '\\') => {
                it.next();
            }
            (None, '\'' | '"' | '`') => q = Some(c),
            (None, '<') if it.peek() == Some(&'<') => return (true, false, false),
            (None, '#') if prev.is_none_or(|p| p.is_whitespace() || ";&|(){}<>".contains(p)) => {
                return (false, true, false);
            }
            (None, _) => {}
        }
        prev = Some(c);
    }
    (false, false, q.is_some())
}

/// cmd's view of quotes: every `"` toggles, `^` escapes the next char outside a quote, `\` and `'`
/// mean nothing. cmd quotes end at the line, so an odd count would swallow the next joined line.
fn cmd_quote_open(line: &str) -> bool {
    let (mut open, mut it) = (false, line.chars());
    while let Some(c) = it.next() {
        match c {
            '"' => open = !open,
            '^' if !open => {
                it.next();
            }
            _ => {}
        }
    }
    open
}

/// Refuse, before rendering, a used variable whose effective value holds a line feed anywhere
/// (`MultilineValue`, line `0`): joining it would silently make two commands.
///
/// # Examples
///
/// ```
/// use mda_core::model::ParsedVar;
/// use mda_core::render::{Values, check_join_values};
/// let vars = [ParsedVar::text("VK-a", "x\ny")];
/// assert!(check_join_values("echo <VK-b>", &vars, &Values::new()).is_ok());
/// assert!(check_join_values("echo <VK-a>", &vars, &Values::new()).is_err());
/// ```
pub fn check_join_values(
    code: &str,
    vars: &[ParsedVar],
    values: &Values,
) -> Result<(), Unjoinable> {
    let table = Table::new(vars, code, values);
    // Used = a root named in `code` (declared-only vars are never rendered); first-appearance order.
    for name in detect_vars(code).into_iter().map(|v| v.name) {
        let Some(root) = table.root(&name) else {
            continue;
        };
        let mut found = Vec::new();
        leaves(root, &mut found);
        if found.iter().any(|s| s.contains('\n')) {
            return Err(Unjoinable {
                line: 0,
                reason: UnjoinReason::MultilineValue,
                name: Some(name),
            });
        }
    }
    Ok(())
}
