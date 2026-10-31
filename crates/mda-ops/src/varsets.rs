//! `varsets.match` / `varsets.apply` / `varsets.save` (W5b, spec §13.3): rank Variables sub-sets
//! against an artifact, apply one as a diff (nothing written), save values as a new Variables file.

use mda_core::model::{ModelBlock, ParsedArtifact, ParsedVar};
use mda_core::parse::text::js_trim;
use mda_core::registry::ArtifactType;
use mda_core::render::{Values, check_values, select_block, variable_names};
use mda_core::serialize::from_parsed;
use mda_core::variables::{SubsetRef, is_var_name, target};
use mda_core::varset::{Applied, Profile, Score, order, subsets};

use crate::artifact::{artifact_path, bad_path, load, root_of};
use crate::artifact_write::WriteResponse;
use crate::render::render_error;
use crate::vars::{create_variables_file, scan};
use crate::{Ctx, OpError, error, index_create};

/// `varsets.match` params: the artifact and an optional block index.
///
/// # Examples
///
/// ```
/// let r: mda_ops::varsets::MatchRequest = serde_json::from_value(serde_json::json!({"path": "Snippets/a.md"})).unwrap();
/// assert!(r.block.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchRequest {
    pub path: String,
    #[serde(default)]
    pub block: Option<usize>,
}

/// `varsets.apply` params: the Variables file, an optional sub-set, the client's current values.
///
/// # Examples
///
/// ```
/// let r: mda_ops::varsets::ApplyRequest = serde_json::from_value(serde_json::json!({"path": "Variables/a.md"})).unwrap();
/// assert!(r.subset.is_none() && r.values.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyRequest {
    pub path: String,
    #[serde(default)]
    pub subset: Option<SubsetRef>,
    #[serde(default)]
    pub values: Values,
}

/// `varsets.save` params: a new Variables file's title, description, tags, values and folder.
///
/// # Examples
///
/// ```
/// let r: mda_ops::varsets::SaveRequest = serde_json::from_value(serde_json::json!({"title": "Dev"})).unwrap();
/// assert!(r.dir.is_none() && r.values.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub values: Values,
    #[serde(default)]
    pub dir: Option<String>,
}

/// The ranking: the artifact's variables once, each candidate file path once, the ordered matches.
///
/// # Examples
///
/// ```
/// let r = mda_ops::varsets::MatchResponse { artifact_vars: vec![], paths: vec![], matches: vec![] };
/// assert!(serde_json::to_value(&r).unwrap().get("artifactVars").is_some());
/// ```
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResponse {
    pub artifact_vars: Vec<String>,
    pub paths: Vec<String>,
    pub matches: Vec<Match>,
}

/// One ranked sub-set: `pathIndex` into `paths`, its address, its label and its score.
///
/// # Examples
///
/// ```
/// use mda_core::{variables::SubsetRef, varset::{score, Profile}};
/// let m = mda_ops::varsets::Match { path_index: 0, subset: SubsetRef { index: 0, heading: String::new() },
///     label: "Dev".into(), score: score(&Profile::new(vec![], &[]), &[], &[]) };
/// assert!(serde_json::to_value(&m).unwrap().get("matchRatio").is_some());
/// ```
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub path_index: usize,
    pub subset: SubsetRef,
    pub label: String,
    #[serde(flatten)]
    pub score: Score,
}

