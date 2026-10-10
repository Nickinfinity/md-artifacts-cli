//! Client-side CLI input: files named on the command line (`-` = stdin) and the request objects
//! built from them. Not vault I/O.

use std::io::Read;

use mda_ops::OpError;
use serde_json::{Map, Value, json};

use crate::serve;

/// The `edit` object of an `artifact.patch` request (clap guarantees exactly one edit kind).
pub(crate) fn patch_edit(
    title: Option<String>,
    description: Option<String>,
    block: Option<usize>,
    heading: Option<String>,
    code_file: Option<String>,
) -> Result<Value, OpError> {
    if let Some(v) = title {
        return Ok(json!({ "field": "title", "value": v }));
    }
    if let Some(v) = description {
        return Ok(json!({ "field": "description", "value": v }));
    }
    let code = read_input(code_file.as_deref().unwrap_or("-"))?;
    Ok(match (block, heading) {
        (Some(b), Some(h)) => json!({ "field": "code", "block": b, "heading": h, "code": code }),
        _ => json!({ "field": "code", "code": code }),
    })
}

/// Client-side input (`-` = stdin, else a file), capped at the protocol frame size. Not vault I/O:
/// no `--vault` containment applies to a file the user names on their own command line.
pub(crate) fn read_input(p: &str) -> Result<String, OpError> {
    // ponytail: `--model <FIFO>` blocks until a writer appears (the user's own argument); check
    // metadata first if a client ever passes untrusted paths here.
    let cap = serve::MAX_LINE_BYTES as u64;
    let mut buf = Vec::new();
    let res = if p == "-" {
        std::io::stdin().lock().take(cap + 1).read_to_end(&mut buf)
    } else {
        std::fs::File::open(p).and_then(|f| f.take(cap + 1).read_to_end(&mut buf))
    };
    res.map_err(|e| bad_request(&format!("cannot read input: {}", e.kind())))?;
    if buf.len() as u64 > cap {
        return Err(bad_request("input too large"));
    }
    String::from_utf8(buf).map_err(|_| bad_request("input is not UTF-8"))
}

pub(crate) fn bad_request(reason: &str) -> OpError {
    OpError::new(mda_ops::error::OP_BAD_REQUEST).with("reason", reason)
}

/// A JSON object read from a CLI input (`--values`): file or `-` for stdin.
pub(crate) fn read_object(p: &str) -> Result<Value, OpError> {
    let v: Value =
        serde_json::from_str(&read_input(p)?).map_err(|e| bad_request(&e.to_string()))?;
    if v.is_object() {
        Ok(v)
    } else {
        Err(bad_request("values must be a JSON object"))
    }
}

/// `artifact.render` params from `mda artifact render`.
pub(crate) fn render_params(
    path: String,
    block: Option<usize>,
    code_file: Option<String>,
    values: Option<String>,
) -> Result<Value, OpError> {
    let mut p = Map::new();
    p.insert("path".into(), json!(path));
    if let Some(b) = block {
        p.insert("block".into(), json!(b));
    }
    if let Some(f) = code_file {
        p.insert("code".into(), json!(read_input(&f)?));
    }
    if let Some(f) = values {
        p.insert("values".into(), read_object(&f)?);
    }
    Ok(Value::Object(p))
}

/// `artifact.write_file` params from `mda artifact write-file`; the workspace becomes absolute here
/// (client side), so the op sees the path the user meant.
pub(crate) fn write_file_params(w: WriteFileArgs) -> Result<Value, OpError> {
    let ws = std::path::absolute(&w.workspace).map_err(|_| bad_request("workspace"))?;
    let mut p = Map::new();
    p.insert("path".into(), json!(w.path));
    p.insert("workspaceRoot".into(), json!(ws.display().to_string()));
    p.insert("destDir".into(), json!(w.dest));
    if let Some(n) = w.name {
        p.insert("fileName".into(), json!(n));
    }
    if let Some(b) = w.block {
        p.insert("block".into(), json!(b));
    }
    if let Some(f) = w.values {
        p.insert("values".into(), read_object(&f)?);
    }
    if let Some(h) = w.hash {
        p.insert("expectedHash".into(), json!(h));
    }
    Ok(Value::Object(p))
}

/// `artifact.prefill` params from `mda artifact prefill`; bad `source`/`type` strings reach the op's
/// serde (exit 1), not clap (exit 2).
pub(crate) fn prefill_params(
    source: String,
    artifact_type: String,
    text_file: &str,
    language_id: Option<String>,
    file_name: Option<String>,
) -> Result<Value, OpError> {
    let mut p = Map::new();
    p.insert("source".into(), json!(source));
    p.insert("type".into(), json!(artifact_type));
    p.insert("text".into(), json!(read_input(text_file)?));
    if let Some(l) = language_id {
        p.insert("languageId".into(), json!(l));
    }
    if let Some(f) = file_name {
        p.insert("fileName".into(), json!(f));
    }
    Ok(Value::Object(p))
}

/// The `write-file` subcommand's arguments, grouped to keep the dispatch arm short.
pub(crate) struct WriteFileArgs {
    pub path: String,
    pub workspace: std::path::PathBuf,
    pub dest: String,
    pub name: Option<String>,
    pub block: Option<usize>,
    pub values: Option<String>,
    pub hash: Option<String>,
}
