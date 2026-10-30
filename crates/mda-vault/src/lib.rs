//! All filesystem access for MD Artifacts.
//!
//! Every path a client or a vault file names passes through [`contain`] before any I/O, and every
//! read is bounded by [`read_bounded`]. Errors are [`VaultError`]; `mda-ops` maps them to codes.

pub mod config;
mod contain;
pub mod error;
mod listing;
mod read;
mod rename;
mod vault;
mod write;

pub use contain::{Root, contain};
pub use error::{VaultError, WalkLimit};
pub use listing::{DirListing, MAX_WALK_BYTES, MAX_WALK_DEPTH, MAX_WALK_FILES, list_dir, walk_md};
pub use read::{file_len, read_bounded};
pub use rename::rename_new;
pub use vault::{DirState, create_type_dir, dir_state, validate_vault};
pub use write::{content_hash, create_new, delete, replace};
