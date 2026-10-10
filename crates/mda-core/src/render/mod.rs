//! Token substitution and the directive engine (spec §4.1, §10): plain tokens, paths, `each`/`end`,
//! `join`, Choice, and the expansion limits. The single owner of rendering; tokenizes only through
//! [`crate::parse::tokens::TOKEN_PATTERN`].

mod budget;
mod eval;
mod loops;
mod resolve;
mod scan;

use std::collections::BTreeMap;

use crate::error::{LimitExceeded, VarsError};
use crate::model::{ParsedArtifact, ParsedVar};

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

/// A successful render: the text, its warnings, and whether the text holds an ESC character
/// (terminal clients refuse it, D-11).
///
/// # Examples
///
/// ```
/// use mda_core::render::Rendered;
/// let r = Rendered { output: "x".into(), warnings: vec![], contains_escape: false };
/// assert!(serde_json::to_string(&r).unwrap().contains(r#""containsEscape":false"#));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub output: String,
    pub warnings: Vec<Warning>,
    pub contains_escape: bool,
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

/// Render `code` with the defaults in `vars` and the client's `values` (spec §4.1, §10).
///
/// # Examples
///
/// ```
/// use mda_core::{model::ParsedVar, render::{Values, render}};
/// let r = render("echo <VK-a>", &[ParsedVar::text("VK-a", "x")], &Values::new()).unwrap();
/// assert!(!r.contains_escape);
/// ```
pub fn render(code: &str, vars: &[ParsedVar], values: &Values) -> Result<Rendered, LimitExceeded> {
    let table = resolve::Table::new(vars, code, values);
    let toks = scan::scan(code);
    let mut budget = budget::Budget::default();
    let mut warnings = budget::Warnings::default();
    eval::eval(code, &toks, &table, &mut budget, &mut warnings)?;
    let output = budget.out;
    Ok(Rendered {
        contains_escape: output.contains('\u{1b}'),
        output,
        warnings: warnings.into_sorted(),
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
    // Block rules: no blocks -> the top code/vars; one block -> `None` or `Some(0)` selects it.
    let (body, vars) = if p.blocks.is_empty() {
        if block.is_some() {
            return Err(RenderError::BlockNotFound(block));
        }
        (&p.code, &p.vars)
    } else {
        let i = match (block, p.blocks.len()) {
            (Some(i), _) => i,
            (None, 1) => 0,
            (None, _) => return Err(RenderError::BlockNotFound(None)),
        };
        let b = p.blocks.get(i).ok_or(RenderError::BlockNotFound(Some(i)))?;
        (&b.code, &b.vars)
    };
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
