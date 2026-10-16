//! Pure domain logic for MD Artifacts: the artifact model, parsing, serialization and rendering.
//!
//! This crate performs no I/O: no filesystem, no process, no clock, no environment, no stdout.
//! Every client reaches it through `mda-ops`.

pub mod error;
pub mod language;
pub mod model;
pub mod multi_index;
pub mod naming;
pub mod parse;
pub mod patch;
pub mod registry;
pub mod render;
pub mod serialize;
pub mod variables;
pub mod varset;
pub mod vks;
