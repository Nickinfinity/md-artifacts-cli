//! Human rendering: the one English error table and per-op success printers.

use mda_ops::OpError;
use serde_json::Value;

/// English text for an error code; `None` for a code this table does not know.
///
/// # Examples
///
/// ```
/// assert!(mda::cli::print::english("op.unknown").is_some());
/// assert!(mda::cli::print::english("no.such").is_none());
/// ```
pub fn english(code: &str) -> Option<&'static str> {
    use mda_ops::error::*;
    Some(match code {
        OP_UNKNOWN => "no such operation",
        OP_BAD_REQUEST => "the request parameters are invalid",
        PROTOCOL_VERSION_MISMATCH => "client and engine protocol versions are incompatible",
        PATH_OUTSIDE_ROOT => "path is outside the allowed root",
        PATH_NOT_FOUND => "path does not exist",
        FILE_TOO_LARGE => "file exceeds the size limit",
        FILE_NOT_REGULAR => "not a regular file",
        IO_FAILED => "I/O failure",
        OP_INTERNAL => "internal engine error",
        VAULT_NOT_SELECTED => "no vault selected (pass --vault)",
        ARTIFACT_BAD_PATH => "not an artifact path (expected <TypeDir>/….md)",
        VKS_SYNTAX => "variables block: syntax error",
        VKS_INDENT => "variables block: bad indentation",
        VKS_BAD_KEY => "variables block: invalid key",
        VKS_DUPLICATE_KEY => "variables block: duplicate key",
        VKS_UNSUPPORTED => "variables block: unsupported YAML construct",
        VKS_MIXED_LIST => "variables block: list mixes strings and records",
        VKS_INLINE_COMMENT => "variables block: comment after a value",
        VKS_CONTROL_CHAR => "variables block: control character",
        VKS_LIMIT => "variables block: size limit exceeded",
        VKS_MIXED_DIALECT => "variables block: KEY=value line in a YAML block",
        VKS_UNREPRESENTABLE => "variables block: value cannot be written",
        FILE_CONFLICT => "the file changed since it was read (hash mismatch)",
        FILE_EXISTS => "a file already exists at that path",
        ARTIFACT_VARS_INVALID => "the file's variables block is invalid; fix it before saving",
        ARTIFACT_UNREPRESENTABLE => "the artifact cannot be written in the file format",
        ARTIFACT_BLOCK_NOT_FOUND => "no block with that index and heading",
        ARTIFACT_NOT_SERIALIZABLE => "this file cannot be rewritten by the engine",
        RENDER_LIMIT => "rendering exceeds a size limit",
        RENDER_CONTAINS_ESCAPE => {
            "rendered output contains an ESC character; use --json to receive it"
        }
        NAMING_PATH_INJECTION => "the output file name contains a path separator, \"..\", or NUL",
        NAMING_EMPTY => "the output file name is empty",
        NAMING_EDGE_SPACE => "the output file name starts or ends with a space",
        NAMING_EDGE_DOT => "the output file name ends with a dot",
        NAMING_ILLEGAL_CHAR => {
            "the output file name contains an illegal character (\\ / : * ? \" < > |)"
        }
        NAMING_CONTROL_CHAR => "the output file name contains a control character",
        NAMING_RESERVED => "the output file name is a reserved system name",
        NAMING_TOO_LONG => "the output file name is longer than 255 bytes",
        ARTIFACT_NOT_WHOLE_FILE => "this artifact type does not write a file",
        ARTIFACT_MULTI_BLOCK => "a whole-file artifact must have a single block",
        ARTIFACT_IS_INDEX => "a template index is run, not written",
        RENDER_UNKNOWN_VAR => "variable has no value; token left as written",
        RENDER_EACH_NOT_LIST => "each: the value is not a list",
        RENDER_UNTERMINATED => "each without a matching end",
        RENDER_UNMATCHED_END => "end without a matching each",
        RENDER_SELF_NESTED => "each over a list it is already inside",
        RENDER_NOT_SCALAR => "the token reaches a list or record outside a loop",
        RENDER_JOIN_RECORD => "join reaches a record",
        RENDER_JOIN_EMPTY => "join found no values",
        _ => return None,
    })
}

/// `<code>: <English>` plus params. Values are Debug-escaped: they can echo client text, and a raw
/// ESC or newline must not reach the terminal.
pub fn error_line(e: &OpError) -> String {
    coded_line(e.code, e.params.iter())
}

/// `<code>: <English> k="v" …`, the one shape of errors and warnings.
fn coded_line<'a>(code: &str, params: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    let mut s = format!("{code}: {}", english(code).unwrap_or("unknown error"));
    for (k, v) in params {
        s.push_str(&format!(" {k}={v:?}"));
    }
    s
}

/// A refusal of a successful response the human CLI will not print (R-2: rendered output holding
/// ESC); `None` to print it.
///
/// # Examples
///
/// ```
/// let _ = mda::cli::print::refuse("system.ops", &serde_json::json!({}));
/// ```
pub fn refuse(op: &str, v: &Value) -> Option<OpError> {
    // ponytail: refuses ESC only; C1 U+009B (single-char CSI) passes — R-25(15); refuse C1 too if a terminal client needs it.
    // write_file output goes to a file, so only render can reach the terminal.
    (op == "artifact.render" && v["containsEscape"] == true)
        .then(|| OpError::new(mda_ops::error::RENDER_CONTAINS_ESCAPE))
}

