//! `index.plan` and `index.run` (spec §8): plan an index's steps and destination candidates, then write
//! every linked whole-file artifact into the client's workspace in one non-interactive batch.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use mda_core::model::ParsedArtifact;
use mda_core::multi_index::{RelPathError, dest_candidates, is_index, plan_links, safe_rel_path};
use mda_core::parse::{decode, parse_from_content};
use mda_core::render::{Values, Warning, check_values};
use mda_vault::{Root, contain, read_bounded};

use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, load, root_of};
use crate::error::vault_error;
use crate::write_file::{WriteFileResponse, check_whole_file, emit_whole_file};
use crate::{Ctx, OpError, error};

/// Most links one index may hold (`index.plan`), checked before any linked read.
pub const MAX_LINKS: usize = 1_000;
/// Most `paths:` entries one index may declare (`index.plan`): bounds the per-step candidate copies.
pub const MAX_PATHS: usize = 64;
/// Most steps one `index.run` (or paths one `index.create`) may carry, checked before any read.
pub const MAX_STEPS: usize = 1_000;

/// `index.plan` params: the index path and the folder the run starts from (`""` = workspace root).
///
/// # Examples
///
/// ```
/// let r: mda_ops::index::PlanRequest = serde_json::from_value(serde_json::json!({"path": "Templates/i.md"})).unwrap();
/// assert!(r.mirror_from.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRequest {
    pub path: String,
    #[serde(default)]
    pub mirror_from: String,
}

/// One planned step: the raw link, its normalised path and dir, the offered destinations, and why it
/// would be skipped (if it would).
///
/// # Examples
///
/// ```
/// let s = mda_ops::index::PlanStep { raw: "a".into(), rel_path: "a.md".into(), rel_dir: String::new(), candidates: vec![], skip: None };
/// assert!(serde_json::to_string(&s).unwrap().contains(r#""relPath":"a.md""#));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub raw: String,
    pub rel_path: String,
    pub rel_dir: String,
    pub candidates: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<OpError>,
}

/// A link target `safe_rel_path` rejected, with its `path.*` code.
///
/// # Examples
///
/// ```
/// let r = mda_ops::index::Rejected { raw: "../x".into(), code: "path.parent_segment" };
/// assert!(serde_json::to_string(&r).unwrap().contains("parent_segment"));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Rejected {
    pub raw: String,
    pub code: &'static str,
}

/// `index.plan` response.
///
/// # Examples
///
/// ```
/// let r = mda_ops::index::PlanResponse { steps: vec![], rejected: vec![] };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"steps":[],"rejected":[]}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct PlanResponse {
    pub steps: Vec<PlanStep>,
    pub rejected: Vec<Rejected>,
}

/// One `index.run` step: the linked path (as planned), the workspace-relative destination folder, and an
/// optional output file name.
///
/// # Examples
///
/// ```
/// let s: mda_ops::index::RunStep = serde_json::from_value(serde_json::json!({"relPath": "a.md", "destDir": ""})).unwrap();
/// assert!(s.file_name.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunStep {
    pub rel_path: String,
    pub dest_dir: String,
    #[serde(default)]
    pub file_name: Option<String>,
}

/// `index.run` params: the index, the client's workspace root (absolute), the steps and one values map.
///
/// # Examples
///
/// ```
/// let r: mda_ops::index::RunRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/i.md", "workspaceRoot": "/w", "steps": []})).unwrap();
/// assert!(r.values.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRequest {
    pub path: String,
    pub workspace_root: String,
    pub steps: Vec<RunStep>,
    #[serde(default)]
    pub values: Values,
}

/// One result of a batch (`index.run`, `index.create`): written, or skipped with a code.
///
/// # Examples
///
/// ```
/// let s = mda_ops::index::StepResult::skipped("a".into(), mda_ops::OpError::new("file.exists"));
/// assert!(serde_json::to_string(&s).unwrap().contains(r#""status":"skipped""#));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(
    tag = "status",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum StepResult {
    Written {
        rel_path: String,
        path: String,
        hash: String,
        warnings: Vec<Warning>,
        contains_escape: bool,
    },
    Skipped {
        rel_path: String,
        code: &'static str,
        params: BTreeMap<String, String>,
    },
}

