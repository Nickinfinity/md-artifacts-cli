//! The Variables-file edit ops (`vars.*`, W5): create a file, set its tags, add / rename /
//! describe / delete sub-sets, add / rename / set / delete variables. Every edit but `file.new`
//! carries the hash the client last read and goes through the one rewrite path (X-12).

use std::path::Path;

use mda_core::model::ArtifactModel;
use mda_core::naming::{slugify, validate_name};
use mda_core::parse::text::js_trim;
use mda_core::serialize::serialize;
use mda_core::variables::{self, EditError};
use mda_vault::{Root, content_hash, create_new};

use crate::artifact::{artifact_path, bad_path, root_of, segments};
use crate::artifact_write::{WriteResponse, map_serialize, max_bytes, rewrite};
use crate::write_file::exists_actual;
use crate::{ArtifactType, Ctx, OpError, error, index_create};

mod requests;
pub use requests::*;

/// The one edit path: the file must sit under `Variables/` **and** parse as a Variables file (a
/// Snippet parked there must not be rewritten as vars), then `f` edits the model and `rewrite`
/// does the hash check, serialize + guard and atomic replace.
fn edit(
    ctx: &Ctx,
    path: &str,
    hash: &str,
    f: impl FnOnce(&mut ArtifactModel) -> Result<(), EditError>,
) -> Result<WriteResponse, OpError> {
    let root = &root_of(ctx)?;
    if artifact_path(path)? != ArtifactType::Variables {
        return Err(bad_path(path));
    }
    rewrite(root, path, hash, |cur| {
        if cur.frontmatter.artifact_type != ArtifactType::Variables {
            return Err(bad_path(path));
        }
        let mut m = variables::model_of(&cur)?;
        f(&mut m)?;
        Ok(m)
    })
}

/// Create `<dir>/<slug(title)>.md` from `model`, never overwriting (`file.exists`). `dir` defaults
/// to `Variables` and must be it or under it. Shared with `varsets.save` (W5b).
pub(crate) fn create_variables_file(
    root: &Root,
    dir: Option<&str>,
    model: ArtifactModel,
) -> Result<WriteResponse, OpError> {
    let dir = dir.unwrap_or(ArtifactType::Variables.info().dir);
    let segs = segments(dir).map_err(|()| bad_path(dir))?;
    if segs.first() != Some(&ArtifactType::Variables.info().dir) {
        return Err(bad_path(dir));
    }
    let mut slug = slugify(&model.title);
    if slug.is_empty() {
        slug = "untitled".to_owned();
    }
    let name = format!("{slug}.md");
    validate_name(&name)?;
    let path = format!("{dir}/{name}");
    let text = serialize(&model, max_bytes()).map_err(|e| map_serialize(e, &path))?;
    let rel = Path::new(&path);
    create_new(root, rel, text.as_bytes()).map_err(|e| exists_actual(root, rel, e))?;
    let hash = content_hash(text.as_bytes());
    Ok(WriteResponse { path, hash })
}

/// `vars.file.new`: create a blank Variables file; never overwrites (`file.exists`).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{file_new, NewFileRequest}};
/// let req: NewFileRequest = serde_json::from_value(serde_json::json!({"title": "Dev"})).unwrap();
/// assert_eq!(file_new(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn file_new(ctx: &Ctx, req: NewFileRequest) -> Result<WriteResponse, OpError> {
    let root = root_of(ctx)?;
    let title = js_trim(&req.title);
    if title.is_empty() {
        return Err(OpError::new(error::VARS_BAD_TITLE));
    }
    let mut model = index_create::model(ArtifactType::Variables, title);
    model.description = req.description;
    model.tags = req.tags;
    create_variables_file(&root, req.dir.as_deref(), model)
}

/// `vars.meta.tags`: replace the file's tags.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{meta_tags, TagsRequest}};
/// let req: TagsRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "tags": []})).unwrap();
/// assert_eq!(meta_tags(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn meta_tags(ctx: &Ctx, req: TagsRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        m.tags = req.tags;
        Ok(())
    })
}

/// `vars.subset.add`: append an empty sub-set.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{subset_add, SubsetAddRequest}};
/// let req: SubsetAddRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "heading": "P"})).unwrap();
/// assert_eq!(subset_add(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn subset_add(ctx: &Ctx, req: SubsetAddRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::add_subset(m, &req.heading)
    })
}

/// `vars.subset.rename`: rename a sub-set.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{subset_rename, SubsetRenameRequest}};
/// let req: SubsetRenameRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "subset": {"index": 0, "heading": "A"}, "heading": "B"})).unwrap();
/// assert_eq!(subset_rename(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn subset_rename(ctx: &Ctx, req: SubsetRenameRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::rename_subset(m, &req.subset, &req.heading)
    })
}

/// `vars.subset.describe`: set or remove a sub-set's description.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{subset_describe, SubsetDescribeRequest}};
/// let req: SubsetDescribeRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "subset": {"index": 0, "heading": "A"}, "description": "d"})).unwrap();
/// assert_eq!(subset_describe(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn subset_describe(ctx: &Ctx, req: SubsetDescribeRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::describe_subset(m, &req.subset, &req.description)
    })
}

/// `vars.subset.delete`: delete a sub-set.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{subset_delete, SubsetDeleteRequest}};
/// let req: SubsetDeleteRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "subset": {"index": 0, "heading": "A"}})).unwrap();
/// assert_eq!(subset_delete(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn subset_delete(ctx: &Ctx, req: SubsetDeleteRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::delete_subset(m, &req.subset)
    })
}

/// `vars.var.add`: add a variable.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{var_add, VarAddRequest}};
/// let req: VarAddRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a"})).unwrap();
/// assert_eq!(var_add(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn var_add(ctx: &Ctx, req: VarAddRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::add_var(m, req.subset.as_ref(), &req.name, req.value)
    })
}

/// `vars.var.rename`: rename a variable, keeping its value.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{var_rename, VarRenameRequest}};
/// let req: VarRenameRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a", "newName": "VK-b"})).unwrap();
/// assert_eq!(var_rename(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn var_rename(ctx: &Ctx, req: VarRenameRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::rename_var(m, req.subset.as_ref(), &req.name, &req.new_name)
    })
}

/// `vars.var.set`: set a variable's value.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{var_set, VarSetRequest}};
/// let req: VarSetRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a", "value": "x"})).unwrap();
/// assert_eq!(var_set(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn var_set(ctx: &Ctx, req: VarSetRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::set_var(m, req.subset.as_ref(), &req.name, req.value)
    })
}

/// `vars.var.delete`: delete a variable (every var of that name).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{var_delete, VarDeleteRequest}};
/// let req: VarDeleteRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a"})).unwrap();
/// assert_eq!(var_delete(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn var_delete(ctx: &Ctx, req: VarDeleteRequest) -> Result<WriteResponse, OpError> {
    edit(ctx, &req.path, &req.expected_hash, |m| {
        variables::delete_var(m, req.subset.as_ref(), &req.name)
    })
}