/// `varsets.match`: rank every Variables sub-set against an artifact's variables and tags.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, varsets::{match_, MatchRequest}};
/// let req = MatchRequest { path: "Snippets/a.md".into(), block: None };
/// assert_eq!(match_(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn match_(ctx: &Ctx, req: MatchRequest) -> Result<MatchResponse, OpError> {
    let root = &root_of(ctx)?;
    artifact_path(&req.path)?;
    let art = load(root, &req.path)?.artifact;
    let (code, vars) = select_block(&art, req.block).map_err(render_error)?;
    let names = variable_names(code, vars);
    // Built once per call: every candidate is scored against these maps, never the artifact's lists.
    let profile = Profile::new(
        names.clone(),
        art.frontmatter.tags.as_deref().unwrap_or_default(),
    );
    let mut paths = Vec::new();
    let mut ranked: Vec<(usize, Match)> = Vec::new();
    for sc in scan(root)? {
        let Ok(p) = sc.file else { continue };
        if p.vars_error.is_some() || p.blocks.iter().any(|b| b.vars_error.is_some()) {
            continue;
        }
        let tags = p.frontmatter.tags.clone().unwrap_or_default();
        let before = ranked.len();
        for sub in subsets(&p).into_iter().filter(|s| !s.vars.is_empty()) {
            let score = mda_core::varset::score(&profile, &sub.vars, &tags);
            ranked.push((
                paths.len(),
                Match {
                    path_index: paths.len(),
                    label: label(&sub.heading, &p),
                    subset: SubsetRef {
                        index: sub.index,
                        heading: sub.heading,
                    },
                    score,
                },
            ));
        }
        if ranked.len() > before {
            paths.push(sc.path);
        }
    }
    let key = |(pi, m): &(usize, Match)| {
        let path = paths.get(*pi).map_or("", String::as_str);
        (m.score.score, path, m.subset.index)
    };
    ranked.sort_by(|a, b| order(key(a), key(b)));
    Ok(MatchResponse {
        artifact_vars: names,
        paths,
        matches: ranked.into_iter().map(|(_, m)| m).collect(),
    })
}

/// `varsets.apply`: a sub-set's values over the client's (a diff as data); nothing is written.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, varsets::{apply, ApplyRequest}};
/// let req = ApplyRequest { path: "Variables/a.md".into(), subset: None, values: Default::default() };
/// assert_eq!(apply(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn apply(ctx: &Ctx, req: ApplyRequest) -> Result<Applied, OpError> {
    let root = &root_of(ctx)?;
    if artifact_path(&req.path)? != ArtifactType::Variables {
        return Err(bad_path(&req.path));
    }
    check_values(&req.values)?;
    let p = load(root, &req.path)?.artifact;
    if p.frontmatter.artifact_type != ArtifactType::Variables {
        return Err(bad_path(&req.path));
    }
    if p.vars_error.is_some() || p.blocks.iter().any(|b| b.vars_error.is_some()) {
        return Err(OpError::new(error::ARTIFACT_VARS_INVALID).with("path", &req.path));
    }
    let m = from_parsed(&p);
    let i = target(&m, req.subset.as_ref())?;
    let vars = m
        .blocks
        .get(i)
        .map(|b| b.vars.as_slice())
        .unwrap_or_default();
    Ok(mda_core::varset::apply(&req.values, vars))
}

/// `varsets.save`: values as a new Variables file `<dir>/<slug(title)>.md`; never overwrites.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, varsets::{save, SaveRequest}};
/// let req: SaveRequest = serde_json::from_value(serde_json::json!({"title": "Dev"})).unwrap();
/// assert_eq!(save(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn save(ctx: &Ctx, req: SaveRequest) -> Result<WriteResponse, OpError> {
    let root = root_of(ctx)?;
    let title = js_trim(&req.title);
    if title.is_empty() {
        return Err(OpError::new(error::VARS_BAD_TITLE));
    }
    if let Some(name) = req.values.keys().find(|n| !is_var_name(n)) {
        return Err(OpError::new(error::VARS_BAD_NAME).with("name", name));
    }
    let mut model = index_create::model(ArtifactType::Variables, title);
    model.description = req.description;
    model.tags = req.tags;
    if !req.values.is_empty() {
        model.blocks.push(ModelBlock {
            vars: req
                .values
                .into_iter()
                .map(|(name, value)| ParsedVar { name, value })
                .collect(),
            ..ModelBlock::default()
        });
    }
    create_variables_file(&root, req.dir.as_deref(), model)
}

/// TS `subSetLabel`: the heading, else the file's title, else its file name.
fn label(heading: &str, p: &ParsedArtifact) -> String {
    if !heading.is_empty() {
        return heading.to_owned();
    }
    p.frontmatter
        .title
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| p.file_name.clone())
}
