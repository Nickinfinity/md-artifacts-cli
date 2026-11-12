//! `mda leetcode …`: LeetCode exercises. H5.0/H6.0 skeleton; T5.1 fills the human printer. Write
//! inputs (the attempt JSON) are read here, client side; the op does the vault I/O.

use mda_ops::OpError;
use serde_json::{Map, Value, json};

use super::input::read_object;
use super::print::clean;

/// `mda leetcode …`
#[derive(clap::Subcommand)]
pub enum LeetcodeCmd {
    /// Parse one LeetCode exercise (`LeetCode/<rel>.md`).
    Read {
        /// Vault-relative path.
        path: String,
        /// Also return the layout (the carrier `leetcode write` re-emits byte for byte).
        #[arg(long)]
        layout: bool,
    },
    /// Record a run outcome (status, solved meta, attempt) when the hash is `--hash`.
    #[command(name = "outcome", alias = "record-outcome")]
    Outcome {
        /// Vault-relative path.
        path: String,
        /// The content hash last read (`leetcode read --json`).
        #[arg(long)]
        hash: String,
        /// `unsolved`, `attempted` or `solved`.
        #[arg(long)]
        status: String,
        /// The solution's language (an alias such as `py` is resolved).
        #[arg(long)]
        language: String,
        /// ISO-8601 time of the solve (writes the solved meta with `--duration`).
        #[arg(long = "solved-at")]
        solved_at: Option<String>,
        /// Elapsed time of the solve, e.g. `8m22s`.
        #[arg(long, requires = "solved_at")]
        duration: Option<String>,
        /// The attempt as a JSON object file (`at`, `duration`, `passed`, `bigO?`, `confidence?`, `code`), `-` = stdin.
        #[arg(long)]
        attempt: Option<String>,
    },
    /// Create an exercise from the canonical skeleton; never overwrites.
    #[command(name = "new", alias = "create")]
    New {
        /// Vault-relative path, `LeetCode/<rel>.md`.
        path: String,
        /// The exercise title.
        #[arg(long)]
        title: String,
        /// `function`, `package` or `stack`.
        #[arg(long = "type")]
        leetcode_type: String,
        /// `easy`, `medium` or `hard`.
        #[arg(long)]
        difficulty: String,
        /// Category tag, e.g. `hash-map`.
        #[arg(long)]
        algorithm: Option<String>,
        /// A tag (repeatable).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Write an exercise from a `leetcode read --json [--layout]` response file; creates, or replaces
    /// when `--hash` is given.
    Write {
        /// Vault-relative path, `LeetCode/<rel>.md`.
        path: String,
        /// The `{exercise, layout?}` JSON file (its `hash` is ignored), `-` = stdin.
        #[arg(long)]
        file: String,
        /// Replace: the content hash last read.
        #[arg(long)]
        hash: Option<String>,
    },
}

/// The op and params an `mda leetcode` subcommand stands for.
pub(super) fn leetcode_request(cmd: LeetcodeCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        LeetcodeCmd::Read { path, layout } => {
            let p = if layout {
                json!({ "path": path, "layout": true })
            } else {
                json!({ "path": path })
            };
            ("leetcode.read".to_owned(), Ok(p))
        }
        LeetcodeCmd::Write { path, file, hash } => {
            ("leetcode.write".to_owned(), write_params(path, &file, hash))
        }
        LeetcodeCmd::Outcome {
            path,
            hash,
            status,
            language,
            solved_at,
            duration,
            attempt,
        } => {
            let mut p = Map::new();
            p.insert("path".into(), json!(path));
            p.insert("expectedHash".into(), json!(hash));
            p.insert("status".into(), json!(status));
            p.insert("language".into(), json!(language));
            if let Some(at) = solved_at {
                let mut s = Map::new();
                s.insert("solvedAt".into(), json!(at));
                if let Some(d) = duration {
                    s.insert("duration".into(), json!(d));
                }
                p.insert("solved".into(), Value::Object(s));
            }
            let params = match attempt.map(|f| read_object(&f)).transpose() {
                Ok(a) => {
                    if let Some(a) = a {
                        p.insert("attempt".into(), a);
                    }
                    Ok(Value::Object(p))
                }
                Err(e) => Err(e),
            };
            ("leetcode.record_outcome".to_owned(), params)
        }
        LeetcodeCmd::New {
            path,
            title,
            leetcode_type,
            difficulty,
            algorithm,
            tags,
        } => {
            let mut p = Map::new();
            p.insert("path".into(), json!(path));
            p.insert("title".into(), json!(title));
            p.insert("leetcodeType".into(), json!(leetcode_type));
            p.insert("difficulty".into(), json!(difficulty));
            if let Some(a) = algorithm {
                p.insert("algorithm".into(), json!(a));
            }
            if !tags.is_empty() {
                p.insert("tags".into(), json!(tags));
            }
            ("leetcode.create".to_owned(), Ok(Value::Object(p)))
        }
    }
}

/// `leetcode.write` params from a read-response file: its `exercise` and `layout`, never its `hash`.
fn write_params(path: String, file: &str, hash: Option<String>) -> Result<Value, OpError> {
    let src = read_object(file)?;
    let mut p = Map::new();
    p.insert("path".into(), json!(path));
    p.insert(
        "exercise".into(),
        src.get("exercise").cloned().unwrap_or(Value::Null),
    );
    if let Some(l) = src.get("layout") {
        p.insert("layout".into(), l.clone());
    }
    if let Some(h) = hash {
        p.insert("expectedHash".into(), json!(h));
    }
    Ok(Value::Object(p))
}

/// Human text for `leetcode.*`: `leetcode.read` prints a summary; the writes fall back to JSON.
pub(super) fn success(op: &str, v: &Value) -> Option<String> {
    if op != "leetcode.read" {
        return None;
    }
    let ex = v.get("exercise")?;
    let s = |k: &str| clean(ex.get(k).and_then(Value::as_str).unwrap_or(""), false);
    let warnings = ex
        .get("warnings")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let hash = clean(v.get("hash").and_then(Value::as_str).unwrap_or(""), false);
    Some(format!(
        "{} ({}, {}, {})\nwarnings: {warnings}\nhash: {hash}",
        s("title"),
        s("leetcodeType"),
        s("difficulty"),
        s("status"),
    ))
}
