//! `artifact.read` and `artifact.tree`: parse one artifact file (with its content hash); list one
//! level of a type directory.

use std::path::Path;

use mda_core::model::ParsedArtifact;
use mda_core::naming::{NameError, validate_dir_segment};
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::{ArtifactType, Format, type_for_dir};
use mda_vault::{
    DirState, Root, VaultError, contain, content_hash, dir_state, list_dir, read_bounded,
};

use crate::error::vault_error_at;
use crate::location::{Place, Resolved, physical, resolve};
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

/// D-28: every new directory segment below a resolved place's base (or the vault root) passes
/// `validate_dir_segment`. The pure check runs first (no filesystem touch); `Reserved` is always
/// refused, any other failure only when the directory does not already exist (a Finder-made NFD
/// folder keeps its files, D-8).
pub(crate) fn check_dirs_at(place: &Place, dirs: &[&str]) -> Result<(), OpError> {
    check_dirs_in(&place.root, place.base.as_deref(), dirs)
}

fn check_dirs_in(root: &Root, base: Option<&str>, dirs: &[&str]) -> Result<(), OpError> {
    let mut prefix = base.unwrap_or_default().to_string();
    for sk in dirs {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(sk);
        let Err(e) = validate_dir_segment(sk) else {
            continue;
        };
        if e == NameError::Reserved || dir_state(root, &prefix) != DirState::Present {
            return Err(OpError::from(e).with("segment", *sk));
        }
    }
    Ok(())
}

/// The segments strictly between the type dir (first) and the file (last) of `segs`.
pub(crate) fn mid_dirs<'a>(segs: &'a [&'a str]) -> &'a [&'a str] {
    segs.split_first()
        .and_then(|(_, r)| r.split_last())
        .map_or(&[], |(_, m)| m)
}

/// The session vault (owned: `select`/`use` may switch it); else the deferred default error
/// (`vault.unavailable` / `config.invalid`, V-3); else `vault.not_selected`.
pub(crate) fn root_of(ctx: &Ctx) -> Result<Root, OpError> {
    if let Some(r) = ctx.root.borrow().as_ref() {
        return Ok(r.clone());
    }
    Err(ctx
        .default_error
        .borrow()
        .clone()
        .unwrap_or_else(|| OpError::new(error::VAULT_NOT_SELECTED)))
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

/// The generic ops parse the artifact format; a type in another format (LeetCode) is refused by
/// type, pure, before any I/O. Delete, verbatim rename, tree and `naming.check` skip this.
pub(crate) fn generic_type(t: ArtifactType) -> Result<ArtifactType, OpError> {
    if t.info().format == Format::Artifact {
        Ok(t)
    } else {
        Err(OpError::new(error::ARTIFACT_UNSUPPORTED_TYPE).with("type", t.as_str()))
    }
}

/// `artifact_path` then `generic_type`: the path rule first, so a bad path stays `bad_path`.
pub(crate) fn generic_path(p: &str) -> Result<ArtifactType, OpError> {
    generic_type(artifact_path(p)?)
}

/// `locate` for the generic ops: refuses non-artifact-format types before `locate`'s I/O.
pub(crate) fn locate_generic(ctx: &Ctx, virt: &str) -> Result<Resolved, OpError> {
    generic_path(virt)?;
    locate(ctx, virt)
}

/// D-5 + W1 review (b): the artifact path rule, then the virtual → physical mapping, then at a
/// root location the canonical path may hold no `.`-component (a symlink into `.obsidian`).
pub(crate) fn locate(ctx: &Ctx, virt: &str) -> Result<Resolved, OpError> {
    artifact_path(virt)?;
    let r = physical(ctx, virt)?;
    no_dot_below_root(&r.place, &r.rel, virt)?;
    Ok(r)
}

/// At a root place, refuse a physical `rel` whose canonical form has a `.`-prefixed component.
pub(crate) fn no_dot_below_root(place: &Place, rel: &str, virt: &str) -> Result<(), OpError> {
    if place.base.is_some() {
        return Ok(());
    }
    let full =
        contain(&place.root, Path::new(rel)).map_err(|e| vault_error_at(e, &place.root, virt))?;
    let dotted = full.strip_prefix(place.root.path()).is_ok_and(|p| {
        p.components()
            .any(|c| c.as_os_str().to_str().is_none_or(|s| s.starts_with('.')))
    });
    if dotted { Err(bad_path(virt)) } else { Ok(()) }
}

/// Read and parse one artifact on a resolved location: reads the physical `rel`, parses (types)
/// and names errors by the virtual path.
pub(crate) fn load_at(r: &Resolved) -> Result<ReadResponse, OpError> {
    load_in(&r.place.root, &r.rel, &r.virt)
}

fn load_in(root: &Root, rel: &str, virt: &str) -> Result<ReadResponse, OpError> {
    let bytes = read_bounded(root, Path::new(rel), MAX_ARTIFACT_BYTES)
        .map_err(|e| vault_error_at(e, root, virt))?;
    Ok(ReadResponse {
        artifact: parse_from_content(&decode(&bytes), virt),
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
    load_at(&locate_generic(ctx, &req.path)?)
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
    let ty = req.artifact_type;
    let place = resolve(ctx, ty)?;
    let root = &place.root;
    if let Some(d) = &req.dir {
        segments(d).map_err(|()| bad_path(d))?;
        // Root location: `.obsidian` and friends are not a type's files (D-13).
        if place.base.is_none() && d.split('/').any(|s| s.starts_with('.')) {
            return Err(bad_path(d));
        }
    }
    let join = |base: Option<&str>| match (base, &req.dir) {
        (Some(b), Some(d)) => format!("{b}/{d}"),
        (Some(b), None) => b.to_owned(),
        (None, Some(d)) => d.clone(),
        (None, None) => String::new(),
    };
    let (rel, virt) = (join(place.base.as_deref()), join(Some(ty.info().dir)));
    no_dot_below_root(&place, &rel, &virt)?;
    let listing = match list_dir(root, Path::new(&rel)) {
        Ok(l) => l,
        // A fresh vault has no type directories yet (TS parity).
        Err(VaultError::NotFound { .. }) if req.dir.is_none() => return Ok(TreeResponse::default()),
        Err(e) => return Err(vault_error_at(e, root, &virt)),
    };
    let visible = |n: &String| place.base.is_some() || !n.starts_with('.');
    // ponytail: every file is parsed per call; cache only if a real vault proves slow (E-11)
    let files = listing
        .files
        .iter()
        .filter(|f| visible(f))
        .map(|f| tree_file(ty, &place, &rel, &virt, f))
        .collect();
    Ok(TreeResponse {
        dirs: listing.dirs.into_iter().filter(visible).collect(),
        files,
    })
}

fn tree_file(ty: ArtifactType, place: &Place, rel: &str, virt: &str, file: &str) -> TreeFile {
    let path = format!("{virt}/{file}");
    let name = file.strip_suffix(".md").unwrap_or(file).to_owned();
    let phys = if rel.is_empty() {
        file.to_owned()
    } else {
        format!("{rel}/{file}")
    };
    let r = Resolved {
        ty,
        place: place.clone(),
        rel: phys,
        virt: path.clone(),
    };
    match load_at(&r).map(|r| r.artifact) {
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
