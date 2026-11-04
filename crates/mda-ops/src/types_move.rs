//! `types.move.plan` / `types.move.apply`: move a type's `.md` files to a new location —
//! copy, verify, switch the location, delete the sources (spec §15, D-9, D-18, D-19).
//! `plan` writes nothing; `apply` re-walks the source, checks it against the plan, and only then
//! writes. Non-`.md` entries are left behind (warned); a crashed move is not resumed (D-18).

use mda_core::registry::ArtifactType;
use mda_vault::content_hash;

use crate::location::{
    LocationSpec, LocationView, check_location, persist_location, place_for, resolve,
};
use crate::migrate::FileRef;
use crate::vault::machine;
use crate::{Ctx, OpError};

mod apply;
mod shared;

/// `types.move.plan` request: the type and where it would go (`{}` = the default location).
///
/// # Examples
///
/// ```
/// let r: mda_ops::types_move::PlanRequest =
///     serde_json::from_value(serde_json::json!({"type": "Snippet", "to": {"dir": "Lib"}})).unwrap();
/// assert_eq!(r.to.dir.as_deref(), Some("Lib"));
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub to: LocationSpec,
}

/// `types.move.plan` response: the files a move would copy (virtual path + hash), the targets that
/// already exist or cannot be written (virtual paths), their total size, and the warnings.
///
/// # Examples
///
/// ```
/// let r = mda_ops::types_move::PlanResponse { files: vec![], conflicts: vec![], bytes: 0, warnings: vec![] };
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"files":[],"conflicts":[],"bytes":0,"warnings":[]}"#);
/// ```
#[derive(Debug, serde::Serialize)]
pub struct PlanResponse {
    pub files: Vec<FileRef>,
    pub conflicts: Vec<String>,
    pub bytes: u64,
    pub warnings: Vec<OpError>,
}

/// `types.move.apply` request: the plan's `files` are the contract; a changed set is refused.
///
/// # Examples
///
/// ```
/// let r: mda_ops::types_move::ApplyRequest =
///     serde_json::from_value(serde_json::json!({"type": "Snippet", "to": {}, "files": []})).unwrap();
/// assert!(r.files.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub to: LocationSpec,
    pub files: Vec<FileRef>,
}

/// `types.move.apply` response: how many files moved, the new location, the warnings.
///
/// # Examples
///
/// ```
/// use mda_ops::types_move::ApplyResponse;
/// use mda_ops::location::LocationView;
/// let r = ApplyResponse { moved: 0, location: LocationView { vault: "/v".into(), dir: None, root: Some(true) }, warnings: vec![] };
/// assert_eq!(serde_json::to_value(&r).unwrap()["moved"], 0);
/// ```
#[derive(Debug, serde::Serialize)]
pub struct ApplyResponse {
    pub moved: usize,
    pub location: LocationView,
    pub warnings: Vec<OpError>,
}

/// `types.move.plan`: list the source files and the targets in the way. Writes nothing.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, types_move::{plan, PlanRequest}};
/// let req: PlanRequest = serde_json::from_value(serde_json::json!({"type": "Snippet", "to": {}})).unwrap();
/// assert_eq!(plan(&Ctx::new(None), req).unwrap_err().code, "config.invalid");
/// ```
pub fn plan(ctx: &Ctx, req: PlanRequest) -> Result<PlanResponse, OpError> {
    let t = req.artifact_type;
    let (_, cfg) = machine(ctx)?;
    let loc = req.to.to_location(t)?;
    check_location(ctx, &cfg, t, loc.as_ref())?;
    let target = place_for(ctx, t, loc.as_ref())?;
    let (files, left) = shared::walk_source(t, &resolve(ctx, t)?)?;
    let mut conflicts = Vec::new();
    for s in &files {
        let clash = match shared::target_rel(t, &target, &s.virt) {
            Ok(rel) => shared::occupied(&target, &rel, &s.virt)?,
            Err(e) if e.code == crate::error::MOVE_BAD_PATH => true,
            Err(e) => return Err(e),
        };
        if clash {
            conflicts.push(s.virt.clone());
        }
    }
    Ok(PlanResponse {
        bytes: files.iter().map(|s| s.bytes.len() as u64).sum(),
        files: files
            .iter()
            .map(|s| FileRef {
                path: s.virt.clone(),
                hash: content_hash(&s.bytes),
            })
            .collect(),
        conflicts,
        warnings: shared::left_warning(left).into_iter().collect(),
    })
}

/// `types.move.apply`: copy → verify → switch → delete (D-9). Every check runs before the first
/// write; a failure before the switch removes only this call's copies; after it nothing is undone.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, types_move::{apply, ApplyRequest}};
/// let req: ApplyRequest = serde_json::from_value(serde_json::json!({"type": "Snippet", "to": {}, "files": []})).unwrap();
/// assert_eq!(apply(&Ctx::new(None), req).unwrap_err().code, "config.invalid");
/// ```
pub fn apply(ctx: &Ctx, req: ApplyRequest) -> Result<ApplyResponse, OpError> {
    let t = req.artifact_type;
    let (dir, cfg) = machine(ctx)?; // read once: its hash guards the switch
    let loc = req.to.to_location(t)?;
    check_location(ctx, &cfg, t, loc.as_ref())?;
    let target = place_for(ctx, t, loc.as_ref())?;
    let src = resolve(ctx, t)?; // kept: after the switch `resolve` points at the target
    apply::overlap(&src, &target)?;
    apply::bounds(t, &req.files)?;
    let (files, left) = shared::walk_source(t, &src)?;
    apply::matches_plan(&files, &req.files)?;
    let rels = files
        .iter()
        .map(|s| shared::target_rel(t, &target, &s.virt))
        .collect::<Result<Vec<_>, _>>()?;
    let made = apply::copy_verified(&target, &files, &rels)?;
    if let Err(e) = persist_location(&dir, cfg, t, loc) {
        apply::rollback(&target, &made);
        return Err(e);
    }
    let mut warnings: Vec<OpError> = shared::left_warning(left).into_iter().collect();
    warnings.extend(apply::delete_sources(&src, &files));
    Ok(ApplyResponse {
        moved: files.len(),
        location: LocationView::of(&target),
        warnings,
    })
}
