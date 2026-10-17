//! `vault.validate` (W6, spec §11.3): every artifact file in the six type dirs, read and checked;
//! errors and warnings per file. Writes nothing.

use std::collections::BTreeMap;

use mda_core::migrate::{Skip, fenceless_sections, frontmatter, vks};
use mda_core::parse::{decode, file_fence_block, is_flagged, parse_from_content};
use mda_core::registry::ArtifactType;
use mda_core::variables::{EditError, model_of};

use crate::error::{MIGRATE_PENDING, VARS_FENCELESS_SECTION, VARS_MIXED_FILE, vault_error};
use crate::{Ctx, OpError};

/// Most issues returned; counts stay exact past it.
pub const MAX_ISSUES: usize = 10_000;

/// `vault.validate` params: none.
///
/// # Examples
///
/// ```
/// let _: mda_ops::validate::ValidateRequest = serde_json::from_value(serde_json::json!({})).unwrap();
/// ```
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateRequest {}

/// How bad an issue is: errors make `mda vault validate` exit 1.
///
/// # Examples
///
/// ```
/// assert_eq!(serde_json::to_value(mda_ops::validate::Severity::Warning).unwrap(), "warning");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One finding in one file.
///
/// # Examples
///
/// ```
/// use mda_ops::validate::{Issue, Severity};
/// let i = Issue { path: "a".into(), code: "vars.mixed_file", params: Default::default(), severity: Severity::Warning };
/// assert_eq!(serde_json::to_value(&i).unwrap()["severity"], "warning");
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Issue {
    pub path: String,
    pub code: &'static str,
    pub params: BTreeMap<String, String>,
    pub severity: Severity,
}

/// The report: files visited, exact counts, and the first [`MAX_ISSUES`] issues.
///
/// # Examples
///
/// ```
/// let r = mda_ops::validate::ValidateResponse { files: 0, errors: 0, warnings: 0, truncated: false, issues: vec![] };
/// assert_eq!(serde_json::to_value(&r).unwrap()["truncated"], false);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct ValidateResponse {
    pub files: usize,
    pub errors: usize,
    pub warnings: usize,
    pub truncated: bool,
    pub issues: Vec<Issue>,
}

/// `vault.validate`: read and check every artifact file.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// assert!(mda_ops::validate::validate(&ctx, Default::default()).is_err());
/// ```
pub fn validate(ctx: &Ctx, req: ValidateRequest) -> Result<ValidateResponse, OpError> {
    let ValidateRequest {} = req;
    let root = crate::artifact::root_of(ctx)?;
    let mut out = ValidateResponse {
        files: 0,
        errors: 0,
        warnings: 0,
        truncated: false,
        issues: Vec::new(),
    };
    crate::migrate::scope(&root, |path, bytes| {
        out.files += 1;
        let found = match bytes {
            Ok(b) => issues_of(path, &b),
            Err(e) => {
                let e = vault_error(e, &root);
                vec![issue(path, e.code, e.params, Severity::Error)]
            }
        };
        for i in found {
            push(&mut out, i);
        }
    })?;
    Ok(out)
}

fn issue(
    path: &str,
    code: &'static str,
    params: BTreeMap<String, String>,
    severity: Severity,
) -> Issue {
    Issue {
        path: path.to_owned(),
        code,
        params,
        severity,
    }
}

fn warn(path: &str, code: &'static str, k: &str, v: &str) -> Issue {
    let params = if k.is_empty() {
        BTreeMap::new()
    } else {
        BTreeMap::from([(k.to_owned(), v.to_owned())])
    };
    issue(path, code, params, Severity::Warning)
}

/// Count every issue; keep only the first [`MAX_ISSUES`] (a hostile file can make millions).
fn push(out: &mut ValidateResponse, i: Issue) {
    match i.severity {
        Severity::Error => out.errors += 1,
        Severity::Warning => out.warnings += 1,
    }
    if out.issues.len() < MAX_ISSUES {
        out.issues.push(i);
    } else {
        out.truncated = true;
    }
}

/// The findings for one file, in the order of spec §11.3: variable errors, pending migrations,
/// Variables-file warnings. Pending is the migrations' own answer, never a second detector.
fn issues_of(path: &str, bytes: &[u8]) -> Vec<Issue> {
    let content = decode(bytes);
    let p = parse_from_content(&content, path);
    let mut v = Vec::new();
    if let Some(e) = &p.vars_error {
        v.push(issue(path, e.code, e.params.clone(), Severity::Error));
    }
    // The file-level error is block `file_block`'s own; skip that block by position, not by value
    // (two sections can fail identically).
    let file_block = file_fence_block(&content);
    for (n, b) in p.blocks.iter().enumerate() {
        if let Some(e) = b.vars_error.as_ref().filter(|_| Some(n) != file_block) {
            let mut params = e.params.clone();
            params.insert("block".to_owned(), n.to_string());
            v.push(issue(path, e.code, params, Severity::Error));
        }
    }
    if matches!(frontmatter::rewrite(&content), Ok(Some(_))) {
        v.push(warn(path, MIGRATE_PENDING, "kind", "frontmatter"));
    }
    match vks::rewrite(&content, path) {
        Ok(Some(_)) => v.push(warn(path, MIGRATE_PENDING, "kind", "vks")),
        Err(Skip::UnfencedVars) => {
            v.push(warn(path, mda_core::error::MIGRATE_UNFENCED_VARS, "", ""));
        }
        _ => {}
    }
    if p.frontmatter.artifact_type == ArtifactType::Variables && !is_flagged(&content) {
        for h in fenceless_sections(&content) {
            v.push(warn(path, VARS_FENCELESS_SECTION, "heading", &h));
        }
        if model_of(&p) == Err(EditError::Mixed) {
            v.push(warn(path, VARS_MIXED_FILE, "", ""));
        }
    }
    v
}