/// One stderr line per render warning in a response (Debug-escaped params).
///
/// # Examples
///
/// ```
/// let _ = mda::cli::print::warnings(&serde_json::json!({}));
/// ```
pub fn warnings(v: &Value) -> Vec<String> {
    v["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| {
            let params: std::collections::BTreeMap<String, String> = w["params"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, x)| (k.clone(), x.as_str().unwrap_or("").to_owned()))
                .collect();
            let line = coded_line(w["code"].as_str().unwrap_or("?"), params.iter());
            format!("warning: {line}")
        })
        .collect()
}

/// Human text for a successful response of `op`; pretty JSON when no printer exists.
/// Printers of vault-derived text pass it through `clean` (control characters escaped).
pub fn success(op: &str, v: &Value) -> String {
    match op {
        "system.version" => format!(
            "mda {}\nprotocol {}",
            v["engine"].as_str().unwrap_or("?"),
            v["protocol"].as_str().unwrap_or("?")
        ),
        "system.ops" => v["ops"]
            .as_array()
            .map(|ops| {
                ops.iter()
                    .map(|o| {
                        format!(
                            "{}  {}",
                            o["name"].as_str().unwrap_or("?"),
                            o["summary"].as_str().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        "artifact.read" => show(v),
        "artifact.tree" => ls(v),
        "artifact.render" => v["output"].as_str().unwrap_or("").to_owned(),
        "artifact.create" | "artifact.update" | "artifact.patch" | "artifact.write_file" => {
            format!("wrote {}  {}", s(v, "path"), s(v, "hash"))
        }
        "artifact.delete" => format!("deleted {}", s(v, "path")),
        _ => serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

/// Escape every control character (vault text is untrusted; a raw ESC would drive the terminal).
/// `multiline` keeps `\n` and `\t` for code bodies.
fn clean(s: &str, multiline: bool) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\t' if multiline => c.to_string(),
            c if c.is_control() => c.escape_default().to_string(),
            c => c.to_string(),
        })
        .collect()
}

fn s(v: &Value, k: &str) -> String {
    clean(v[k].as_str().unwrap_or(""), false)
}

fn code(v: &Value) -> String {
    clean(v["code"].as_str().unwrap_or(""), true)
}

fn vars(v: &Value) -> String {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|x| format!("{}={}", s(x, "name"), s(x, "defaultValue")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn show(v: &Value) -> String {
    let fm = &v["frontmatter"];
    let title = if fm["title"].is_string() {
        s(fm, "title")
    } else {
        s(v, "fileName")
    };
    let mut out = vec![format!(
        "{title}  [{}]  {}",
        s(fm, "artifactType"),
        s(v, "filePath")
    )];
    if let Some(d) = fm["description"].as_str() {
        out.push(clean(d, false));
    }
    if let Some(t) = fm["tags"].as_array().filter(|t| !t.is_empty()) {
        let t: Vec<String> = t
            .iter()
            .filter_map(Value::as_str)
            .map(|x| clean(x, false))
            .collect();
        out.push(format!("tags: {}", t.join(", ")));
    }
    let vs = vars(&v["vars"]);
    if !vs.is_empty() {
        out.push(format!("vars: {vs}"));
    }
    if let Some(e) = v.get("varsError") {
        out.push(format!(
            "varsError: {} line {}",
            s(e, "code"),
            s(&e["params"], "line")
        ));
    }
    out.push(String::new());
    match v["blocks"].as_array().filter(|b| !b.is_empty()) {
        None => out.push(code(v)),
        Some(bs) => {
            for b in bs {
                out.push(format!("## {}", s(b, "heading")));
                out.push(code(b));
            }
        }
    }
    out.join("\n")
}

fn ls(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    for d in v["dirs"].as_array().into_iter().flatten() {
        out.push(format!("{}/", clean(d.as_str().unwrap_or(""), false)));
    }
    for f in v["files"].as_array().into_iter().flatten() {
        out.push(match f.pointer("/error/code").and_then(Value::as_str) {
            Some(c) => format!("{}  ! {}", s(f, "name"), clean(c, false)),
            None => format!("{}  {}", s(f, "name"), s(f, "title")),
        });
    }
    out.join("\n")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests
mod tests {
    use super::success;
    use serde_json::json;

    #[test]
    fn write_printers() {
        let w = json!({"path": "Templates/n.md", "hash": "abc"});
        for op in ["artifact.create", "artifact.update", "artifact.patch"] {
            assert_eq!(success(op, &w), "wrote Templates/n.md  abc", "{op}");
        }
        assert_eq!(
            success("artifact.delete", &json!({"path": "Templates/n.md"})),
            "deleted Templates/n.md"
        );
    }

    // The only path vault text takes to the terminal on a write.
    #[test]
    fn write_printers_escape_controls() {
        let w = json!({"path": "Templates/e\u{1b}vil.md", "hash": "a\u{1b}b"});
        for op in ["artifact.create", "artifact.delete"] {
            assert!(!success(op, &w).contains('\u{1b}'), "{op}");
        }
    }
}