impl StepResult {
    /// A written step, from `artifact.write_file`'s response.
    pub fn written(rel_path: String, w: WriteFileResponse) -> Self {
        Self::Written {
            rel_path,
            path: w.path,
            hash: w.hash,
            warnings: w.warnings,
            contains_escape: w.contains_escape,
        }
    }

    /// A skipped step: the error's code and params.
    pub fn skipped(rel_path: String, e: OpError) -> Self {
        Self::Skipped {
            rel_path,
            code: e.code,
            params: e.params,
        }
    }
}

/// A batch response: every step's result in order, plus the two counts.
///
/// # Examples
///
/// ```
/// use mda_ops::{OpError, index::{BatchResponse, StepResult}};
/// let b = BatchResponse::new(vec![StepResult::skipped("a".into(), OpError::new("file.exists"))]);
/// assert_eq!((b.written, b.skipped), (0, 1));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct BatchResponse {
    pub results: Vec<StepResult>,
    pub written: usize,
    pub skipped: usize,
}

impl BatchResponse {
    /// Wrap results, counting each status.
    pub fn new(results: Vec<StepResult>) -> Self {
        let written = results
            .iter()
            .filter(|r| matches!(r, StepResult::Written { .. }))
            .count();
        let skipped = results.len() - written;
        Self {
            results,
            written,
            skipped,
        }
    }
}

/// A `safe_rel_path` rejection as an op error: its `path.*` code with the raw string as `path`.
///
/// # Examples
///
/// ```
/// use mda_core::multi_index::RelPathError;
/// let e = mda_ops::index::path_error(RelPathError::Absolute, "/etc");
/// assert_eq!((e.code, e.params["path"].as_str()), ("path.absolute", "/etc"));
/// ```
pub fn path_error(e: RelPathError, raw: &str) -> OpError {
    OpError::new(e.code()).with("path", raw)
}

/// `index.limit{limit, max}`.
///
/// # Examples
///
/// ```
/// let e = mda_ops::index::limit_error("links", 1000);
/// assert_eq!((e.params["limit"].as_str(), e.params["max"].as_str()), ("links", "1000"));
/// ```
pub fn limit_error(limit: &str, max: usize) -> OpError {
    OpError::new(error::INDEX_LIMIT)
        .with("limit", limit)
        .with("max", max.to_string())
}

/// `index.plan`: an index's steps, destination candidates, skips and rejected links.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, index};
/// let req = index::PlanRequest { path: "Templates/i.md".into(), mirror_from: String::new() };
/// assert_eq!(index::plan(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn plan(ctx: &Ctx, req: PlanRequest) -> Result<PlanResponse, OpError> {
    let root = root_of(ctx)?;
    artifact_path(&req.path)?;
    // The mirrored folder is client text: reject, never sanitise.
    let mirror = if req.mirror_from.is_empty() {
        String::new()
    } else {
        safe_rel_path(&req.mirror_from).map_err(|e| path_error(e, &req.mirror_from))?
    };
    let idx = load(root, &req.path)?.artifact;
    check_index(&idx, &req.path)?;
    let declared = idx.frontmatter.paths.as_deref().unwrap_or_default();
    if declared.len() > MAX_PATHS {
        return Err(limit_error("paths", MAX_PATHS));
    }
    // Normalised and deduplicated once; every step then copies at most MAX_PATHS entries.
    let mut seen = HashSet::new();
    let paths: Vec<String> = declared
        .iter()
        .filter_map(|p| safe_rel_path(p).ok())
        .filter(|p| seen.insert(p.clone()))
        .collect();
    let lp = plan_links(&idx.code);
    if lp.steps.len() + lp.rejected.len() > MAX_LINKS {
        return Err(limit_error("links", MAX_LINKS));
    }
    let (ir, dir) = index_root(root, &req.path)?;
    let mut skips: HashMap<String, Option<OpError>> = HashMap::new();
    let steps = lp
        .steps
        .into_iter()
        .map(|s| {
            let skip = skips
                .entry(s.rel_path.clone())
                .or_insert_with(|| {
                    read_linked(root, &ir, dir, &s.rel_path)
                        .and_then(|p| check_whole_file(&p))
                        .err()
                })
                .clone();
            PlanStep {
                candidates: dest_candidates(&mirror, &s.rel_dir, &paths),
                raw: s.raw,
                rel_path: s.rel_path,
                rel_dir: s.rel_dir,
                skip,
            }
        })
        .collect();
    let rejected = lp
        .rejected
        .into_iter()
        .map(|(raw, e)| Rejected {
            raw,
            code: e.code(),
        })
        .collect();
    Ok(PlanResponse { steps, rejected })
}

