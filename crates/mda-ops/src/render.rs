//! `artifact.render`: render one artifact (or one block, or unsaved code for it) with the client's
//! values. Pure apart from the bounded read; nothing is written.

use mda_core::render::{
    JoinError, JoinTechnique, RenderError, Rendered, Unjoinable, Values, check_join_values,
    check_values, has_terminal_control, join_lines, select_block,
};

use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, load, root_of};
use crate::{Ctx, OpError, error};

/// `artifact.render` params: the artifact path, an optional block index, optional unsaved code for
/// that block, and the client's values by full variable name.
///
/// # Examples
///
/// ```
/// let r: mda_ops::render::RenderRequest =
///     serde_json::from_value(serde_json::json!({"path": "Snippets/a.md"})).unwrap();
/// assert!(r.block.is_none() && r.values.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderRequest {
    pub path: String,
    #[serde(default)]
    pub block: Option<usize>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub values: Values,
    /// Join the rendered lines into one command line (`render.unjoinable` when not provably safe).
    #[serde(default)]
    pub join: Option<JoinTechnique>,
}

/// Map a core render refusal to its code: `artifact.block_not_found{block}` (`""` when no block was
/// named) or `render.limit{limit, max}`.
///
/// # Examples
///
/// ```
/// use mda_core::error::{LimitExceeded, RenderLimit};
/// use mda_core::render::RenderError;
/// let e = mda_ops::render::render_error(RenderError::Limit(LimitExceeded { limit: RenderLimit::Steps, max: 9 }));
/// assert_eq!((e.code, e.params["limit"].as_str(), e.params["max"].as_str()), ("render.limit", "steps", "9"));
/// let e = mda_ops::render::render_error(RenderError::BlockNotFound(None));
/// assert_eq!(e.params["block"], "");
/// ```
pub fn render_error(e: RenderError) -> OpError {
    match e {
        RenderError::BlockNotFound(i) => OpError::new(error::ARTIFACT_BLOCK_NOT_FOUND)
            .with("block", i.map(|i| i.to_string()).unwrap_or_default()),
        RenderError::Limit(l) => OpError::new(error::RENDER_LIMIT)
            .with("limit", l.limit.as_str())
            .with("max", l.max.to_string()),
    }
}

/// `artifact.render`: render one artifact with the client's values.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, render};
/// let req = render::RenderRequest { path: "Snippets/a.md".into(), block: None, code: None, values: Default::default(), join: None };
/// assert_eq!(render::render(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn render(ctx: &Ctx, req: RenderRequest) -> Result<Rendered, OpError> {
    let root = &root_of(ctx)?;
    artifact_path(&req.path)?;
    // Before any I/O: `render` collects every token match before its step budget applies, so an
    // oversize `code` must be refused here, and bad values must not cost a 1 MiB read.
    if let Some(c) = &req.code
        && c.len() as u64 > MAX_ARTIFACT_BYTES
    {
        return Err(OpError::new(error::FILE_TOO_LARGE)
            .with("path", &req.path)
            .with("size", c.len().to_string())
            .with("max", MAX_ARTIFACT_BYTES.to_string()));
    }
    check_values(&req.values)?;
    let art = load(root, &req.path)?.artifact;
    let (body, vars) = select_block(&art, req.block).map_err(render_error)?;
    let code = req.code.as_deref().unwrap_or(body);
    if req.join.is_some() {
        check_join_values(code, vars, &req.values).map_err(unjoinable_error)?;
    }
    let r = mda_core::render::render(code, vars, &req.values)
        .map_err(|l| render_error(RenderError::Limit(l)))?;
    finish(r, req.join)
}

/// `render.unjoinable{line, reason, name?}` from the scanner's refusal.
///
/// # Examples
///
/// ```
/// use mda_core::render::{UnjoinReason, Unjoinable};
/// let e = mda_ops::render::unjoinable_error(Unjoinable { line: 2, reason: UnjoinReason::Comment, name: None });
/// assert_eq!((e.code, e.params["line"].as_str(), e.params["reason"].as_str()), ("render.unjoinable", "2", "comment"));
/// ```
pub fn unjoinable_error(u: Unjoinable) -> OpError {
    let e = OpError::new(error::RENDER_UNJOINABLE)
        .with("line", u.line.to_string())
        .with("reason", u.reason.as_str());
    match u.name {
        Some(n) => e.with("name", n),
        None => e,
    }
}

/// Apply `join` (when sent) to a finished render: the one place `joined` becomes true, and only
/// after every kept line passed the scanner.
fn finish(mut r: Rendered, join: Option<JoinTechnique>) -> Result<Rendered, OpError> {
    let Some(t) = join else { return Ok(r) };
    r.output = join_lines(&r.output, t).map_err(|e| match e {
        JoinError::Unjoinable(u) => unjoinable_error(u),
        JoinError::Limit(l) => render_error(RenderError::Limit(l)),
    })?;
    r.contains_escape = has_terminal_control(&r.output);
    r.joined = true;
    Ok(r)
}

/// `render.text` params: free text and the client's values (no artifact, no vault).
///
/// # Examples
///
/// ```
/// let r: mda_ops::render::TextRequest = serde_json::from_value(serde_json::json!({"text": "<VK-a>"})).unwrap();
/// assert!(r.values.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextRequest {
    pub text: String,
    #[serde(default)]
    pub values: Values,
    /// As on `artifact.render`.
    #[serde(default)]
    pub join: Option<JoinTechnique>,
}

/// `render.text`: fill `<VK-…>` tokens in `text` (X-7) — the `artifact.render` response, every render
/// limit, no vault.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, render};
/// let req = render::TextRequest { text: "plain".into(), values: Default::default(), join: None };
/// assert!(!render::text(&Ctx::new(None), req).unwrap().contains_escape);
/// ```
pub fn text(ctx: &Ctx, req: TextRequest) -> Result<Rendered, OpError> {
    // Pure: no vault, so no `root_of`. Same gates and order as `artifact.render`.
    let _ = ctx;
    if req.text.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(OpError::new(error::FILE_TOO_LARGE)
            .with("path", "")
            .with("size", req.text.len().to_string())
            .with("max", MAX_ARTIFACT_BYTES.to_string()));
    }
    check_values(&req.values)?;
    if req.join.is_some() {
        check_join_values(&req.text, &[], &req.values).map_err(unjoinable_error)?;
    }
    let r = mda_core::render::render(&req.text, &[], &req.values)
        .map_err(|l| render_error(RenderError::Limit(l)))?;
    finish(r, req.join)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_request_rejects_unknown_field() {
        let r = serde_json::from_value::<RenderRequest>(serde_json::json!({"path": "a", "x": 1}));
        assert!(r.is_err());
    }

    #[test]
    fn values_reject_number() {
        let r = serde_json::from_value::<RenderRequest>(
            serde_json::json!({"path": "a", "values": {"VK-a": 1}}),
        );
        assert!(r.is_err());
    }
}
