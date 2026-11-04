//! `mda types …`: list, enable and locate artifact types (spec §15); `mda types move …` lives in
//! `types_move.rs`. Subcommands → op requests, and their human printers.

use std::path::PathBuf;

use mda_ops::OpError;
use serde_json::{Map, Value, json};

use super::input::absolute;

/// `mda types …`
#[derive(clap::Subcommand)]
pub enum TypesCmd {
    /// Each type: dir, enabled, dir exists, location.
    List,
    /// Enable (creates the dir) or disable (never deletes) a type in this vault.
    #[command(name = "set-enabled")]
    SetEnabled {
        /// The artifact type, exact spelling (`Snippet`, `Command`, …).
        #[arg(value_name = "TYPE")]
        artifact_type: String,
        /// `true` or `false`.
        #[arg(action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Point a type at a location without moving files (no flags: back to the default).
    #[command(name = "set-location")]
    SetLocation {
        /// The artifact type, exact spelling.
        #[arg(value_name = "TYPE")]
        artifact_type: String,
        #[command(flatten)]
        at: LocationArgs,
    },
    /// Move a type's files to a new location, then switch to it.
    #[command(subcommand)]
    Move(super::types_move::MoveCmd),
}

/// A location on the command line (`--at`, never `--vault`: that is the global session flag).
#[derive(clap::Args)]
pub struct LocationArgs {
    /// The vault (absolute after resolving); omitted = the session vault.
    #[arg(long)]
    pub at: Option<PathBuf>,
    /// The directory inside the vault (one segment).
    #[arg(long, conflicts_with = "root")]
    pub dir: Option<String>,
    /// The vault root (the type has the vault to itself).
    #[arg(long)]
    pub root: bool,
}

impl LocationArgs {
    /// The `{vault?, dir?, root?}` fields, `vault` made absolute client side.
    pub(super) fn fields(&self) -> Result<Map<String, Value>, OpError> {
        let mut m = Map::new();
        if let Some(p) = &self.at {
            m.insert("vault".into(), json!(absolute(p, "at")?));
        }
        if let Some(d) = &self.dir {
            m.insert("dir".into(), json!(d));
        }
        if self.root {
            m.insert("root".into(), json!(true));
        }
        Ok(m)
    }
}

/// The op and params for `mda types …`.
pub(super) fn types_request(cmd: TypesCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        TypesCmd::List => ("types.list".to_owned(), Ok(json!({}))),
        TypesCmd::SetEnabled {
            artifact_type,
            enabled,
        } => (
            "types.set_enabled".to_owned(),
            Ok(json!({ "type": artifact_type, "enabled": enabled })),
        ),
        TypesCmd::SetLocation { artifact_type, at } => (
            "types.set_location".to_owned(),
            at.fields().map(|mut m| {
                m.insert("type".into(), json!(artifact_type));
                Value::Object(m)
            }),
        ),
        TypesCmd::Move(cmd) => super::types_move::move_request(cmd),
    }
}

/// Human text for the `types.*` ops; `None` for any other op. Every stored path goes through
/// `clean` (a hand-edited config must not reach the terminal raw).
pub(super) fn success(op: &str, v: &Value) -> Option<String> {
    use super::print::{clean, s};
    if op.starts_with("types.move.") {
        return super::types_move::success(op, v);
    }
    let name = |x: &Value, k: &str| clean(x[k].as_str().unwrap_or("?"), false);
    let lines: Vec<String> = match op {
        "types.list" => v
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| {
                if let Some(code) = t.pointer("/error/code").and_then(Value::as_str) {
                    return format!(
                        "{}  {}/  error: {}",
                        name(t, "type"),
                        name(t, "dir"),
                        clean(code, false)
                    );
                }
                let on = if t["enabled"] == true {
                    "enabled"
                } else {
                    "disabled"
                };
                let nodir = if t["dirExists"] == true {
                    ""
                } else {
                    "  (no dir)"
                };
                format!(
                    "{}  {}/  {on}{nodir}  → {}",
                    name(t, "type"),
                    name(t, "dir"),
                    place(&t["location"])
                )
            })
            .collect(),
        "types.set_enabled" => {
            if v["enabled"] == true {
                // The response names the type, not its dir: the registry (re-exported) has it.
                let dir = mda_ops::TYPES
                    .iter()
                    .find(|t| json!(t.artifact_type) == v["type"])
                    .map_or("?", |t| t.dir);
                let made = if v["created"] == true {
                    format!("  created {dir}/")
                } else {
                    String::new()
                };
                vec![format!("enabled {}{made}", s(v, "type"))]
            } else {
                vec![format!("disabled {}", s(v, "type"))]
            }
        }
        "types.set_location" => vec![format!("location → {}", place(&v["location"]))],
        _ => return None,
    };
    Some(lines.join("\n"))
}

/// A `LocationView` as one path: the vault joined with its dir, or the bare vault at a root.
fn place(l: &Value) -> String {
    let vault = super::print::clean(l["vault"].as_str().unwrap_or("?"), false);
    match l["dir"].as_str() {
        Some(d) => format!("{vault}/{}", super::print::clean(d, false)),
        None => vault,
    }
}
