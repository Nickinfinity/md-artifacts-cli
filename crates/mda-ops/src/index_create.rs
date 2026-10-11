//! `index.create` (spec §8): scaffold a template index plus one empty sibling artifact per path, every
//! check done before the first write, collisions skipped (never overwritten).

use std::collections::HashSet;
use std::path::Path;

use mda_core::model::{ArtifactModel, ModelBlock};
use mda_core::multi_index::safe_rel_path;
use mda_core::naming::{MAX_NAME_BYTES, NameError, node_ext, node_stem, slugify};
use mda_core::registry::{ArtifactType, OutputNameKey};
use mda_core::serialize::serialize;
use mda_vault::{contain, content_hash, create_new};

use crate::artifact::{bad_path, root_of, segments};
use crate::artifact_write::{map_serialize, max_bytes};
use crate::error::vault_error;
use crate::index::{BatchResponse, MAX_STEPS, StepResult, limit_error, path_error};
use crate::write_file::{WriteFileResponse, exists_actual};
use crate::{Ctx, OpError, error};

/// `index.create` params: the whole-file type, the vault-relative folder (under the type's dir) and the
/// workspace-style paths each sibling stands for.
///
/// # Examples
///
/// ```
/// let r: mda_ops::index_create::CreateIndexRequest = serde_json::from_value(serde_json::json!(
///     {"type": "Template", "dir": "Templates/gen", "paths": ["a.ts"]})).unwrap();
/// assert_eq!(r.paths.len(), 1);
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateIndexRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub dir: String,
    pub paths: Vec<String>,
}

/// `index.create`: write the siblings, then the index.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, index_create};
/// let req: index_create::CreateIndexRequest = serde_json::from_value(serde_json::json!(
///     {"type": "Template", "dir": "Templates", "paths": []})).unwrap();
/// assert_eq!(index_create::create(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn create(ctx: &Ctx, req: CreateIndexRequest) -> Result<BatchResponse, OpError> {
    let root = root_of(ctx)?;
    let t = req.artifact_type;
    let info = t.info();
    if !info.writes_file {
        return Err(OpError::new(error::ARTIFACT_NOT_WHOLE_FILE).with("type", t.as_str()));
    }
    let dir_ok = segments(&req.dir).is_ok_and(|s| s.first() == Some(&info.dir));
    if !dir_ok {
        return Err(bad_path(&req.dir));
    }
    if req.paths.len() > MAX_STEPS {
        return Err(limit_error("steps", MAX_STEPS));
    }
    contain(root, Path::new(&req.dir)).map_err(|e| vault_error(e, root))?;

    // Derive, build and serialize every file before the first write: a late rejection leaves nothing.
    let mut used = HashSet::new();
    let mut files: Vec<(String, String, String)> = Vec::new(); // (result relPath, vault path, bytes)
    let mut links = Vec::new();
    for raw in &req.paths {
        let norm = safe_rel_path(raw).map_err(|e| path_error(e, raw))?;
        let link = derive_link(&norm, &mut used)?;
        let model = sibling(t, &link, &norm);
        let path = format!("{}/{link}.md", req.dir);
        let text = serialize(&model, max_bytes()).map_err(|e| map_serialize(e, &path))?;
        files.push((norm, path, text));
        links.push(link);
    }
    let path = format!("{}/{}.md", req.dir, index_name(&links));
    let text =
        serialize(&index_model(t, &links), max_bytes()).map_err(|e| map_serialize(e, &path))?;
    files.push((String::new(), path, text));

    let results = files
        .into_iter()
        .map(|(rel_path, path, text)| {
            let rel = Path::new(&path);
            match create_new(root, rel, text.as_bytes()) {
                Ok(()) => StepResult::written(
                    rel_path,
                    WriteFileResponse {
                        hash: content_hash(text.as_bytes()),
                        path,
                        warnings: vec![],
                        contains_escape: false,
                    },
                ),
                Err(e) => StepResult::skipped(rel_path, exists_actual(root, rel, e)),
            }
        })
        .collect();
    Ok(BatchResponse::new(results))
}

/// The slugged link for a normalised input path, unique within `used` (the emitted set, so
/// `a.ts, a.md, a-2.ts` give `a, a-2, a-2-2`).
fn derive_link(norm: &str, used: &mut HashSet<String>) -> Result<String, OpError> {
    let mut raw: Vec<&str> = norm.split('/').collect();
    let base = raw.pop().unwrap_or_default();
    raw.push(node_stem(base));
    let mut slugs = Vec::with_capacity(raw.len());
    for seg in raw {
        let s = slugify(seg);
        if s.is_empty() {
            return Err(OpError::new(error::INDEX_UNSLUGGABLE).with("segment", seg));
        }
        slugs.push(s);
    }
    let stem = slugs.join("/");
    let mut link = stem.clone();
    let mut n = 1;
    while used.contains(&link) {
        n += 1;
        link = format!("{stem}-{n}");
    }
    // Every dir slug is a directory name; the last (with its suffix and `.md`) is the file name.
    let file = link.rsplit('/').next().unwrap_or_default();
    let dirs_long = slugs.iter().rev().skip(1).any(|s| s.len() > MAX_NAME_BYTES);
    if dirs_long || file.len() + ".md".len() > MAX_NAME_BYTES {
        return Err(NameError::TooLong.into());
    }
    used.insert(link.clone());
    Ok(link)
}

/// An empty whole-file artifact for `norm`: title is the link's last segment, the output name is
/// carried by the type's own key (`extension` or `target`).
fn sibling(t: ArtifactType, link: &str, norm: &str) -> ArtifactModel {
    let base = norm.rsplit('/').next().unwrap_or(norm);
    let mut m = model(t, link.rsplit('/').next().unwrap_or(link));
    match t.info().output_name_key {
        Some(OutputNameKey::Extension) => m.extension = node_ext(base).to_owned(),
        Some(OutputNameKey::Target) => m.target = base.to_owned(),
        None => {}
    }
    m.blocks = vec![ModelBlock::default()];
    m
}

/// The index: `index: true`, one numbered `[[link]]` per sibling.
fn index_model(t: ArtifactType, links: &[String]) -> ArtifactModel {
    let mut m = model(t, "Index");
    m.index = true;
    let code = links
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{}. [[{l}]]", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    m.blocks = vec![ModelBlock {
        code,
        ..ModelBlock::default()
    }];
    m
}

/// `index`, else `index-2`, `index-3`… while a link has exactly that name.
fn index_name(links: &[String]) -> String {
    let mut name = "index".to_owned();
    let mut n = 1;
    while links.contains(&name) {
        n += 1;
        name = format!("index-{n}");
    }
    name
}

fn model(t: ArtifactType, title: &str) -> ArtifactModel {
    ArtifactModel {
        artifact_type: t,
        title: title.to_owned(),
        description: String::new(),
        tags: vec![],
        extension: String::new(),
        provider: String::new(),
        model: String::new(),
        version: String::new(),
        env: String::new(),
        target: String::new(),
        index: false,
        blocks: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_type_is_exact() {
        let v = serde_json::json!({"type": "template", "dir": "Templates", "paths": []});
        assert!(serde_json::from_value::<CreateIndexRequest>(v).is_err());
    }
}