/// `index.not_index` unless the parsed file is a template index.
fn check_index(p: &ParsedArtifact, path: &str) -> Result<(), OpError> {
    if is_index(&p.frontmatter) {
        Ok(())
    } else {
        Err(OpError::new(error::INDEX_NOT_INDEX).with("path", path))
    }
}

/// A `Root` on the index's own directory (🔒 R-22): a symlink inside it reaching another vault folder
/// is contained to the vault but not to here. Returns the root and the vault-relative `index_dir`.
fn index_root<'a>(root: &Root, path: &'a str) -> Result<(Root, &'a str), OpError> {
    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    let real = contain(root, Path::new(dir)).map_err(|e| vault_error(e, root))?;
    let ir = Root::new(&real).map_err(|e| vault_error(e, root))?;
    Ok((ir, dir))
}

/// Read and parse one linked file through the index-dir root; errors map against the vault root so
/// params stay vault-relative.
fn read_linked(
    root: &Root,
    ir: &Root,
    index_dir: &str,
    rel: &str,
) -> Result<ParsedArtifact, OpError> {
    let bytes =
        read_bounded(ir, Path::new(rel), MAX_ARTIFACT_BYTES).map_err(|e| vault_error(e, root))?;
    Ok(parse_from_content(
        &decode(&bytes),
        &format!("{index_dir}/{rel}"),
    ))
}

/// `index.run`: write every step's linked artifact into the workspace; never aborts after step 1.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, index};
/// let req: index::RunRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Templates/i.md", "workspaceRoot": "/w", "steps": []})).unwrap();
/// assert_eq!(index::run(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn run(ctx: &Ctx, req: RunRequest) -> Result<BatchResponse, OpError> {
    let root = root_of(ctx)?;
    artifact_path(&req.path)?;
    if !Path::new(&req.workspace_root).is_absolute() {
        return Err(
            OpError::new(error::OP_BAD_REQUEST).with("reason", "workspaceRoot must be absolute")
        );
    }
    if req.steps.len() > MAX_STEPS {
        return Err(limit_error("steps", MAX_STEPS));
    }
    check_values(&req.values)?;
    let ws = Root::new(Path::new(&req.workspace_root)).map_err(OpError::from)?;
    let idx = load(root, &req.path)?.artifact;
    check_index(&idx, &req.path)?;
    let (ir, dir) = index_root(root, &req.path)?;
    // ponytail: total work per run ~ 1 000 x (1 MiB read + <= 1 MiB render + write) ~ 3 GiB worst case -- add a run-wide byte budget if a real client needs it
    let results = req
        .steps
        .iter()
        .map(|s| match step(root, &ws, &ir, dir, s, &req.values) {
            Ok(w) => StepResult::written(s.rel_path.clone(), w),
            Err(e) => StepResult::skipped(s.rel_path.clone(), e),
        })
        .collect();
    Ok(BatchResponse::new(results))
}

/// One step; any error skips it, never the run.
fn step(
    root: &Root,
    ws: &Root,
    ir: &Root,
    dir: &str,
    s: &RunStep,
    values: &Values,
) -> Result<WriteFileResponse, OpError> {
    let rel = safe_rel_path(&s.rel_path).map_err(|e| path_error(e, &s.rel_path))?;
    let p = read_linked(root, ir, dir, &rel).map_err(|e| echo_path(e, &s.rel_path))?;
    emit_whole_file(
        ws,
        &s.dest_dir,
        &p,
        None,
        s.file_name.as_deref(),
        values,
        None,
    )
}

/// An escape error names the step's `relPath` as the client sent it, not the index-dir-relative form.
fn echo_path(mut e: OpError, sent: &str) -> OpError {
    if e.code == error::PATH_OUTSIDE_ROOT {
        e.params.insert("path".into(), sent.into());
    }
    e
}

#[cfg(test)]
mod tests;
