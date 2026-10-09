//! Pure domain logic for MD Artifacts: the artifact model, parsing, serialization and rendering.
//!
//! This crate performs no I/O: no filesystem, no process, no clock, no environment, no stdout.
//! Every client reaches it through `mda-ops`.
