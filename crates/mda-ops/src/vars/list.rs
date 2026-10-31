//! `vars.list` / `vars.search` (W5b): every Variables file with its shape and sub-sets, or the files
//! and parts that match a case-insensitive query (spec §13.2).

use mda_core::error::VarsError;
use mda_core::varset::{Query, Shape, Subset, file_text, filter_subsets, shape, subsets};

use super::{Scanned, scan};
use crate::artifact::{TreeError, root_of};
use crate::{Ctx, OpError};

/// `vars.list` params: none.
///
/// # Examples
///
/// ```
/// let _: mda_ops::vars::ListRequest = serde_json::from_value(serde_json::json!({})).unwrap();
/// assert!(serde_json::from_value::<mda_ops::vars::ListRequest>(serde_json::json!({"x": 1})).is_err());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListRequest {}

/// `vars.search` params: the query text.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::SearchRequest = serde_json::from_value(serde_json::json!({"query": "host"})).unwrap();
/// assert_eq!(r.query, "host");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub query: String,
}

/// The listed (or matching) Variables files, in walk order.
///
/// # Examples
///
/// ```
/// let r = mda_ops::vars::ListResponse { files: vec![] };
/// assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::json!({"files": []}));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct ListResponse {
    pub files: Vec<VarsFile>,
}

/// One file: parsed fields + shape + sub-sets, or (read failure) only `path`, `name`, `subsets: []`,
/// `error`.
///
/// # Examples
///
/// ```
/// let f = mda_ops::vars::VarsFile { path: "Variables/a.md".into(), name: "a".into(), title: None, description: None,
///     tags: None, shape: None, subsets: vec![], vars_error: None, error: None };
/// assert_eq!(serde_json::to_value(&f).unwrap(), serde_json::json!({"path": "Variables/a.md", "name": "a", "subsets": []}));
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VarsFile {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Shape>,
    pub subsets: Vec<Subset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars_error: Option<VarsError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TreeError>,
}

/// `vars.list`: every Variables file under `Variables/` (any depth), in walk order.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{list, ListRequest}};
/// assert_eq!(list(&Ctx::new(None), ListRequest {}).unwrap_err().code, "vault.not_selected");
/// ```
pub fn list(ctx: &Ctx, req: ListRequest) -> Result<ListResponse, OpError> {
    let ListRequest {} = req;
    let files = scan(&root_of(ctx)?)?.into_iter().map(entry).collect();
    Ok(ListResponse { files })
}

/// `vars.search`: the files, sub-sets and variables matching the query (empty query = `vars.list`).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, vars::{search, SearchRequest}};
/// let req = SearchRequest { query: "host".into() };
/// assert_eq!(search(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn search(ctx: &Ctx, req: SearchRequest) -> Result<ListResponse, OpError> {
    let Some(q) = Query::new(&req.query) else {
        return list(ctx, ListRequest {});
    };
    let files = scan(&root_of(ctx)?)?
        .into_iter()
        .filter_map(|s| keep(entry(s), &q))
        .collect();
    Ok(ListResponse { files })
}

/// One scanned file as a listing entry; a read failure keeps only path, stem and the error code.
fn entry(s: Scanned) -> VarsFile {
    match s.file {
        Ok(p) => VarsFile {
            shape: Some(shape(&p)),
            subsets: subsets(&p),
            path: s.path,
            name: p.file_name,
            title: p.frontmatter.title,
            description: p.frontmatter.description,
            tags: p.frontmatter.tags,
            vars_error: p.vars_error,
            error: None,
        },
        Err(e) => VarsFile {
            name: stem(&s.path),
            path: s.path,
            title: None,
            description: None,
            tags: None,
            shape: None,
            subsets: vec![],
            vars_error: None,
            error: Some(TreeError { code: e.code }),
        },
    }
}

/// Last path segment minus `.md` (the vault-relative path is already validated by the walk).
fn stem(path: &str) -> String {
    let leaf = path.rsplit('/').next().unwrap_or(path);
    leaf.strip_suffix(".md").unwrap_or(leaf).to_owned()
}

/// TS `filterVariableNodes`: a file hit keeps the whole file; else only the sub-sets/vars that hit.
/// `filter_subsets` keeps each sub-set's own `index`, so clients still address the right block.
fn keep(mut f: VarsFile, q: &Query) -> Option<VarsFile> {
    if f.error.is_some() {
        return q.hit(&f.name).then_some(f);
    }
    let label = f.title.as_deref().unwrap_or(&f.name);
    let desc = f.description.as_deref().unwrap_or("");
    if q.hit(&file_text(label, desc, f.tags.as_deref().unwrap_or(&[]))) {
        return Some(f);
    }
    f.subsets = filter_subsets(f.subsets, q);
    (!f.subsets.is_empty()).then_some(f)
}
