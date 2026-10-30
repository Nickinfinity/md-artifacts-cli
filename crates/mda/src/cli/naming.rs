//! `mda naming …`: vault file-name checks (the human printer lives in `print.rs`).

use mda_ops::OpError;
use serde_json::{Map, Value, json};

/// `mda naming …`
#[derive(clap::Subcommand)]
pub enum NamingCmd {
    /// Check a file-name stem (without `.md`), or derive the default stem from `--title`.
    Check {
        /// The artifact type, exact spelling (`Snippet`, `AIPrompt`, …).
        #[arg(long = "type")]
        artifact_type: String,
        /// The title the default stem is derived from (ignored when `--file-name` is given).
        #[arg(long)]
        title: Option<String>,
        /// The stem to check, as given.
        #[arg(long = "file-name")]
        file_name: Option<String>,
    },
}

/// The op and params an `mda naming` subcommand stands for.
pub(super) fn naming_request(cmd: NamingCmd) -> (String, Result<Value, OpError>) {
    let NamingCmd::Check {
        artifact_type,
        title,
        file_name,
    } = cmd;
    let mut p = Map::new();
    p.insert("type".into(), json!(artifact_type));
    if let Some(t) = title {
        p.insert("title".into(), json!(t));
    }
    if let Some(f) = file_name {
        p.insert("fileName".into(), json!(f));
    }
    ("naming.check".to_owned(), Ok(Value::Object(p)))
}
