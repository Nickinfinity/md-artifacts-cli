//! How a payload sits in the file (spec §7.6, W7c): fenced, a bare body, or flag-delimited regions
//! with the author's notes around them. Read reports it, create/update write it back, so
//! `create(read(f))` loses nothing the format defines.

/// The payload's on-disk form. `fence` is the default and is never serialized.
///
/// # Examples
///
/// ```
/// use mda_core::model::PayloadKind;
/// assert_eq!(serde_json::to_string(&PayloadKind::Flags).unwrap(), r#""flags""#);
/// assert!(PayloadKind::default().is_fence());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PayloadKind {
    /// A code fence per block (spec §1–§2).
    #[default]
    Fence,
    /// The whole body is the payload, no fence (whole-file types only, spec §7.5 rule 3).
    Bare,
    /// `%%oa:start%%` … `%%oa:end%%` regions, one per block (spec §7).
    Flags,
}

impl PayloadKind {
    /// True for the default form (serde `skip_serializing_if`).
    ///
    /// # Examples
    ///
    /// ```
    /// assert!(!mda_core::model::PayloadKind::Bare.is_fence());
    /// ```
    pub fn is_fence(&self) -> bool {
        *self == Self::Fence
    }
}

/// One flagged region's surroundings: its flag name, the notes before its start flag and whether
/// it is wrapped in `***` visual rules.
///
/// # Examples
///
/// ```
/// use mda_core::model::RegionLayout;
/// let r: RegionLayout = serde_json::from_str(r#"{"name":"Dev"}"#).unwrap();
/// assert!(r.notes.is_empty() && !r.rule);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegionLayout {
    /// The flag name (`""` = unnamed). With two or more regions it is the block heading.
    #[serde(default)]
    pub name: String,
    /// Text between the previous region (or the frontmatter) and this start flag, blank lines
    /// trimmed at both ends. Never part of the payload.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// The region's first and last lines are `***` visual rules (chrome, not payload).
    #[serde(default, skip_serializing_if = "is_false")]
    pub rule: bool,
}

/// The payload's layout. Default (fenced, nothing else) is omitted from every JSON shape.
///
/// # Examples
///
/// ```
/// use mda_core::model::{Layout, PayloadKind};
/// let l: Layout = serde_json::from_str(r#"{"payload":"flags","regions":[{"name":""}]}"#).unwrap();
/// assert_eq!(l.payload, PayloadKind::Flags);
/// assert!(Layout::default().is_default());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layout {
    #[serde(default, skip_serializing_if = "PayloadKind::is_fence")]
    pub payload: PayloadKind,
    /// `flags` only: one entry per region, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<RegionLayout>,
    /// `flags` only: text after the last region, minus the defaults section (`vars:` + `vks` fence).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

impl Layout {
    /// True for a plain fenced payload (serde `skip_serializing_if`).
    ///
    /// # Examples
    ///
    /// ```
    /// assert!(mda_core::model::Layout::default().is_default());
    /// ```
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}
