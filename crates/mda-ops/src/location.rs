//! THE type-location resolver (spec §15, D-1…D-5): a virtual path `<TypeDir>/<rel>` → the vault
//! and physical vault-relative path its type's location gives. Every type-scoped op resolves here.
//!
//! Order per op (D-4, D-12, D-3, D-5): read the machine file once (`config_dir == None` = no
//! locations; corrupt = `config.invalid`), pick the type's vault (explicit, validated, else the
//! session vault), refuse a clash with any other type's effective location, pin an explicit `dir`.
//! A root place has `base: None`; a walk start built from it is exactly `""`, never `"."`.

use std::path::Path;

use mda_core::registry::{ArtifactType, TYPES, type_for_dir};
use mda_vault::{Location, Root, Where, contain, location, plain_path, validate_vault};

use crate::artifact::{bad_path, root_of, segments};
use crate::vault::read_plain;
use crate::{Ctx, OpError, error};

/// A type's resolved place: its vault and the base directory inside it (`None` = vault root).
///
/// # Examples
///
/// ```
/// let root = mda_ops::Root::new(&std::env::temp_dir()).unwrap();
/// let p = mda_ops::location::Place { root, base: None };
/// assert!(p.base.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct Place {
    pub root: Root,
    pub base: Option<String>,
}

/// One virtual path resolved: its type, place, physical vault-relative path and the virtual path
/// as the client sent it (D-15).
///
/// # Examples
///
/// ```
/// use mda_ops::{ArtifactType, location::{Place, Resolved}};
/// let root = mda_ops::Root::new(&std::env::temp_dir()).unwrap();
/// let r = Resolved { ty: ArtifactType::Snippet, place: Place { root, base: None },
///     rel: "a.md".into(), virt: "Snippets/a.md".into() };
/// assert_eq!(r.rel, "a.md");
/// ```
#[derive(Debug, Clone)]
pub struct Resolved {
    pub ty: ArtifactType,
    pub place: Place,
    pub rel: String,
    pub virt: String,
}

/// A type's effective location: its vault and where inside it.
struct Eff {
    root: Root,
    place: Where,
}

impl Eff {
    /// The directory below the vault root, `None` at a root location.
    fn dir(&self, t: ArtifactType) -> Option<&str> {
        match &self.place {
            Where::Default => Some(t.info().dir),
            Where::Dir(d) => Some(d),
            Where::Root => None,
        }
    }
}

/// `t`'s effective location from the already-read `cfg`. An explicit vault must pass
/// `validate_vault` (canonical: the stored string is untrusted, D-5); else the session vault.
fn effective(
    ctx: &Ctx,
    cfg: &mda_vault::config::MachineConfig,
    t: ArtifactType,
) -> Result<Eff, OpError> {
    let loc = location(cfg, t)?.unwrap_or(Location {
        vault: None,
        place: Where::Default,
    });
    let root = match loc.vault {
        Some(v) => validate_vault(&plain_path(Path::new(&v))).map_err(|_| {
            OpError::new(error::CONFIG_LOCATION_INVALID)
                .with("type", t.as_str())
                .with("reason", "vault")
        })?,
        None => root_of(ctx)?,
    };
    Ok(Eff {
        root,
        place: loc.place,
    })
}

/// D-3: two types may not claim one vault+dir, and a root type has its vault alone. Directories
/// compare canonically when both exist, else ASCII-lowercase (case-insensitive file systems).
fn clash(a: &Eff, ta: ArtifactType, b: &Eff, tb: ArtifactType) -> bool {
    if a.root.path() != b.root.path() {
        return false;
    }
    let (Some(da), Some(db)) = (a.dir(ta), b.dir(tb)) else {
        return true;
    };
    match (
        a.root.path().join(da).canonicalize(),
        b.root.path().join(db).canonicalize(),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => da.eq_ignore_ascii_case(db),
    }
}

/// D-5 b: after `contain`, the canonical first component below the root is the pinned dir.
fn pinned(root: &Root, dir: &str) -> bool {
    contain(root, Path::new(dir)).is_ok_and(|full| {
        full.strip_prefix(root.path())
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str().map(str::to_owned))
            .is_some_and(|c| c.eq_ignore_ascii_case(dir))
    })
}

/// The place of type `t` for this op (D-4: explicit vault > session vault).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, ArtifactType, location::resolve};
/// assert_eq!(resolve(&Ctx::new(None), ArtifactType::Snippet).unwrap_err().code, "vault.not_selected");
/// ```
pub fn resolve(ctx: &Ctx, t: ArtifactType) -> Result<Place, OpError> {
    let cfg = match &ctx.config_dir {
        Some(d) => read_plain(d)?,
        None => Default::default(),
    };
    let mine = effective(ctx, &cfg, t)?;
    for o in TYPES.iter().map(|i| i.artifact_type).filter(|o| *o != t) {
        // An unreadable other type cannot be claimed against; it fails on its own ops.
        if let Ok(theirs) = effective(ctx, &cfg, o)
            && clash(&mine, t, &theirs, o)
        {
            return Err(OpError::new(error::CONFIG_LOCATION_CONFLICT)
                .with("type", t.as_str())
                .with("other", o.as_str()));
        }
    }
    if let Where::Dir(d) = &mine.place
        && !pinned(&mine.root, d)
    {
        return Err(bad_path(t.info().dir));
    }
    let base = mine.dir(t).map(str::to_owned);
    Ok(Place {
        root: mine.root,
        base,
    })
}

/// Resolve the virtual path `virt` (first segment = a type dir). `rel` is the physical
/// vault-relative path; `virt` is echoed as sent (D-15). At a root location any `.`-segment is
/// refused (`.obsidian/`, `.mda/`, `.git/`).
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, location::physical};
/// assert_eq!(physical(&Ctx::new(None), "Snippets/a.md").unwrap_err().code, "vault.not_selected");
/// ```
pub fn physical(ctx: &Ctx, virt: &str) -> Result<Resolved, OpError> {
    let segs = segments(virt).map_err(|()| bad_path(virt))?;
    let ty = segs
        .first()
        .and_then(|d| type_for_dir(d))
        .ok_or_else(|| bad_path(virt))?;
    let place = resolve(ctx, ty).map_err(|e| {
        if e.code == error::ARTIFACT_BAD_PATH {
            bad_path(virt)
        } else {
            e
        }
    })?;
    let rel = map(ty, &place, virt)?;
    Ok(Resolved {
        ty,
        place,
        rel,
        virt: virt.to_owned(),
    })
}

/// THE virtual → physical mapping inside an already resolved `place` (one `resolve` serves many
/// paths): D-15 identity for a default layout, the configured base for a `dir`, the remaining
/// segments for a root place, which refuses any `.`-segment (D-5 a). String-level only; callers
/// still run the canonical checks before I/O.
///
/// # Examples
///
/// ```ignore
/// // crate-private; see `physical`.
/// ```
pub(crate) fn map(ty: ArtifactType, place: &Place, virt: &str) -> Result<String, OpError> {
    let segs = segments(virt).map_err(|()| bad_path(virt))?;
    let rest = segs.get(1..).unwrap_or_default();
    Ok(match place.base.as_deref() {
        Some(b) if b == ty.info().dir => virt.to_owned(),
        Some(b) => std::iter::once(b)
            .chain(rest.iter().copied())
            .collect::<Vec<_>>()
            .join("/"),
        None if rest.iter().any(|s| s.starts_with('.')) => return Err(bad_path(virt)),
        None => rest.join("/"),
    })
}
