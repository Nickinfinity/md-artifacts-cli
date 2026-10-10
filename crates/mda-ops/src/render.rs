//! `artifact.render`: render one artifact (or one block, or unsaved code for it) with the client's
//! values. Pure apart from the bounded read; nothing is written.

use mda_core::render::{RenderError, Rendered, Values, check_values, render_artifact};

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
/// let req = render::RenderRequest { path: "Snippets/a.md".into(), block: None, code: None, values: Default::default() };
/// assert_eq!(render::render(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn render(ctx: &Ctx, req: RenderRequest) -> Result<Rendered, OpError> {
    let root = root_of(ctx)?;
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
    render_artifact(&art, req.block, req.code.as_deref(), &req.values).map_err(render_error)
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
