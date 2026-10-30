//! Token substitution and the directive engine (spec §4.1, §10): plain tokens, paths, `each`/`end`,
//! `join`, Choice, and the expansion limits. The single owner of rendering; tokenizes only through
//! [`crate::parse::tokens::TOKEN_PATTERN`].

mod budget;
mod eval;
mod join;
mod loops;
mod resolve;
mod scan;

use std::collections::BTreeMap;

use crate::error::{LimitExceeded, VarsError};
use crate::model::{ParsedArtifact, ParsedVar};

pub use join::{JoinError, JoinTechnique, UnjoinReason, Unjoinable, check_join_values, join_lines};

/// Largest rendered output, checked before every append.
pub const MAX_OUTPUT_BYTES: usize = 1 << 20;
/// Loop iterations per render, checked before each iteration.
pub const MAX_ITERATIONS: usize = 100_000;
/// Template nodes plus value nodes visited per render (bounds work that emits nothing).
pub const MAX_STEPS: usize = 1_000_000;
/// Open `each` loops at once, checked while matching markers (before any recursion).
pub const MAX_LOOP_DEPTH: usize = 64;

/// Client-supplied values by full variable name (`VK-x`). Shape checked by serde, semantics by
/// [`check_values`].
pub type Values = BTreeMap<String, crate::vks::VksValue>;

/// One render warning: a code from [`crate::error::RENDER_WARNING_CODES`] plus string params
/// (`line`, and `name` or `path`). Same JSON shape as an error.
///
/// # Examples
///
/// ```
/// use mda_core::render::Warning;
/// let w = Warning::new("render.unknown_var", 3).with("name", "VK-x");
/// assert_eq!(w.params["line"], "3");
/// assert_eq!(w.params["name"], "VK-x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Warning {
    pub code: &'static str,
    pub params: BTreeMap<String, String>,
}

impl Warning {
    /// A warning at 1-based `line` (sets `params.line`).
    pub fn new(code: &'static str, line: usize) -> Self {
        let mut params = BTreeMap::new();
        params.insert("line".to_owned(), line.to_string());
        Self { code, params }
    }

    /// Add one parameter (overwrites).
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

/// A successful render: the text, its warnings, whether the text holds a terminal control
/// (see [`has_terminal_control`]; terminal clients refuse it), and whether it was joined into one
/// line ([`join_lines`]; `joined` is absent from the JSON unless true).
///
/// # Examples
///
/// ```
/// use mda_core::render::Rendered;
/// let r = Rendered { output: "x".into(), warnings: vec![], contains_escape: false, joined: false };
/// let json = serde_json::to_string(&r).unwrap();
/// assert!(json.contains(r#""containsEscape":false"#) && !json.contains("joined"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub output: String,
    pub warnings: Vec<Warning>,
    pub contains_escape: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub joined: bool,
}

/// Why [`render_artifact`] produced nothing.
///
/// # Examples
///
/// ```
/// use mda_core::render::RenderError;
/// assert_eq!(RenderError::BlockNotFound(Some(2)), RenderError::BlockNotFound(Some(2)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    BlockNotFound(Option<usize>),
    Limit(LimitExceeded),
}

/// True when `s` holds a terminal control: any `char::is_control` (C0, DEL, C1) except TAB, LF and
/// a CR directly followed by LF (CRLF vault files keep their `\r`; a bare CR rewrites the line).
///
/// # Examples
///
/// ```
/// use mda_core::render::has_terminal_control;
/// assert!(has_terminal_control("a\u{9b}31m") && has_terminal_control("a\rb"));
/// assert!(!has_terminal_control("a\r\nb\t"));
/// ```
pub fn has_terminal_control(s: &str) -> bool {
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c.is_control() && c != '\t' && c != '\n' && !(c == '\r' && it.peek() == Some(&'\n')) {
            return true;
        }
    }
    false
}

