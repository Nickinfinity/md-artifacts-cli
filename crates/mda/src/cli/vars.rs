//! `mda vars …`: the Variables-file edit subcommands and the requests they stand for.

use mda_ops::OpError;
use serde_json::{Value, json};

use super::input::bad_request;

/// `mda vars …`
#[derive(clap::Subcommand)]
pub enum VarsCmd {
    /// Create a Variables file.
    #[command(subcommand)]
    File(FileCmd),
    /// File tags.
    #[command(subcommand)]
    Meta(MetaCmd),
    /// Sub-sets (`## heading` blocks).
    #[command(subcommand)]
    Subset(SubsetCmd),
    /// Variables.
    #[command(subcommand)]
    Var(VarCmd),
}

/// Optional sub-set reference (absent = the untitled block).
#[derive(clap::Args)]
pub struct At {
    /// Sub-set index (as `artifact show` lists blocks); needs `--heading`.
    #[arg(long = "subset", requires = "heading")]
    pub index: Option<usize>,
    /// The heading of `--subset` (exact; `""` = the untitled block).
    #[arg(long, requires = "index")]
    pub heading: Option<String>,
}

/// Required sub-set reference.
#[derive(clap::Args)]
pub struct AtReq {
    /// Sub-set index (as `artifact show` lists blocks).
    #[arg(long = "subset")]
    pub index: usize,
    /// The heading of `--subset` (exact; `""` = the untitled block).
    #[arg(long)]
    pub heading: String,
}

