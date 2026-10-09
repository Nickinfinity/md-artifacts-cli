//! All filesystem access for MD Artifacts.
//!
//! Every path a client or a vault file names passes through [`contain`] before any I/O, and every
//! read is bounded by [`read_bounded`]. Errors are [`VaultError`]; `mda-ops` maps them to codes.

mod contain;
pub mod error;
mod read;

pub use contain::{Root, contain};
pub use error::VaultError;
pub use read::read_bounded;
