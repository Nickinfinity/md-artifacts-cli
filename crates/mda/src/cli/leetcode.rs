//! `mda leetcode …`: LeetCode exercises. H5.0 skeleton; T5.1 fills the human printer.

use mda_ops::OpError;
use serde_json::{Value, json};

/// `mda leetcode …`
#[derive(clap::Subcommand)]
pub enum LeetcodeCmd {
    /// Parse one LeetCode exercise (`LeetCode/<rel>.md`).
    Read {
        /// Vault-relative path.
        path: String,
    },
}

/// The op and params an `mda leetcode` subcommand stands for.
pub(super) fn leetcode_request(cmd: LeetcodeCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        LeetcodeCmd::Read { path } => ("leetcode.read".to_owned(), Ok(json!({ "path": path }))),
    }
}

/// Human text for `leetcode.*` (stub until T5.1: `None` falls back to the default printer).
pub(super) fn success(_op: &str, _v: &Value) -> Option<String> {
    None
}
