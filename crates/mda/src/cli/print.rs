//! Human rendering: per-op success printers (the English error table lives in `english.rs`).

use mda_ops::OpError;
use serde_json::Value;

pub use super::english::english;

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
/// a terminal control, `containsEscape`); `None` to print it.
///
/// # Examples
///
/// ```
/// let _ = mda::cli::print::refuse("system.ops", &serde_json::json!({}));
/// ```
pub fn refuse(op: &str, v: &Value) -> Option<OpError> {
    // write_file output goes to a file, so only render can reach the terminal.
    (matches!(op, "artifact.render" | "render.text") && v["containsEscape"] == true)
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
    let top = v["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| (None, w));
    // Batch responses (index run/create) carry warnings per result, tagged with the result's relPath.
    let per_result = v["results"].as_array().into_iter().flatten().flat_map(|r| {
        let rel = r["relPath"].as_str();
        r["warnings"]
            .as_array()
            .into_iter()
            .flatten()
            .map(move |w| (rel, w))
    });
    top.chain(per_result)
        .map(|(rel, w)| {
            let line = coded_value(w["code"].as_str().unwrap_or("?"), &w["params"]);
            match rel {
                Some(r) => format!("warning: {r:?}: {line}"),
                None => format!("warning: {line}"),
            }
        })
        .collect()
}

/// `coded_line` over a JSON `params` object.
pub(super) fn coded_value(code: &str, params: &Value) -> String {
    let params: std::collections::BTreeMap<String, String> = params
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, x)| (k.clone(), x.as_str().unwrap_or("").to_owned()))
        .collect();
    coded_line(code, params.iter())
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
        "artifact.render" | "render.text" => v["output"].as_str().unwrap_or("").to_owned(),
        op if ["vars.file.", "vars.meta.", "vars.subset.", "vars.var."]
            .iter()
            .any(|p| op.starts_with(p))
            || matches!(
                op,
                "artifact.create"
                    | "artifact.update"
                    | "artifact.patch"
                    | "artifact.write_file"
                    | "varsets.save"
            ) =>
        {
            format!("wrote {}  {}", s(v, "path"), s(v, "hash"))
        }
        "artifact.rename" => {
            let mut out = format!("wrote {}  {}", s(v, "path"), s(v, "hash"));
            for i in v["updatedIndexes"].as_array().into_iter().flatten() {
                out.push_str(&format!("\nupdated {}  {}", s(i, "path"), s(i, "hash")));
            }
            out
        }
        "artifact.delete" => format!("deleted {}", s(v, "path")),
        "naming.output" => s(v, "fileName"),
        // The name is client text: escaped, and a refusal carries its code and English.
        "naming.check" => match v["error"].as_object() {
            None => s(v, "fileName"),
            Some(e) => format!(
                "{}: {}",
                s(v, "fileName"),
                coded_value(e["code"].as_str().unwrap_or("?"), &e["params"])
            ),
        },
        "vars.list" | "vars.search" | "varsets.match" | "varsets.apply" => {
            super::varsets::success(op, v).unwrap_or_default()
        }
        "index.plan" => plan(v),
        "index.run" | "index.create" => batch(v),
        op if op.starts_with("migrate.") || op == "vault.validate" => {
            super::migrate::success(op, v).unwrap_or_default()
        }
        op if op.starts_with("vault.") || op.starts_with("types.") || op == "config.get" => {
            super::vault::success(op, v).unwrap_or_default()
        }
        _ => serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

/// Escape every control character (vault text is untrusted; a raw ESC would drive the terminal).
/// `multiline` keeps `\n` and `\t` for code bodies.
pub(super) fn clean(s: &str, multiline: bool) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\t' if multiline => c.to_string(),
            c if c.is_control() => c.escape_default().to_string(),
            c => c.to_string(),
        })
        .collect()
}

pub(super) fn s(v: &Value, k: &str) -> String {
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

// Link text and relPaths are vault-authored: always `{:?}` (escapes ESC).
fn plan(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    for st in v["steps"].as_array().into_iter().flatten() {
        let rel = st["relPath"].as_str().unwrap_or("");
        let dest = st
            .pointer("/candidates/0")
            .and_then(Value::as_str)
            .unwrap_or("");
        let mut line = format!("{rel:?}  -> {dest:?}");
        if let Some(c) = st.pointer("/skip/code").and_then(Value::as_str) {
            line.push_str(&format!("  skip {}", clean(c, false)));
        }
        out.push(line);
    }
    for r in v["rejected"].as_array().into_iter().flatten() {
        out.push(format!(
            "rejected {:?}  {}",
            r["raw"].as_str().unwrap_or(""),
            s(r, "code")
        ));
    }
    out.join("\n")
}

fn batch(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    for r in v["results"].as_array().into_iter().flatten() {
        out.push(if r["status"] == "written" {
            format!("wrote {}  {}", s(r, "path"), s(r, "hash"))
        } else {
            let line = coded_value(r["code"].as_str().unwrap_or("?"), &r["params"]);
            format!("skipped {:?}  {line}", r["relPath"].as_str().unwrap_or(""))
        });
    }
    out.push(format!(
        "{} written, {} skipped",
        v["written"].as_u64().unwrap_or(0),
        v["skipped"].as_u64().unwrap_or(0)
    ));
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
    fn vars_write_prints_wrote() {
        let w = json!({"path": "Variables/flat.md", "hash": "h"});
        assert_eq!(success("vars.var.set", &w), "wrote Variables/flat.md  h");
        assert_eq!(success("vars.file.new", &w), "wrote Variables/flat.md  h");
        assert_ne!(success("vars.list", &w), "wrote Variables/flat.md  h");
    }

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

    #[test]
    fn varsets_save_prints_wrote() {
        let w = json!({"path": "Variables/local-dev.md", "hash": "h"});
        assert_eq!(
            success("varsets.save", &w),
            "wrote Variables/local-dev.md  h"
        );
    }

    #[test]
    fn render_text_refused_with_esc() {
        let r = json!({"output": "x", "containsEscape": true});
        assert!(super::refuse("render.text", &r).is_some());
        assert!(super::refuse("render.text", &json!({"containsEscape": false})).is_none());
        assert_eq!(success("render.text", &json!({"output": "hi x"})), "hi x");
    }

    #[test]
    fn list_escapes_vault_text() {
        let v = json!({"files": [{"path": "Variables/e.md", "name": "e", "title": "a\u{1b}[31mb",
            "shape": "sets", "tags": ["t\u{1b}x"],
            "subsets": [{"index": 0, "heading": "h\u{1b}h", "description": "d\u{1b}d",
                "vars": [{"name": "VK-a", "defaultValue": "v\u{1b}v"}]}]}]});
        let out = success("vars.list", &v);
        assert!(out.contains("Variables/e.md"), "{out:?}");
        assert!(!out.contains('\u{1b}'), "{out:?}");
    }
}
