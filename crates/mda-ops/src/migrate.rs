//! `migrate.frontmatter.plan|apply` (W6, spec §11; the vks migration was removed in W7e): plans write
//! nothing; apply re-runs the rewrite on fresh, hash-checked bytes and writes per file.

use std::collections::BTreeMap;
use std::path::Path;

use mda_core::migrate::{Normalise, Skip, frontmatter, normalises};
use mda_core::parse::decode;
use mda_core::registry::{ArtifactType, TYPES};
use mda_vault::{
    MAX_WALK_BYTES, Root, VaultError, WalkLimit, content_hash, file_len, read_bounded, replace,
    walk_md,
};

use crate::artifact::{MAX_ARTIFACT_BYTES, artifact_path, root_of};
use crate::error::{
    FILE_CONFLICT, FILE_TOO_LARGE, MIGRATE_UP_TO_DATE, OP_BAD_REQUEST, vault_error,
};
use crate::{Ctx, OpError};

/// Most entries one apply takes.
pub const MAX_APPLY_FILES: usize = 10_000;

/// Plan params: none (the scope is the whole vault, spec §11).
///
/// # Examples
///
/// ```
/// let _: mda_ops::migrate::PlanRequest = serde_json::from_value(serde_json::json!({})).unwrap();
/// ```
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {}

/// One file to apply: its vault-relative path and the hash the plan read.
///
/// # Examples
///
/// ```
/// let f: mda_ops::migrate::FileRef = serde_json::from_value(serde_json::json!({"path": "Snippets/a.md", "hash": "h"})).unwrap();
/// assert_eq!(f.path, "Snippets/a.md");
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub path: String,
    pub hash: String,
}

/// Apply params: the files to rewrite (≤ [`MAX_APPLY_FILES`]).
///
/// # Examples
///
/// ```
/// let r: mda_ops::migrate::ApplyRequest = serde_json::from_value(serde_json::json!({"files": []})).unwrap();
/// assert!(r.files.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    pub files: Vec<FileRef>,
}

/// A planned frontmatter rewrite.
///
/// # Examples
///
/// ```
/// use mda_ops::{ArtifactType, migrate::FrontmatterChange};
/// let c = FrontmatterChange { path: "Snippets/a.md".into(), hash: "h".into(), old_line: "type: snippet".into(),
///     new_line: "artifactType: Snippet".into(), artifact_type: ArtifactType::Snippet, normalises: vec![] };
/// assert!(serde_json::to_value(&c).unwrap().get("normalises").is_none());
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterChange {
    pub path: String,
    pub hash: String,
    pub old_line: String,
    pub new_line: String,
    pub artifact_type: ArtifactType,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub normalises: Vec<Normalise>,
}

/// A file a plan leaves untouched, with the reason.
///
/// # Examples
///
/// ```
/// let s = mda_ops::migrate::Skipped { path: "a".into(), code: "migrate.leading_blank", params: Default::default() };
/// assert_eq!(serde_json::to_value(&s).unwrap()["code"], "migrate.leading_blank");
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Skipped {
    pub path: String,
    pub code: &'static str,
    pub params: BTreeMap<String, String>,
}

/// A plan: the files that would change and the files left with a reason, in walk order.
///
/// # Examples
///
/// ```
/// let p: mda_ops::migrate::PlanResponse<mda_ops::migrate::FrontmatterChange> = mda_ops::migrate::PlanResponse { changes: vec![], skipped: vec![] };
/// assert_eq!(serde_json::to_value(&p).unwrap(), serde_json::json!({"changes": [], "skipped": []}));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct PlanResponse<C> {
    pub changes: Vec<C>,
    pub skipped: Vec<Skipped>,
}

/// One apply entry's outcome (the `index.run` step shape).
///
/// # Examples
///
/// ```
/// let r = mda_ops::migrate::ApplyResult::Written { path: "a".into(), hash: "h".into() };
/// assert_eq!(serde_json::to_value(&r).unwrap()["status"], "written");
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(
    tag = "status",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum ApplyResult {
    Written {
        path: String,
        hash: String,
    },
    Skipped {
        path: String,
        code: &'static str,
        params: BTreeMap<String, String>,
    },
}

/// An apply: one result per request entry, in request order, plus the counts.
///
/// # Examples
///
/// ```
/// let r = mda_ops::migrate::ApplyResponse { results: vec![], written: 0, skipped: 0 };
/// assert_eq!(serde_json::to_value(&r).unwrap()["written"], 0);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct ApplyResponse {
    pub results: Vec<ApplyResult>,
    pub written: usize,
    pub skipped: usize,
}

/// Walk the six type dirs (spec §11) with one shared budget; per-file failures reach `visit`, caps
/// refuse the call (`walk.limit`).
pub(crate) fn scope(
    root: &Root,
    visit: impl FnMut(&str, Result<Vec<u8>, VaultError>),
) -> Result<(), OpError> {
    walk_md(
        root,
        &TYPES.each_ref().map(|t| t.dir),
        MAX_ARTIFACT_BYTES,
        visit,
    )
    .map_err(|e| vault_error(e, root))
}

impl From<Skip> for OpError {
    fn from(s: Skip) -> Self {
        Self {
            code: s.code(),
            params: s.params(),
        }
    }
}

