//! All filesystem access for MD Artifacts.
//!
//! Every path a client or a vault file names passes through [`contain`] before any I/O, and every
//! read is bounded by [`read_bounded`]. Errors are [`VaultError`]; `mda-ops` maps them to codes.

mod contain;
pub mod error;
mod listing;
mod read;
mod write;

pub use contain::{Root, contain};
pub use error::VaultError;
pub use listing::{DirListing, list_dir};
pub use read::read_bounded;
pub use write::{content_hash, create_new, delete, replace};
