//! The `vks` codec, read side: both dialects (legacy `KEY=value` and the strict YAML subset of spec
//! §9), chosen per fence by [`classify`]. Every rejection is an `Err(VarsError)`, never a partial list.

mod classify;
mod emit;
mod legacy;
pub mod value;
mod yaml;

pub use classify::{Dialect, classify};
pub use emit::{check_vars, emit_body};
pub use value::{VksList, VksRecord, VksValue};

use crate::error::{Limit, VKS_LIMIT, VarsError};
use crate::model::ParsedVar;

/// Largest `vks` body read, in bytes (spec §9.7).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Deepest container nesting inside one var's value.
pub const MAX_DEPTH: usize = 6;
/// Most `VksValue` nodes in one body.
pub const MAX_NODES: usize = 10_000;
/// Most items in one list.
pub const MAX_LIST_ITEMS: usize = 1_000;
/// Most keys in one map, the top level included (user override of the seed's 200).
pub const MAX_MAP_KEYS: usize = 255;
/// Most `regions × file defaults` a flagged multi-block file may expand to: every flagged block
/// receives every file default (TS parity), so output grows with the product (decision #26).
pub const MAX_BLOCK_DEFAULTS: usize = 100_000;

/// Read a ```` ```vks ```` fence body, either dialect.
///
/// # Examples
///
/// ```
/// let vars = mda_core::vks::read_fence("VK-a=1").unwrap();
/// assert_eq!(vars[0].name, "VK-a");
/// ```
pub fn read_fence(body: &str) -> Result<Vec<ParsedVar>, VarsError> {
    check_size(body)?;
    match classify(body) {
        Dialect::Yaml => yaml::read_yaml(body),
        Dialect::Legacy => legacy::read_legacy(body),
    }
}

/// Read an unfenced `vars:` section: always legacy.
///
/// # Examples
///
/// ```
/// let vars = mda_core::vks::read_section("VK-a=1").unwrap();
/// assert_eq!(vars.len(), 1);
/// ```
pub fn read_section(body: &str) -> Result<Vec<ParsedVar>, VarsError> {
    check_size(body)?;
    legacy::read_legacy(body)
}

/// Rejected before any split: the size bound is the work-bounding check for both dialects.
fn check_size(body: &str) -> Result<(), VarsError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(limit_error(Limit::BodyBytes, MAX_BODY_BYTES, 0));
    }
    Ok(())
}

/// A `vks.limit` error (`limit`, `max` params) at `line`.
pub(crate) fn limit_error(limit: Limit, max: usize, line: usize) -> VarsError {
    VarsError::new(VKS_LIMIT, line)
        .with("limit", limit.as_str())
        .with("max", max.to_string())
}
