//! The engine's core code constants and the error values a parsed artifact carries, split by family
//! (`error/{vks,naming,render,leetcode,path}.rs`; every item re-exported here, so `mda_core::error::X`
//! paths never change). Codes are protocol: `mda-ops` lists them in `ALL_CODES`; renaming one is a
//! breaking change.

mod leetcode;
mod naming;
mod path;
mod render;
mod vks;

pub use leetcode::*;
pub use naming::*;
pub use path::*;
pub use render::*;
pub use vks::*;

/// The `reason` vocabulary of `artifact.unrepresentable`.
///
/// # Examples
///
/// ```
/// use mda_core::error::FieldReason;
/// assert_eq!(FieldReason::RoundTrip.as_str(), "round_trip");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldReason {
    RoundTrip,
    MultiBlock,
}

impl FieldReason {
    /// The protocol spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoundTrip => "round_trip",
            Self::MultiBlock => "multi_block",
        }
    }
}

/// The `reason` vocabulary of `artifact.not_serializable`: why the engine will not rewrite a file.
///
/// # Examples
///
/// ```
/// use mda_core::error::Refusal;
/// assert_eq!(Refusal::NoFrontmatter.as_str(), "no_frontmatter");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Flagged,
    Index,
    Variables,
    NoFrontmatter,
    NoFence,
}

impl Refusal {
    /// The protocol spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flagged => "flagged",
            Self::Index => "index",
            Self::Variables => "variables",
            Self::NoFrontmatter => "no_frontmatter",
            Self::NoFence => "no_fence",
        }
    }
}

/// A value the emitter refused: the top-level var it belongs to and why.
///
/// # Examples
///
/// ```
/// use mda_core::error::{EmitReason, Unrepresentable};
/// let u = Unrepresentable { var: "VK-a".into(), reason: EmitReason::Backticks };
/// assert_eq!(u.reason.as_str(), "backticks");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unrepresentable {
    pub var: String,
    pub reason: EmitReason,
}

/// The model cannot be written in the file format. Params `field`, `reason` (`round_trip` |
/// `multi_block`). Core-owned since W6 so a migration `Skip` names it (re-exported by `mda-ops`).
pub const ARTIFACT_UNREPRESENTABLE: &str = "artifact.unrepresentable";
/// Frontmatter migration: blank line(s) precede the frontmatter block (spec §11.1). No params.
pub const MIGRATE_LEADING_BLANK: &str = "migrate.leading_blank";
/// Frontmatter migration: the `type:` value is quoted (spec §11.1). Param `value` (raw, quotes kept).
pub const MIGRATE_QUOTED_TYPE: &str = "migrate.quoted_type";
