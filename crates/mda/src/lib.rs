//! The `mda` front-ends: the command line ([`cli`]), the JSON-RPC server ([`serve`]) and the one
//! logger ([`debug`]). Every operation runs through `mda_ops::dispatch`; this crate only parses,
//! frames and prints.

pub mod cli;
pub mod debug;
pub mod serve;
