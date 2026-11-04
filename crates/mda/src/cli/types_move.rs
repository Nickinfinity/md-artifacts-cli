//! `mda types move plan|apply`: move a type's files to a new location (spec §15, D-9, D-32).

use mda_ops::OpError;
use serde_json::{Value, json};

use super::input::{bad_request, read_input};
use super::types::LocationArgs;

/// `mda types move …`
#[derive(clap::Subcommand)]
pub enum MoveCmd {
    /// List the files a move would copy and the targets that already exist (writes nothing).
    Plan {
        /// The artifact type, exact spelling.
        #[arg(value_name = "TYPE")]
        artifact_type: String,
        #[command(flatten)]
        at: LocationArgs,
    },
    /// Copy, verify, switch the location, then delete the sources.
    Apply {
        /// The artifact type, exact spelling.
        #[arg(value_name = "TYPE")]
        artifact_type: String,
        #[command(flatten)]
        at: LocationArgs,
        /// A `plan` response (or its `files` array) as JSON: a file, or `-` for stdin.
        #[arg(long)]
        files: String,
    },
}

/// The op and params for `mda types move …`: `{type, to: {vault?, dir?, root?}, files?}`.
pub(super) fn move_request(cmd: MoveCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        MoveCmd::Plan { artifact_type, at } => (
            "types.move.plan".to_owned(),
            at.fields()
                .map(|to| json!({ "type": artifact_type, "to": Value::Object(to) })),
        ),
        MoveCmd::Apply {
            artifact_type,
            at,
            files,
        } => (
            "types.move.apply".to_owned(),
            at.fields().and_then(|to| {
                Ok(json!({ "type": artifact_type, "to": Value::Object(to), "files": plan_files(&files)? }))
            }),
        ),
    }
}

/// The `files` array of a plan response (the whole response, or the array alone).
fn plan_files(p: &str) -> Result<Value, OpError> {
    let v: Value =
        serde_json::from_str(&read_input(p)?).map_err(|e| bad_request(&e.to_string()))?;
    let files = if v.is_array() {
        v
    } else {
        v.get("files").cloned().unwrap_or_default()
    };
    if files.is_array() {
        Ok(files)
    } else {
        Err(bad_request(
            "files must be a plan response or its files array",
        ))
    }
}

/// Human text for `types.move.*`: the plan as a file list with its conflicts, the apply as one
/// line. Paths come from the vault, so every one goes through `clean`.
pub(super) fn success(op: &str, v: &Value) -> Option<String> {
    use super::print::{clean, s};
    let text = |x: &Value| clean(x.as_str().unwrap_or("?"), false);
    Some(match op {
        "types.move.plan" => {
            let mut out: Vec<String> = Vec::new();
            for f in v["files"].as_array().into_iter().flatten() {
                out.push(format!("{}  {}", text(&f["path"]), s(f, "hash")));
            }
            for c in v["conflicts"].as_array().into_iter().flatten() {
                out.push(format!("conflict {}", text(c)));
            }
            out.push(format!(
                "{} file(s), {} byte(s), {} conflict(s)",
                v["files"].as_array().map_or(0, Vec::len),
                v["bytes"].as_u64().unwrap_or(0),
                v["conflicts"].as_array().map_or(0, Vec::len)
            ));
            out.join("\n")
        }
        "types.move.apply" => {
            let at = &v["location"];
            let place = match at["dir"].as_str() {
                Some(d) => format!("{}/{}", text(&at["vault"]), clean(d, false)),
                None => text(&at["vault"]),
            };
            format!(
                "moved {} file(s) to {place}",
                v["moved"].as_u64().unwrap_or(0)
            )
        }
        _ => return None,
    })
}