/// Render `code` with the defaults in `vars` and the client's `values` (spec §4.1, §10).
///
/// # Examples
///
/// ```
/// use mda_core::{model::ParsedVar, render::{Values, render}};
/// let r = render("echo <VK-a>", &[ParsedVar::text("VK-a", "x")], &Values::new()).unwrap();
/// assert!(!r.contains_escape);
/// let c1 = Values::from([("VK-a".to_owned(), mda_core::vks::VksValue::Str("\u{9b}".into()))]);
/// assert!(render("echo <VK-a>", &[], &c1).unwrap().contains_escape); // C1 counts, not only ESC
/// ```
pub fn render(code: &str, vars: &[ParsedVar], values: &Values) -> Result<Rendered, LimitExceeded> {
    let table = resolve::Table::new(vars, code, values);
    let toks = scan::scan(code);
    let mut budget = budget::Budget::default();
    let mut warnings = budget::Warnings::default();
    eval::eval(code, &toks, &table, &mut budget, &mut warnings)?;
    let output = budget.out;
    Ok(Rendered {
        contains_escape: has_terminal_control(&output),
        output,
        warnings: warnings.into_sorted(),
        joined: false,
    })
}

/// Select the block to render (`block`, spec block-selection rules), optionally replace its code
/// with `code` (unsaved editor text), and render it.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, render::{Values, render_artifact}};
/// let p = parse_from_content("---\nartifactType: Snippet\n---\n```sh\nx\n```\n", "Snippets/a.md");
/// assert!(render_artifact(&p, None, None, &Values::new()).is_ok());
/// ```
pub fn render_artifact(
    p: &ParsedArtifact,
    block: Option<usize>,
    code: Option<&str>,
    values: &Values,
) -> Result<Rendered, RenderError> {
    let (body, vars) = select_block(p, block)?;
    render(code.unwrap_or(body), vars, values).map_err(RenderError::Limit)
}

/// Check client values with the codec's value rules (§9.7 limits, key grammar, control characters)
/// before any render work.
///
/// # Examples
///
/// ```
/// use mda_core::render::{Values, check_values};
/// assert!(check_values(&Values::new()).is_ok());
/// ```
pub fn check_values(values: &Values) -> Result<(), VarsError> {
    // ponytail: clones the map once (bounded by the request size) -- check_vars takes a slice.
    let vars: Vec<ParsedVar> = values
        .iter()
        .map(|(name, value)| ParsedVar {
            name: name.clone(),
            value: value.clone(),
        })
        .collect();
    crate::vks::check_vars(&vars)
}

/// The block a render reads (spec block rules): no blocks → the top code/vars; one block → `None` or
/// `Some(0)`; several → `Some(i)` required.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, render::select_block};
/// let p = parse_from_content("---\nartifactType: Snippet\n---\n```sh\nx\n```\n", "Snippets/a.md");
/// assert!(select_block(&p, Some(5)).is_err());
/// ```
pub fn select_block(
    p: &ParsedArtifact,
    block: Option<usize>,
) -> Result<(&str, &[ParsedVar]), RenderError> {
    // Block rules: no blocks -> the top code/vars; one block -> `None` or `Some(0)` selects it.
    if p.blocks.is_empty() {
        if block.is_some() {
            return Err(RenderError::BlockNotFound(block));
        }
        return Ok((&p.code, &p.vars));
    }
    let i = match (block, p.blocks.len()) {
        (Some(i), _) => i,
        (None, 1) => 0,
        (None, _) => return Err(RenderError::BlockNotFound(None)),
    };
    let b = p.blocks.get(i).ok_or(RenderError::BlockNotFound(Some(i)))?;
    Ok((&b.code, &b.vars))
}

/// The render variable rule (R-25(1)): declared names, then every detected root, deduped, first
/// appearance.
///
/// # Examples
///
/// ```
/// use mda_core::{model::ParsedVar, render::variable_names};
/// let n = variable_names("echo", &[ParsedVar::text("VK-a", "x")]);
/// assert_eq!(n.first().map(String::as_str), Some("VK-a"));
/// ```
pub fn variable_names(code: &str, vars: &[ParsedVar]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let detected = crate::parse::tokens::detect_vars(code);
    vars.iter()
        .chain(&detected)
        .filter(|v| seen.insert(v.name.as_str()))
        .map(|v| v.name.clone())
        .collect()
}
