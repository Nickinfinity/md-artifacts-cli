//! `mda varsets …`: rank, apply and save variable sets; plus the `vars.list` / `vars.search` and
//! `varsets.match` / `varsets.apply` human printers.

use mda_ops::OpError;
use serde_json::{Map, Value, json};

use super::input::read_object;
use super::print::{clean, s};

/// `mda varsets …`
#[derive(clap::Subcommand)]
pub enum VarsetsCmd {
    /// Rank every Variables sub-set against an artifact's variables and tags.
    Match {
        /// Vault-relative path of the artifact.
        path: String,
        /// Block index (multi-block artifacts).
        #[arg(long)]
        block: Option<usize>,
    },
    /// Merge a sub-set's values over `--values` (a JSON object file, `-` = stdin); nothing is written.
    Apply {
        /// Vault-relative path of the Variables file.
        path: String,
        #[command(flatten)]
        at: super::vars::At,
        /// Current values as a JSON object (file, or `-` for stdin).
        #[arg(long)]
        values: Option<String>,
    },
    /// Save values as a new Variables file `<dir>/<slug(title)>.md`; never overwrites.
    Save {
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
        /// Values as a JSON object (file, or `-` for stdin).
        #[arg(long)]
        values: Option<String>,
    },
}

/// The op and params an `mda varsets` subcommand stands for.
pub(super) fn varsets_request(cmd: VarsetsCmd) -> (String, Result<Value, OpError>) {
    let (op, params) = match cmd {
        VarsetsCmd::Match { path, block } => {
            let mut p = Map::new();
            p.insert("path".into(), json!(path));
            if let Some(b) = block {
                p.insert("block".into(), json!(b));
            }
            ("varsets.match", Ok(Value::Object(p)))
        }
        VarsetsCmd::Apply { path, at, values } => ("varsets.apply", apply_params(path, at, values)),
        VarsetsCmd::Save {
            title,
            dir,
            description,
            tags,
            values,
        } => (
            "varsets.save",
            save_params(title, dir, description, tags, values),
        ),
    };
    (op.to_owned(), params)
}

fn apply_params(
    path: String,
    a: super::vars::At,
    values: Option<String>,
) -> Result<Value, OpError> {
    let mut p = Map::new();
    p.insert("path".into(), json!(path));
    if let Some(s) = super::vars::at(a) {
        p.insert("subset".into(), s);
    }
    if let Some(f) = values {
        p.insert("values".into(), read_object(&f)?);
    }
    Ok(Value::Object(p))
}

fn save_params(
    title: String,
    dir: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
    values: Option<String>,
) -> Result<Value, OpError> {
    let mut p = Map::new();
    p.insert("title".into(), json!(title));
    if let Some(d) = dir {
        p.insert("dir".into(), json!(d));
    }
    if let Some(d) = description {
        p.insert("description".into(), json!(d));
    }
    if !tags.is_empty() {
        p.insert("tags".into(), json!(tags));
    }
    if let Some(f) = values {
        p.insert("values".into(), read_object(&f)?);
    }
    Ok(Value::Object(p))
}

/// Human text for `vars.list` / `vars.search` / `varsets.match` / `varsets.apply`; `None` for other ops.
pub(super) fn success(op: &str, v: &Value) -> Option<String> {
    match op {
        "vars.list" | "vars.search" => Some(list(v)),
        "varsets.match" => Some(matches(v)),
        "varsets.apply" => Some(changes(v)),
        _ => None,
    }
}

/// A variable value as one line: strings as-is, structured values as compact JSON (all vault text).
fn val(v: &Value) -> String {
    match v.as_str() {
        Some(t) => clean(t, false),
        None => clean(&v.to_string(), false),
    }
}

/// `"{head} = {value}"`, with no trailing space when the value is empty.
fn assign(name: &str, value: &str) -> String {
    format!("{name} = {value}").trim_end().to_owned()
}

fn list(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    for f in v["files"].as_array().into_iter().flatten() {
        if let Some(c) = f.pointer("/error/code").and_then(Value::as_str) {
            out.push(format!("{}  ! {}", s(f, "path"), clean(c, false)));
            continue;
        }
        let title = if f["title"].is_string() {
            s(f, "title")
        } else {
            s(f, "name")
        };
        let mut head = format!("{}  {title}  [{}]", s(f, "path"), s(f, "shape"));
        let tags: Vec<String> = f["tags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|t| format!("#{}", clean(t, false)))
            .collect();
        if !tags.is_empty() {
            head.push_str(&format!("  {}", tags.join(" ")));
        }
        out.push(head);
        for st in f["subsets"].as_array().into_iter().flatten() {
            let titled = st["heading"].as_str().is_some_and(|h| !h.is_empty());
            if titled {
                let d = s(st, "description");
                let h = format!("  ## {}", s(st, "heading"));
                out.push(if d.is_empty() { h } else { format!("{h} {d}") });
            }
            let pad = if titled { "    " } else { "  " };
            for x in st["vars"].as_array().into_iter().flatten() {
                let value = x
                    .get("value")
                    .filter(|w| !w.is_null())
                    .map_or_else(|| s(x, "defaultValue"), val);
                out.push(format!("{pad}{}", assign(&s(x, "name"), &value)));
            }
        }
    }
    out.join("\n")
}

fn matches(v: &Value) -> String {
    let n = v["artifactVars"].as_array().map_or(0, Vec::len);
    v["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            let path = m["pathIndex"]
                .as_u64()
                .and_then(|i| v["paths"].get(usize::try_from(i).ok()?))
                .map_or_else(String::new, |p| clean(p.as_str().unwrap_or(""), false));
            format!(
                "{:.3}  {path}  {}  ({}/{n} vars, {} tags)",
                m["score"].as_f64().unwrap_or(0.0),
                s(m, "label"),
                m["matchedVars"].as_array().map_or(0, Vec::len),
                m["tagMatches"].as_u64().unwrap_or(0),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn changes(v: &Value) -> String {
    v["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let act = s(c, "action");
            let mut line = format!("{act:<10}  {}", assign(&s(c, "name"), &val(&c["newValue"])));
            if act == "overridden" {
                line.push_str(&format!("  (was {})", val(&c["oldValue"])));
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
