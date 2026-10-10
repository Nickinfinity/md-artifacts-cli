//! `artifact.read` and `artifact.tree`: parse one artifact file (with its content hash); list one
//! level of a type directory.

use std::path::Path;

use mda_core::model::ParsedArtifact;
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::{ArtifactType, type_for_dir};
use mda_vault::{Root, VaultError, content_hash, list_dir, read_bounded};

use crate::error::vault_error;
use crate::{Ctx, OpError, error};

/// Largest artifact file read (checked by `read_bounded` before reading).
pub const MAX_ARTIFACT_BYTES: u64 = 1 << 20;

/// `artifact.read` params: a vault-relative POSIX path `<TypeDir>/<rel>.md`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact::ReadRequest =
///     serde_json::from_value(serde_json::json!({"path": "Snippets/a.md"})).unwrap();
/// assert_eq!(r.path, "Snippets/a.md");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    pub path: String,
}

/// `artifact.tree` params: a type and an optional sub-directory of its type directory.
///
/// # Examples
///
/// ```
/// let r: mda_ops::artifact::TreeRequest =
///     serde_json::from_value(serde_json::json!({"type": "Snippet"})).unwrap();
/// assert!(r.dir.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeRequest {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    #[serde(default)]
    pub dir: Option<String>,
}

/// `artifact.read` response: the parsed file plus the SHA-256 `hash` of the exact bytes parsed —
/// the hash a client sends back with its next write (W-9).
///
/// # Examples
///
/// ```
/// let a = mda_core::parse::parse_from_content("", "Snippets/a.md");
/// let r = mda_ops::artifact::ReadResponse { artifact: a, hash: "h".into() };
/// let v = serde_json::to_value(&r).unwrap();
/// assert_eq!(v["hash"], "h");
/// assert_eq!(v["filePath"], "Snippets/a.md");
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct ReadResponse {
    #[serde(flatten)]
    pub artifact: ParsedArtifact,
    pub hash: String,
}

/// One level of a type directory: sub-directory names, then files.
///
/// # Examples
///
/// ```
/// let t = mda_ops::artifact::TreeResponse::default();
/// assert_eq!(serde_json::to_string(&t).unwrap(), r#"{"dirs":[],"files":[]}"#);
/// ```
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct TreeResponse {
    pub dirs: Vec<String>,
    pub files: Vec<TreeFile>,
}

/// One `.md` file in a listing; `error` set (and the parsed fields `None`) when it failed to read.
///
/// # Examples
///
/// ```
/// let f = mda_ops::artifact::TreeFile { path: "Snippets/a.md".into(), name: "a".into(),
///     title: None, description: None, tags: None, error: None };
/// assert_eq!(serde_json::to_string(&f).unwrap(), r#"{"path":"Snippets/a.md","name":"a"}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct TreeFile {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TreeError>,
}

/// Why one listed file could not be read.
///
/// # Examples
///
/// ```
/// let e = mda_ops::artifact::TreeError { code: "file.not_regular" };
/// assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"code":"file.not_regular"}"#);
/// ```
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct TreeError {
    pub code: &'static str,
}

/// String-level path rule (reject, never sanitise): every `/` segment non-empty, not `.`/`..`,
/// no `\\`, no NUL. Runs before any I/O; containment is re-checked by the vault on the real path.
pub(crate) fn segments(p: &str) -> Result<Vec<&str>, ()> {
    let segs: Vec<&str> = p.split('/').collect();
    let ok = segs
        .iter()
        .all(|s| !s.is_empty() && *s != "." && *s != ".." && !s.contains(['\\', '\0']));
    if ok { Ok(segs) } else { Err(()) }
}

pub(crate) fn bad_path(p: &str) -> OpError {
    OpError::new(error::ARTIFACT_BAD_PATH).with("path", p)
}

pub(crate) fn root_of(ctx: &Ctx) -> Result<&Root, OpError> {
    ctx.root
        .as_ref()
        .ok_or_else(|| OpError::new(error::VAULT_NOT_SELECTED))
}

/// The artifact path rule (before any I/O): `<TypeDir>/<rel>.md`, every segment valid. Returns the
/// type of the directory the path is in.
pub(crate) fn artifact_path(p: &str) -> Result<ArtifactType, OpError> {
    let segs = segments(p).map_err(|()| bad_path(p))?;
    match segs.first().and_then(|d| type_for_dir(d)) {
        Some(t) if segs.len() >= 2 && p.ends_with(".md") => Ok(t),
        _ => Err(bad_path(p)),
    }
}

fn load(root: &Root, rel: &str) -> Result<ReadResponse, OpError> {
    let bytes =
        read_bounded(root, Path::new(rel), MAX_ARTIFACT_BYTES).map_err(|e| vault_error(e, root))?;
    Ok(ReadResponse {
        artifact: parse_from_content(&decode(&bytes), rel),
        hash: content_hash(&bytes),
    })
}

/// `artifact.read`: parse one artifact file.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact};
/// let req = artifact::ReadRequest { path: "Snippets/a.md".into() };
/// assert_eq!(artifact::read(&Ctx::new(None), req).unwrap_err().code, "vault.not_selected");
/// ```
pub fn read(ctx: &Ctx, req: ReadRequest) -> Result<ReadResponse, OpError> {
    let root = root_of(ctx)?;
    artifact_path(&req.path)?;
    load(root, &req.path)
}

/// `artifact.tree`: one level of a type directory.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, artifact, ArtifactType};
/// let req = artifact::TreeRequest { artifact_type: ArtifactType::Snippet, dir: None };
/// assert!(artifact::tree(&Ctx::new(None), req).is_err());
/// ```
pub fn tree(ctx: &Ctx, req: TreeRequest) -> Result<TreeResponse, OpError> {
    let root = root_of(ctx)?;
    if let Some(d) = &req.dir {
        segments(d).map_err(|()| bad_path(d))?;
    }
    let rel = match &req.dir {
        Some(d) => format!("{}/{d}", req.artifact_type.info().dir),
        None => req.artifact_type.info().dir.to_owned(),
    };
    let listing = match list_dir(root, Path::new(&rel)) {
        Ok(l) => l,
        // A fresh vault has no type directories yet (TS parity).
        Err(VaultError::NotFound { .. }) if req.dir.is_none() => return Ok(TreeResponse::default()),
        Err(e) => return Err(vault_error(e, root)),
    };
    // ponytail: every file is parsed per call; cache only if a real vault proves slow (E-11)
    let files = listing
        .files
        .iter()
        .map(|f| tree_file(root, &rel, f))
        .collect();
    Ok(TreeResponse {
        dirs: listing.dirs,
        files,
    })
}

fn tree_file(root: &Root, rel: &str, file: &str) -> TreeFile {
    let path = format!("{rel}/{file}");
    let name = file.strip_suffix(".md").unwrap_or(file).to_owned();
    match load(root, &path).map(|r| r.artifact) {
        Ok(a) => TreeFile {
            path,
            name,
            title: a.frontmatter.title,
            description: a.frontmatter.description,
            tags: a.frontmatter.tags,
            error: None,
        },
        Err(e) => TreeFile {
            path,
            name,
            title: None,
            description: None,
            tags: None,
            error: Some(TreeError { code: e.code }),
        },
    }
}
