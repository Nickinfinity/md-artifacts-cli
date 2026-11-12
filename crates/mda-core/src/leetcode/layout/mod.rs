//! The layout producer (W7 T7.1): a decoded LeetCode file → ordered [`Block`]s whose `raw` texts
//! concatenate to the input exactly, each tagged with the model fields it feeds. One line scan with
//! offset accumulation over the existing scanners; no second grammar. H7.pre stub with the pinned
//! signature; T7.1 owns this module.

use super::Block;

/// Why no layout was built. The block cap is checked before building (`MAX_LAYOUT_BLOCKS`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LayoutRefusal;
/// assert_eq!(LayoutRefusal::TooManyBlocks, LayoutRefusal::TooManyBlocks);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutRefusal {
    TooManyBlocks,
}

/// The layout of `src` (decoded text, as every `leetcode::*` entry point).
///
/// # Examples
///
/// ```
/// let blocks = mda_core::leetcode::layout("---\ntitle: T\n---\n");
/// assert!(blocks.is_ok());
/// ```
pub fn layout(src: &str) -> Result<Vec<Block>, LayoutRefusal> {
    let _ = src;
    Ok(vec![])
}
