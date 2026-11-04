//! The location write side (W2, spec §15): the wire shapes of a location, the checks a
//! hypothetical location must pass before anything is written, and the one machine-file write.
//! Shared by `types.set_location`, `types.list` and `types.move.*`.

use std::path::Path;

use mda_core::registry::ArtifactType;
use mda_vault::config::{MachineConfig, write_machine};
use mda_vault::{Location, Where, set_location};

use super::{DEFAULT, Place, conflicts, eff_of, finish};
use crate::error::{self, shown};
use crate::{Ctx, OpError};

/// A location as clients see it (D-7): the absolute vault, then `dir` or `root: true`.
///
/// # Examples
///
/// ```
/// use mda_ops::location::{LocationView, Place};
/// let root = mda_ops::Root::new(&std::env::temp_dir()).unwrap();
/// let v = LocationView::of(&Place { root, base: None });
/// assert_eq!(v.root, Some(true));
/// assert!(v.dir.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LocationView {
    pub vault: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<bool>,
}

impl LocationView {
    /// The view of a resolved place (`vault` in plain form, no `\\?\`).
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_ops::location::{LocationView, Place};
    /// let root = mda_ops::Root::new(&std::env::temp_dir()).unwrap();
    /// let v = LocationView::of(&Place { root, base: Some("Lib".into()) });
    /// assert_eq!(v.dir.as_deref(), Some("Lib"));
    /// ```
    pub fn of(place: &Place) -> Self {
        Self {
            vault: shown(place.root.path()),
            dir: place.base.clone(),
            root: place.base.is_none().then_some(true),
        }
    }
}

/// A requested location (`types.set_location` fields, `types.move.*` `to`). `{}` resets to the
/// default; `vault` absent = the session vault (D-4, D-32).
///
/// # Examples
///
/// ```
/// use mda_ops::{ArtifactType, location::LocationSpec};
/// let s = LocationSpec { vault: None, dir: None, root: None };
/// assert_eq!(s.to_location(ArtifactType::Snippet).unwrap(), None);
/// ```
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationSpec {
    pub vault: Option<String>,
    pub dir: Option<String>,
    pub root: Option<bool>,
}

impl LocationSpec {
    /// The stored shape: `{}` → `None` · `{vault}` → `Where::Default` · `dir` → `Dir` (pinned, even
    /// when it equals the registry dir) · `root: true` → `Root`. `dir` + `root: true` is
    /// `config.location_invalid{reason:"both"}`; `root: false` without `dir` is `neither`. Value
    /// checks (absolute vault, dir segment) are `check_location`'s.
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_ops::{ArtifactType, location::LocationSpec};
    /// let s = LocationSpec { vault: None, dir: Some("Lib".into()), root: Some(true) };
    /// let e = s.to_location(ArtifactType::Snippet).unwrap_err();
    /// assert_eq!(e.params["reason"], "both");
    /// ```
    pub fn to_location(&self, t: ArtifactType) -> Result<Option<Location>, OpError> {
        let bad = |reason: &str| {
            OpError::new(error::CONFIG_LOCATION_INVALID)
                .with("type", t.as_str())
                .with("reason", reason)
        };
        let place = match (&self.dir, self.root) {
            (Some(_), Some(true)) => return Err(bad("both")),
            (Some(d), _) => Where::Dir(d.clone()),
            (None, Some(true)) => Where::Root,
            (None, Some(false)) => return Err(bad("neither")),
            (None, None) if self.vault.is_none() => return Ok(None),
            (None, None) => Where::Default,
        };
        Ok(Some(Location {
            vault: self.vault.clone(),
            place,
        }))
    }
}

/// Would `t` at `loc` (`None` = default) be a valid stored location? The shape (D-14 via
/// `set_location` on a clone), the vault (`validate_vault`), then D-3 against every other type's
/// effective location in `cfg`. Nothing is written.
pub(crate) fn check_location(
    ctx: &Ctx,
    cfg: &MachineConfig,
    t: ArtifactType,
    loc: Option<&Location>,
) -> Result<(), OpError> {
    let mut shaped = cfg.clone();
    set_location(&mut shaped, t, loc.cloned())?;
    let mine = eff_of(ctx, t, loc.unwrap_or(&DEFAULT))?;
    conflicts(ctx, &shaped, t, &mine)
}

/// The place `t` would have at `loc` (`None` = default): the D-5 guards on a hypothetical
/// location (vault validated, an explicit `dir` pinned: a symlink out or to another type's dir is
/// `artifact.bad_path`). A not-yet-created dir passes (`contain` anchors on an ancestor).
pub(crate) fn place_for(
    ctx: &Ctx,
    t: ArtifactType,
    loc: Option<&Location>,
) -> Result<Place, OpError> {
    finish(t, eff_of(ctx, t, loc.unwrap_or(&DEFAULT))?)
}

/// Store `loc` for `t` in `cfg` (one `read_plain` read, its hash kept) and write the machine file:
/// hash-checked (`file.conflict` on a race), unknown keys kept.
pub(crate) fn persist_location(
    dir: &Path,
    mut cfg: MachineConfig,
    t: ArtifactType,
    loc: Option<Location>,
) -> Result<(), OpError> {
    set_location(&mut cfg, t, loc)?;
    Ok(write_machine(dir, &cfg)?)
}
