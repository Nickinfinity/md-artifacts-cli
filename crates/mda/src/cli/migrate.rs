//! `mda migrate frontmatter plan|apply` and the migrate / `vault.validate` human printers.

use super::print::{clean, coded_value, s};
use mda_ops::OpError;
use serde_json::{Value, json};

/// `mda migrate …`
#[derive(clap::Subcommand)]
pub enum MigrateCmd {
    /// `type:` → `artifactType:` in frontmatter.
    #[command(subcommand)]
    Frontmatter(Step),
}

/// One migration step.
#[derive(clap::Subcommand)]
pub enum Step {
    /// List what would change (writes nothing).
    Plan,
    /// Plan, then rewrite every planned file (hash-checked).
    Apply,
}

/// The op and params a `migrate` subcommand stands for. Human `apply` plans first, in this
/// process, and applies every planned `{path, hash}` (M-6).
pub(super) fn migrate_request(
    cmd: MigrateCmd,
    ctx: &mda_ops::Ctx,
) -> (String, Result<Value, OpError>) {
    let (kind, step) = match cmd {
        MigrateCmd::Frontmatter(s) => ("frontmatter", s),
    };
    let plan = format!("migrate.{kind}.plan");
    match step {
        Step::Plan => (plan, Ok(json!({}))),
        Step::Apply => {
            let apply = format!("migrate.{kind}.apply");
            match mda_ops::dispatch(ctx, &plan, json!({})) {
                Err(e) => (apply, Err(e)),
                Ok(p) => {
                    let files: Vec<Value> = p
                        .get("changes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|c| json!({"path": c["path"], "hash": c["hash"]}))
                        .collect();
                    (apply, Ok(json!({ "files": files })))
                }
            }
        }
    }
}

/// Human output for the migrate ops and `vault.validate`; `None` falls back to pretty JSON.
pub(super) fn success(op: &str, v: &Value) -> Option<String> {
    Some(match op {
        "migrate.frontmatter.plan" => plan(v),
        "migrate.frontmatter.apply" => apply(v),
        "vault.validate" => validate(v),
        _ => return None,
    })
}

fn normalises(c: &Value) -> String {
    let names: Vec<&str> = c["normalises"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!("  (normalises {})", names.join(", "))
    }
}

fn plan(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    let changes = v["changes"].as_array().map_or(&[][..], Vec::as_slice);
    for c in changes {
        let head = format!(
            "{:?} -> {:?}",
            clean(c["oldLine"].as_str().unwrap_or(""), false),
            clean(c["newLine"].as_str().unwrap_or(""), false)
        );
        out.push(format!("{}  {head}{}", s(c, "path"), normalises(c)));
    }
    let skipped = v["skipped"].as_array().map_or(&[][..], Vec::as_slice);
    for k in skipped {
        out.push(format!(
            "skipped {}  {}",
            s(k, "path"),
            coded_value(k["code"].as_str().unwrap_or("?"), &k["params"])
        ));
    }
    out.push(format!(
        "{} to change, {} skipped",
        changes.len(),
        skipped.len()
    ));
    out.join("\n")
}

fn apply(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    for r in v["results"].as_array().into_iter().flatten() {
        out.push(if r["status"] == "written" {
            format!("wrote {}  {}", s(r, "path"), s(r, "hash"))
        } else {
            let line = coded_value(r["code"].as_str().unwrap_or("?"), &r["params"]);
            format!("skipped {}  {line}", s(r, "path"))
        });
    }
    out.push(format!(
        "{} written, {} skipped",
        v["written"].as_u64().unwrap_or(0),
        v["skipped"].as_u64().unwrap_or(0)
    ));
    out.join("\n")
}

fn validate(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    for i in v["issues"].as_array().into_iter().flatten() {
        out.push(format!(
            "{}  {}  {}",
            s(i, "severity"),
            s(i, "path"),
            coded_value(i["code"].as_str().unwrap_or("?"), &i["params"])
        ));
    }
    let cut = if v["truncated"] == true {
        "  (first 10000 issues shown)"
    } else {
        ""
    };
    out.push(format!(
        "{} files, {} errors, {} warnings{cut}",
        v["files"], v["errors"], v["warnings"]
    ));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_exit_maps_validate_errors() {
        assert_eq!(
            super::super::report_exit("vault.validate", &json!({"errors": 1})),
            1
        );
        let ok = json!({"errors": 0, "warnings": 3});
        assert_eq!(super::super::report_exit("vault.validate", &ok), 0);
        assert_eq!(
            super::super::report_exit("migrate.frontmatter.plan", &json!({"errors": 5})),
            0
        );
    }

    #[test]
    fn printer_escapes_esc() {
        let v = json!({"changes": [{"path": "Snippets/e\u{1b}x.md", "hash": "h",
            "oldLine": "type: a\u{1b}[31m", "newLine": "artifactType: Snippet",
            "artifactType": "Snippet"}], "skipped": []});
        let out = success("migrate.frontmatter.plan", &v).unwrap_or_default();
        assert!(!out.contains('\u{1b}'), "{out:?}");
        assert!(out.ends_with("1 to change, 0 skipped"), "{out:?}");
    }
}
