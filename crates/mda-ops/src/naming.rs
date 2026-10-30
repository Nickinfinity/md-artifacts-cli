//! `naming.check` (ER-4): the vault file-name rule as a pure query, so a client form can show the
//! default name and the refusal a write would return before anything is written (spec §14.1).
//! `naming.output` (ER-5): the output file name `artifact.write_file` would use (spec §5.1, §5.2).

use mda_core::registry::ArtifactType;

use mda_core::naming::output_name;

use crate::artifact::{artifact_path, load, root_of};
use crate::write_file::check_whole_file;
use crate::{Ctx, OpError};

/// `naming.check` params: `fileName` (a stem, without `.md`) is checked as given; absent, the default
/// stem is derived from `title`. `type` is validated and does not change the rule today.
///
/// # Examples
///
/// ```
/// let r: mda_ops::naming::CheckRequest = serde_json::from_value(serde_json::json!(
///     {"type": "Snippet", "title": "Button Group"})).unwrap();
/// assert_eq!(r.title.as_deref(), Some("Button Group"));
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
}

/// `naming.check` response: the stem checked, whether a write would accept it, and if not the
/// `{code, params}` it would return.
///
/// # Examples
///
/// ```
/// let r = mda_ops::naming::CheckResponse { file_name: "a".into(), ok: true, error: None };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"fileName":"a","ok":true}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResponse {
    pub file_name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<OpError>,
}

/// `naming.check`: no vault needed, nothing read or written.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, naming::{check, CheckRequest}};
/// let req: CheckRequest = serde_json::from_value(serde_json::json!({"type": "Snippet"})).unwrap();
/// let _ = check(&Ctx::new(None), req);
/// ```
pub fn check(_ctx: &Ctx, req: CheckRequest) -> Result<CheckResponse, OpError> {
    let file_name = match req.file_name {
        Some(f) => f,
        None => mda_core::naming::default_stem(req.title.as_deref().unwrap_or("")),
    };
    let error = mda_core::naming::validate_stem(&file_name)
        .err()
        .map(OpError::from);
    Ok(CheckResponse {
        ok: error.is_none(),
        file_name,
        error,
    })
}

/// `naming.output` params: a whole-file artifact and the name the user typed, if any.
///
/// # Examples
///
/// ```
/// let r: mda_ops::naming::OutputRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/component.md"})).unwrap();
/// assert!(r.file_name.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputRequest {
    pub path: String,
    #[serde(default)]
    pub file_name: Option<String>,
}

/// `naming.output` response: the output file name.
///
/// # Examples
///
/// ```
/// let r = mda_ops::naming::OutputResponse { file_name: "Button.tsx".into() };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"fileName":"Button.tsx"}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputResponse {
    pub file_name: String,
}

/// `naming.output`: read the artifact, nothing written; refuses exactly what `artifact.write_file` refuses
/// for the same name.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, naming::{output, OutputRequest}};
/// let req: OutputRequest = serde_json::from_value(serde_json::json!({"path": "Templates/a.md"})).unwrap();
/// assert!(output(&Ctx::new(None), req).is_err());
/// ```
pub fn output(ctx: &Ctx, req: OutputRequest) -> Result<OutputResponse, OpError> {
    let root = &root_of(ctx)?;
    artifact_path(&req.path)?;
    let p = load(root, &req.path)?.artifact;
    check_whole_file(&p)?;
    let file_name = output_name(&p, req.file_name.as_deref())?;
    Ok(OutputResponse { file_name })
}