/// `mda vars file …`
#[derive(clap::Subcommand)]
pub enum FileCmd {
    /// New blank Variables file `<dir>/<slug(title)>.md`; never overwrites.
    New {
        /// The file's title (also its slugged name).
        title: String,
        /// The folder, `Variables` or under it (default `Variables`).
        #[arg(long)]
        dir: Option<String>,
        /// The file's description.
        #[arg(long)]
        description: Option<String>,
        /// A tag (repeatable).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
}

/// `mda vars meta …`
#[derive(clap::Subcommand)]
pub enum MetaCmd {
    /// Replace the file's tags (no `--tag` clears them).
    Tags {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The content hash last read (`artifact show --json`).
        #[arg(long)]
        hash: String,
        /// A tag (repeatable).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
}

/// `mda vars subset …`
#[derive(clap::Subcommand)]
pub enum SubsetCmd {
    /// Add an empty sub-set (a flat file's block becomes `Default`).
    Add {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The new sub-set's heading.
        #[arg(value_name = "HEADING")]
        new: String,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
    /// Rename a sub-set.
    Rename {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The new heading.
        #[arg(value_name = "NEW_HEADING")]
        new_heading: String,
        #[command(flatten)]
        at: AtReq,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
    /// Set a sub-set's description (`""` removes it).
    Describe {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The description.
        description: String,
        #[command(flatten)]
        at: AtReq,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
    /// Delete a sub-set.
    Delete {
        /// Vault-relative path of the Variables file.
        path: String,
        #[command(flatten)]
        at: AtReq,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
}

/// `mda vars var …`
#[derive(clap::Subcommand)]
pub enum VarCmd {
    /// Add a variable (default value "").
    Add {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The variable name, verbatim (`VK-…`).
        name: String,
        #[command(flatten)]
        at: At,
        /// The value as a string.
        #[arg(long, conflicts_with = "json_value")]
        value: Option<String>,
        /// The value as JSON (string, array or object).
        #[arg(long = "json-value")]
        json_value: Option<String>,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
    /// Rename a variable, keeping its value.
    Rename {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The current name.
        name: String,
        /// The new name, verbatim (`VK-…`).
        new_name: String,
        #[command(flatten)]
        at: At,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
    /// Set a variable's value: `--value` (string) or `--json-value` (string | array | object).
    #[command(group(clap::ArgGroup::new("val").required(true).args(["value", "json_value"])))]
    Set {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The variable name.
        name: String,
        #[command(flatten)]
        at: At,
        /// The value as a string.
        #[arg(long)]
        value: Option<String>,
        /// The value as JSON (string, array or object).
        #[arg(long = "json-value")]
        json_value: Option<String>,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
    /// Delete a variable.
    Delete {
        /// Vault-relative path of the Variables file.
        path: String,
        /// The variable name.
        name: String,
        #[command(flatten)]
        at: At,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
}

/// The op an `mda vars` subcommand stands for.
fn op_name(cmd: &VarsCmd) -> &'static str {
    match cmd {
        VarsCmd::File(FileCmd::New { .. }) => "vars.file.new",
        VarsCmd::Meta(MetaCmd::Tags { .. }) => "vars.meta.tags",
        VarsCmd::Subset(SubsetCmd::Add { .. }) => "vars.subset.add",
        VarsCmd::Subset(SubsetCmd::Rename { .. }) => "vars.subset.rename",
        VarsCmd::Subset(SubsetCmd::Describe { .. }) => "vars.subset.describe",
        VarsCmd::Subset(SubsetCmd::Delete { .. }) => "vars.subset.delete",
        VarsCmd::Var(VarCmd::Add { .. }) => "vars.var.add",
        VarsCmd::Var(VarCmd::Rename { .. }) => "vars.var.rename",
        VarsCmd::Var(VarCmd::Set { .. }) => "vars.var.set",
        VarsCmd::Var(VarCmd::Delete { .. }) => "vars.var.delete",
    }
}

/// `{"index", "heading"}` when both are given (clap requires them together).
fn at(a: At) -> Option<Value> {
    Some(json!({ "index": a.index?, "heading": a.heading? }))
}

fn at_req(a: AtReq) -> Value {
    json!({ "index": a.index, "heading": a.heading })
}

/// `--value` is always a JSON string; only `--json-value` is parsed (the op rejects numbers).
fn value(value: Option<String>, json_value: Option<String>) -> Result<Option<Value>, OpError> {
    match (value, json_value) {
        (Some(v), _) => Ok(Some(Value::String(v))),
        (None, Some(j)) => serde_json::from_str(&j)
            .map(Some)
            .map_err(|e| bad_request(&format!("json-value: {e}"))),
        (None, None) => Ok(None),
    }
}

/// `base` plus the optional `subset` and `value` keys.
fn with_opts(mut base: Value, subset: Option<Value>, val: Option<Value>) -> Value {
    if let Some(o) = base.as_object_mut() {
        if let Some(s) = subset {
            o.insert("subset".to_owned(), s);
        }
        if let Some(v) = val {
            o.insert("value".to_owned(), v);
        }
    }
    base
}

fn subset_params(cmd: SubsetCmd) -> Value {
    match cmd {
        SubsetCmd::Add { path, new, hash } => {
            json!({ "path": path, "expectedHash": hash, "heading": new })
        }
        SubsetCmd::Rename {
            path,
            new_heading,
            at,
            hash,
        } => json!({
            "path": path, "expectedHash": hash, "subset": at_req(at), "heading": new_heading }),
        SubsetCmd::Describe {
            path,
            description,
            at,
            hash,
        } => json!({
            "path": path, "expectedHash": hash, "subset": at_req(at), "description": description }),
        SubsetCmd::Delete { path, at, hash } => {
            json!({ "path": path, "expectedHash": hash, "subset": at_req(at) })
        }
    }
}

fn var_params(cmd: VarCmd) -> Result<Value, OpError> {
    Ok(match cmd {
        VarCmd::Add {
            path,
            name,
            at: a,
            value: v,
            json_value,
            hash,
        } => with_opts(
            json!({ "path": path, "expectedHash": hash, "name": name }),
            at(a),
            value(v, json_value)?,
        ),
        VarCmd::Rename {
            path,
            name,
            new_name,
            at: a,
            hash,
        } => with_opts(
            json!({ "path": path, "expectedHash": hash, "name": name, "newName": new_name }),
            at(a),
            None,
        ),
        VarCmd::Set {
            path,
            name,
            at: a,
            value: v,
            json_value,
            hash,
        } => with_opts(
            json!({ "path": path, "expectedHash": hash, "name": name }),
            at(a),
            value(v, json_value)?,
        ),
        VarCmd::Delete {
            path,
            name,
            at: a,
            hash,
        } => with_opts(
            json!({ "path": path, "expectedHash": hash, "name": name }),
            at(a),
            None,
        ),
    })
}

/// The params of an `mda vars` subcommand; names and headings go verbatim.
fn params(cmd: VarsCmd) -> Result<Value, OpError> {
    Ok(match cmd {
        VarsCmd::File(FileCmd::New {
            title,
            dir,
            description,
            tags,
        }) => {
            let mut p = json!({ "title": title });
            if let Some(o) = p.as_object_mut() {
                o.extend(dir.map(|d| ("dir".to_owned(), json!(d))));
                o.extend(description.map(|d| ("description".to_owned(), json!(d))));
                if !tags.is_empty() {
                    o.insert("tags".to_owned(), json!(tags));
                }
            }
            p
        }
        VarsCmd::Meta(MetaCmd::Tags { path, hash, tags }) => {
            json!({ "path": path, "expectedHash": hash, "tags": tags })
        }
        VarsCmd::Subset(c) => subset_params(c),
        VarsCmd::Var(c) => var_params(c)?,
    })
}

/// The op and params an `mda vars` subcommand stands for.
pub(super) fn vars_request(cmd: VarsCmd) -> (String, Result<Value, OpError>) {
    (op_name(&cmd).to_owned(), params(cmd))
}
