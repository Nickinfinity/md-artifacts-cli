//! `mda vault …`, `mda config …`: subcommands → op requests (paths made absolute
//! client side), and their human printers (T4.5).

use std::path::PathBuf;

use mda_ops::OpError;
use serde_json::{Value, json};

use super::input::absolute;

/// `mda vault …`
#[derive(clap::Subcommand)]
pub enum VaultCmd {
    /// Inspect a folder: is it a vault, which type dirs exist (reads only).
    Inspect {
        /// The folder.
        path: PathBuf,
    },
    /// Validate a vault, create its default type dirs, remember it and make it the default.
    Select {
        /// The vault folder.
        path: PathBuf,
        /// Do not create the default type directories.
        #[arg(long = "no-defaults")]
        no_defaults: bool,
    },
    /// List known vaults and the default.
    List,
    /// Make a known vault the default.
    Use {
        /// A known vault folder.
        path: PathBuf,
    },
    /// Check every artifact file: read errors, invalid variables, pending migrations (exit 1 on errors).
    Validate,
}

/// `mda config …`
#[derive(clap::Subcommand)]
pub enum ConfigCmd {
    /// The session vault, where it came from, the config dir.
    Get,
}

/// The op and params for `mda vault …`.
pub(super) fn vault_request(cmd: VaultCmd) -> (String, Result<Value, OpError>) {
    let path = |p: &std::path::Path| absolute(p, "path");
    match cmd {
        VaultCmd::Inspect { path: p } => (
            "vault.inspect".to_owned(),
            path(&p).map(|p| json!({ "path": p })),
        ),
        VaultCmd::Select {
            path: p,
            no_defaults,
        } => (
            "vault.select".to_owned(),
            path(&p).map(|p| json!({ "path": p, "createDefaults": !no_defaults })),
        ),
        VaultCmd::List => ("vault.list".to_owned(), Ok(json!({}))),
        VaultCmd::Use { path: p } => (
            "vault.use".to_owned(),
            path(&p).map(|p| json!({ "path": p })),
        ),
        VaultCmd::Validate => ("vault.validate".to_owned(), Ok(json!({}))),
    }
}

/// The op and params for `mda config …`.
pub(super) fn config_request(cmd: ConfigCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        ConfigCmd::Get => ("config.get".to_owned(), Ok(json!({}))),
    }
}

/// Human text for the vault/config ops; `None` for any other op. Every stored or
/// client-supplied path goes through `s()`/`clean` (a hand-edited config must not reach the terminal raw).
pub(super) fn success(op: &str, v: &Value) -> Option<String> {
    use super::print::{clean, coded_value, s};
    let rows = |k: &str| v[k].as_array().into_iter().flatten();
    let name = |x: &Value, k: &str| clean(x[k].as_str().unwrap_or("?"), false);
    let lines: Vec<String> = match op {
        "vault.inspect" => {
            let kind = if v["isObsidianVault"] == true {
                "vault"
            } else {
                "not a vault"
            };
            let mut l = vec![format!("{}  {kind}", s(v, "path"))];
            l.extend(rows("dirs").map(|d| format!("{}/  {}", name(d, "dir"), name(d, "state"))));
            let miss: Vec<String> = rows("missingDefaults")
                .map(|t| clean(t.as_str().unwrap_or("?"), false))
                .collect();
            if !miss.is_empty() {
                l.push(format!("missing defaults: {}", miss.join(", ")));
            }
            l
        }
        "vault.select" => std::iter::once(format!("selected {}", s(v, "path")))
            .chain(
                rows("created")
                    .map(|d| format!("created {}/", clean(d.as_str().unwrap_or("?"), false))),
            )
            .collect(),
        "vault.use" => vec![format!("using {}", s(v, "path"))],
        "vault.list" => {
            let l: Vec<String> = rows("vaults")
                .map(|x| {
                    let mark = if x["path"] == v["default"] { "*" } else { " " };
                    let off = if x["available"] == false {
                        "  (unavailable)"
                    } else {
                        ""
                    };
                    format!("{mark} {}{off}", name(x, "path"))
                })
                .collect();
            if l.is_empty() {
                vec!["no known vaults".to_owned()]
            } else {
                l
            }
        }
        "config.get" => {
            let or_none = |k: &str| {
                if v[k].is_string() {
                    s(v, k)
                } else {
                    "none".to_owned()
                }
            };
            let mut l = vec![
                format!("vault {}", or_none("vault")),
                format!("source {}", s(v, "source")),
                format!("config {}", or_none("configDir")),
            ];
            let err = &v["defaultError"];
            if let Some(code) = err["code"].as_str() {
                l.push(format!(
                    "default error: {}",
                    coded_value(code, &err["params"])
                ));
            }
            l
        }
        _ => return None,
    };
    Some(lines.join("\n"))
}
