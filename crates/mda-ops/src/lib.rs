//! THE command list for MD Artifacts. Every operation any client can perform is registered once
//! here and reached through [`dispatch`]: by the CLI, by `mda serve`, and later by the TUI and MCP.

pub mod error;
mod ops_list;
pub mod registry;
pub mod system;

pub use error::OpError;
pub use mda_vault::Root;
pub use registry::{Ctx, OpSpec, check_names, dispatch, dispatch_in, ops, typed};

/// The protocol version this engine speaks. Single owner for `system.version` and `initialize`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_ops::PROTOCOL, "1.0");
/// ```
pub const PROTOCOL: &str = "1.0";