/// Plan core (M-7): walk the six type dirs, run `make` on each file's bytes, collect changes and
/// skips in walk order. Writes nothing.
fn plan_all<C>(
    ctx: &Ctx,
    make: impl Fn(&str, &[u8]) -> Result<Option<C>, Skip>,
) -> Result<PlanResponse<C>, OpError> {
    let root = root_of(ctx)?;
    let (mut changes, mut skipped) = (Vec::new(), Vec::new());
    scope(&root, |path, read| {
        let err = match read {
            Ok(bytes) => match make(path, &bytes) {
                Ok(Some(c)) => return changes.push(c),
                Ok(None) => return,
                Err(s) => OpError::from(s),
            },
            Err(e) => vault_error(e, &root),
        };
        skipped.push(Skipped {
            path: path.to_owned(),
            code: err.code,
            params: err.params,
        });
    })?;
    Ok(PlanResponse { changes, skipped })
}

/// The per-call read budget (`walk.limit{bytes}`), separate from the per-file cap.
struct Budget {
    used: u64,
    exhausted: bool,
}

type Rewrite<'a> = &'a dyn Fn(&str, &str) -> Result<Option<String>, Skip>;

fn skip(path: &str, e: OpError) -> ApplyResult {
    ApplyResult::Skipped {
        path: path.to_owned(),
        code: e.code,
        params: e.params,
    }
}

fn apply_one(root: &Root, f: &FileRef, budget: &mut Budget, rewrite: Rewrite) -> ApplyResult {
    match apply_inner(root, f, budget, rewrite) {
        Ok(hash) => ApplyResult::Written {
            path: f.path.clone(),
            hash,
        },
        Err(e) => skip(&f.path, e),
    }
}

fn apply_inner(
    root: &Root,
    f: &FileRef,
    budget: &mut Budget,
    rewrite: Rewrite,
) -> Result<String, OpError> {
    let rel = Path::new(&f.path);
    let ve = |e| vault_error(e, root);
    let limit = || {
        ve(VaultError::Limit {
            limit: WalkLimit::Bytes,
            max: MAX_WALK_BYTES,
        })
    };
    if budget.exhausted {
        return Err(limit());
    }
    let len = file_len(root, rel).map_err(ve)?;
    if budget.used.saturating_add(len) > MAX_WALK_BYTES {
        budget.exhausted = true;
        return Err(limit());
    }
    budget.used += len;
    let bytes = read_bounded(root, rel, MAX_ARTIFACT_BYTES).map_err(ve)?;
    // A file that grew since `file_len` counts its real size.
    budget.used = budget
        .used
        .saturating_add((bytes.len() as u64).saturating_sub(len));
    let actual = content_hash(&bytes);
    if actual != f.hash {
        return Err(OpError::new(FILE_CONFLICT)
            .with("path", &f.path)
            .with("expected", &f.hash)
            .with("actual", &actual));
    }
    let out = rewrite(&f.path, &decode(&bytes))?
        .ok_or_else(|| OpError::new(MIGRATE_UP_TO_DATE))?
        .into_bytes();
    if out.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(OpError::new(FILE_TOO_LARGE)
            .with("path", &f.path)
            .with("size", out.len().to_string())
            .with("max", MAX_ARTIFACT_BYTES.to_string()));
    }
    replace(root, rel, &out, &f.hash, MAX_ARTIFACT_BYTES).map_err(ve)?;
    Ok(content_hash(&out))
}

/// Apply core (M-9, M-10): cap and path-check the whole request before any I/O, then one
/// independent, hash-checked, budgeted entry at a time.
fn apply_all(ctx: &Ctx, req: ApplyRequest, rewrite: Rewrite) -> Result<ApplyResponse, OpError> {
    let root = root_of(ctx)?;
    if req.files.len() > MAX_APPLY_FILES {
        return Err(OpError::new(OP_BAD_REQUEST).with("reason", "at most 10000 files"));
    }
    for f in &req.files {
        artifact_path(&f.path)?;
    }
    let mut budget = Budget {
        used: 0,
        exhausted: false,
    };
    let results: Vec<ApplyResult> = req
        .files
        .iter()
        .map(|f| apply_one(&root, f, &mut budget, rewrite))
        .collect();
    let written = results
        .iter()
        .filter(|r| matches!(r, ApplyResult::Written { .. }))
        .count();
    let skipped = results.len() - written;
    Ok(ApplyResponse {
        results,
        written,
        skipped,
    })
}

fn fm_change(path: &str, bytes: &[u8]) -> Result<Option<FrontmatterChange>, Skip> {
    Ok(
        frontmatter::rewrite(&decode(bytes))?.map(|r| FrontmatterChange {
            path: path.to_owned(),
            hash: content_hash(bytes),
            old_line: r.old_line,
            new_line: r.new_line,
            artifact_type: r.artifact_type,
            normalises: normalises(bytes),
        }),
    )
}

/// `migrate.frontmatter.plan`: every legacy `type:` line the vault would lose.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// assert!(mda_ops::migrate::frontmatter_plan(&ctx, Default::default()).is_err());
/// ```
pub fn frontmatter_plan(
    ctx: &Ctx,
    req: PlanRequest,
) -> Result<PlanResponse<FrontmatterChange>, OpError> {
    let PlanRequest {} = req;
    plan_all(ctx, fm_change)
}

/// `migrate.frontmatter.apply`: rewrite the listed files (hash-checked, per file).
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// let req = mda_ops::migrate::ApplyRequest { files: vec![] };
/// assert!(mda_ops::migrate::frontmatter_apply(&ctx, req).is_err());
/// ```
pub fn frontmatter_apply(ctx: &Ctx, req: ApplyRequest) -> Result<ApplyResponse, OpError> {
    apply_all(ctx, req, &|_, c| {
        Ok(frontmatter::rewrite(c)?.map(|r| r.content))
    })
}
