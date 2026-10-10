//! Op cases as data: every `op_cases/<op>/<case>.toml` runs through `dispatch`, `mda call --json`
//! and a `serve` session. Adding a case needs no Rust. A coverage guard demands, per registered op,
//! one `expect` and one `expect_error` case.
//!
//! Each leg runs on a **fresh copy** of the case's fixture vault, and `expect_files` (vault-relative
//! path → exact bytes, or `false` for absent) is asserted on that copy after the leg: the sink of
//! every write case (W-13). Hash literals in cases are `shasum -a 256` of fixture bytes; regenerate
//! them when a fixture file changes.
//!
//! Ops that write outside the vault (`artifact.write_file`) get a **per-leg temp workspace** when a
//! case sets `workspace` (a fixture under `tests/fixtures/` copied in), `expect_workspace_files`, or
//! writes `$WORKSPACE` in a params string: every `$WORKSPACE` in `params` becomes that dir's path,
//! and `expect_workspace_files` (workspace-relative paths) is asserted on it after the leg (R-24).
//! Paths inside `expect` are workspace-relative, so no machine path appears in a case.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: tests fail loudly

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::error::{ALL_CODES, OpError};
use mda_ops::{Ctx, Root, dispatch, ops};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    #[serde(default = "empty")]
    params: Value,
    vault: Option<String>,
    expect: Option<Value>,
    expect_error: Option<String>,
    #[serde(default)]
    expect_files: BTreeMap<String, FileExpect>,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    expect_workspace_files: BTreeMap<String, FileExpect>,
}

/// One `expect_files` entry: the file's exact bytes, or `false` (the file must not exist).
#[derive(Deserialize)]
#[serde(untagged)]
enum FileExpect {
    Bytes(String),
    Absent(bool),
}

fn empty() -> Value {
    json!({})
}

/// A fixture vault under `tests/fixtures/`.
fn fixture(v: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(v)
}

fn real_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/op_cases")
}

/// Sorted `.toml` files in `dir` (empty if it does not exist).
fn case_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|x| x == "toml"));
    v.sort();
    v
}

fn load(path: &Path) -> Result<Case, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let case: Case = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if case
        .expect_files
        .values()
        .chain(case.expect_workspace_files.values())
        .any(|f| matches!(f, FileExpect::Absent(true)))
    {
        return Err(format!(
            "{}: expect_files / expect_workspace_files take bytes or false",
            path.display()
        ));
    }
    if case.expect.is_some() == case.expect_error.is_some() {
        return Err(format!(
            "{}: need exactly one of expect, expect_error",
            path.display()
        ));
    }
    Ok(case)
}

/// What a leg observed: the response, or the error JSON plus (for the binary) the exit code.
struct Seen {
    body: Value,
    is_err: bool,
    exit: Option<i32>,
}

fn leg_dispatch(op: &str, params: &Value, vault: Option<&Path>) -> Seen {
    let ctx = Ctx::new(vault.map(|v| Root::new(v).unwrap()));
    match dispatch(&ctx, op, params.clone()) {
        Ok(body) => Seen {
            body,
            is_err: false,
            exit: None,
        },
        Err(e) => Seen {
            body: serde_json::to_value(&e).unwrap(),
            is_err: true,
            exit: None,
        },
    }
}

/// `--vault <abs>` for a case with a fixture vault, nothing otherwise.
fn vault_args(vault: Option<&Path>) -> Vec<String> {
    vault
        .map(|v| vec!["--vault".to_owned(), v.display().to_string()])
        .unwrap_or_default()
}

fn leg_cli(op: &str, params: &Value, vault: Option<&Path>) -> Seen {
    let mut args = vault_args(vault);
    args.extend([
        "call".to_owned(),
        op.to_owned(),
        params.to_string(),
        "--json".to_owned(),
    ]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = common::run(&args, b"");
    let exit = out.status.code();
    let body = common::json_lines(&out.stdout)
        .into_iter()
        .next()
        .unwrap_or(Value::Null);
    Seen {
        body,
        is_err: exit != Some(0),
        exit,
    }
}

fn leg_serve(op: &str, params: &Value, vault: Option<&Path>) -> Seen {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"1.0"}}"#;
    let req = json!({"jsonrpc":"2.0","id":2,"method":op,"params":params}).to_string();
    let args = vault_args(vault);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = common::serve_session(&args, &[init, &req]);
    let lines = common::json_lines(&out.stdout);
    let res = lines.get(1).cloned().unwrap_or(Value::Null);
    match res.get("error") {
        Some(err) => Seen {
            body: err["data"].clone(),
            is_err: true,
            exit: None,
        },
        None => Seen {
            body: res["result"].clone(),
            is_err: false,
            exit: None,
        },
    }
}

/// Failure strings (each naming case and leg) for one case.
fn check_case(op: &str, name: &str, case: &Case) -> Vec<String> {
    if !ops().iter().any(|o| o.name == op) {
        return vec![format!("{op}/{name}: op.unknown (no such registered op)")];
    }
    let want_exit = case.expect_error.as_deref().map(|c| {
        let code = ALL_CODES.iter().find(|a| **a == c);
        code.map_or(-1, |c| i32::from(mda::cli::exit_code(&OpError::new(c))))
    });
    let mut fails = Vec::new();
    for leg in ["dispatch", "cli", "serve"] {
        let copy = case.vault.as_deref().map(|v| {
            let d = std::env::temp_dir()
                .join(format!("mda-oc-{}-{op}-{name}-{leg}", std::process::id()));
            let _ = fs::remove_dir_all(&d);
            common::copy_tree(&fixture(v), &d);
            d
        });
        let ws = workspace_for(op, name, leg, case);
        let params = match &ws {
            Some(w) => subst(&case.params, &w.display().to_string()),
            None => case.params.clone(),
        };
        let vault = copy.as_deref();
        let seen = match leg {
            "dispatch" => leg_dispatch(op, &params, vault),
            "cli" => leg_cli(op, &params, vault),
            _ => leg_serve(op, &params, vault),
        };
        let ok = match (&case.expect, &case.expect_error) {
            (Some(e), _) => !seen.is_err && seen.body == *e && seen.exit.is_none_or(|x| x == 0),
            (_, Some(c)) => {
                seen.is_err
                    && seen.body["code"] == *c
                    && ALL_CODES.contains(&c.as_str())
                    && seen.exit.is_none_or(|x| Some(x) == want_exit)
            }
            _ => false,
        };
        if !ok {
            fails.push(format!(
                "{op}/{name} [{leg}]: got {} (exit {:?}), want {:?}{:?}",
                seen.body, seen.exit, case.expect, case.expect_error
            ));
        }
        let tag = format!("{op}/{name} [{leg}]");
        fails.extend(check_files(
            &tag,
            "expect_files",
            &case.expect_files,
            copy.as_deref(),
        ));
        fails.extend(check_files(
            &tag,
            "expect_workspace_files",
            &case.expect_workspace_files,
            ws.as_deref(),
        ));
        for d in [copy, ws].into_iter().flatten() {
            let _ = fs::remove_dir_all(d);
        }
    }
    fails
}

/// A fresh per-leg workspace dir when the case uses one (seeded from `workspace` if set).
fn workspace_for(op: &str, name: &str, leg: &str, case: &Case) -> Option<PathBuf> {
    let wanted = case.workspace.is_some()
        || !case.expect_workspace_files.is_empty()
        || case.params.to_string().contains("$WORKSPACE");
    if !wanted {
        return None;
    }
    let d = std::env::temp_dir().join(format!(
        "mda-oc-{}-{op}-{name}-{leg}-ws",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    match &case.workspace {
        Some(w) => common::copy_tree(&fixture(w), &d),
        None => fs::create_dir_all(&d).unwrap(),
    }
    Some(d)
}

/// `v` with `$WORKSPACE` replaced by `ws` in every JSON string, at any depth.
fn subst(v: &Value, ws: &str) -> Value {
    match v {
        Value::String(s) => Value::String(s.replace("$WORKSPACE", ws)),
        Value::Array(a) => Value::Array(a.iter().map(|x| subst(x, ws)).collect()),
        Value::Object(m) => {
            Value::Object(m.iter().map(|(k, x)| (k.clone(), subst(x, ws))).collect())
        }
        other => other.clone(),
    }
}

/// A file-expectation sink (`label` = which map), asserted on `dir` (the leg's vault copy or
/// workspace).
fn check_files(
    tag: &str,
    label: &str,
    map: &BTreeMap<String, FileExpect>,
    dir: Option<&Path>,
) -> Vec<String> {
    let mut fails = Vec::new();
    for (rel, want) in map {
        let got = dir.and_then(|v| fs::read(v.join(rel)).ok());
        let ok = match want {
            FileExpect::Bytes(b) => got.as_deref() == Some(b.as_bytes()),
            FileExpect::Absent(_) => got.is_none(),
        };
        if !ok {
            fails.push(format!(
                "{tag}: {label} {rel}: got {:?}",
                got.map(|g| String::from_utf8_lossy(&g).into_owned())
            ));
        }
    }
    fails
}

/// Run every case under `dir`; an unloadable case is a failure too.
fn run_dir(dir: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    for op_dir in case_files_dirs(dir) {
        let op = op_dir.file_name().unwrap().to_string_lossy().into_owned();
        for f in case_files(&op_dir) {
            let name = f.file_stem().unwrap().to_string_lossy().into_owned();
            match load(&f) {
                Ok(c) => fails.extend(check_case(&op, &name, &c)),
                Err(e) => fails.push(e),
            }
        }
    }
    fails
}

fn case_files_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.is_dir());
    v.sort();
    v
}

/// Per op: names of the `expect` / `expect_error` case kinds that have no file in `dir/<op>/`.
fn missing_cases(ops: &[&str], dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for op in ops {
        let cases: Vec<Case> = case_files(&dir.join(op))
            .iter()
            .filter_map(|f| load(f).ok())
            .collect();
        if !cases.iter().any(|c| c.expect.is_some()) {
            out.push(format!("{op}: no expect case"));
        }
        if !cases.iter().any(|c| c.expect_error.is_some()) {
            out.push(format!("{op}: no expect_error case"));
        }
    }
    out
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn put(dir: &Path, op: &str, case: &str, body: &str) {
    fs::create_dir_all(dir.join(op)).unwrap();
    fs::write(dir.join(op).join(format!("{case}.toml")), body).unwrap();
}

#[test]
fn wrong_expect_fails_every_leg_by_name() {
    let d = tmp("wrong");
    put(
        &d,
        "system.version",
        "bad",
        "expect = { engine = \"nope\" }\n",
    );
    let fails = run_dir(&d);
    for leg in ["dispatch", "cli", "serve"] {
        let tag = format!("system.version/bad [{leg}]");
        assert!(fails.iter().any(|f| f.contains(&tag)), "{leg}: {fails:?}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn unregistered_op_dir_fails_op_unknown() {
    let d = tmp("unknown");
    put(&d, "no.such_op", "x", "expect = { a = 1 }\n");
    let fails = run_dir(&d);
    assert!(fails.iter().any(|f| f.contains("op.unknown")), "{fails:?}");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn guard_names_missing_error_case() {
    let d = tmp("guard");
    put(&d, "system.version", "ok", "expect = { a = 1 }\n");
    let m = missing_cases(&["system.version"], &d);
    assert_eq!(m, vec!["system.version: no expect_error case".to_string()]);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn real_ops_are_covered() {
    let names: Vec<&str> = ops().iter().map(|o| o.name).collect();
    assert_eq!(missing_cases(&names, &real_dir()), Vec::<String>::new());
}

#[test]
fn all_real_cases_pass_all_legs() {
    let fails = run_dir(&real_dir());
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn expect_files_mismatch_fails_every_leg() {
    let d = tmp("files");
    put(
        &d,
        "artifact.read",
        "files",
        "vault = \"vault\"\nparams = { path = \"x\" }\nexpect_error = \"artifact.bad_path\"\n\
         expect_files = { \"Snippets/hello.md\" = false }\n",
    );
    let fails = run_dir(&d);
    for leg in ["dispatch", "cli", "serve"] {
        let tag = format!("artifact.read/files [{leg}]: expect_files Snippets/hello.md");
        assert!(fails.iter().any(|f| f.contains(&tag)), "{leg}: {fails:?}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn expect_workspace_files_mismatch_fails_every_leg() {
    let d = tmp("wsfiles");
    put(
        &d,
        "artifact.read",
        "ws",
        "vault = \"vault\"\nparams = { path = \"x\" }\nexpect_error = \"artifact.bad_path\"\n\
         expect_workspace_files = { \"x\" = \"y\" }\n",
    );
    let fails = run_dir(&d);
    for leg in ["dispatch", "cli", "serve"] {
        let tag = format!("artifact.read/ws [{leg}]: expect_workspace_files x");
        assert!(fails.iter().any(|f| f.contains(&tag)), "{leg}: {fails:?}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn substitutes_workspace_in_nested_strings() {
    let v = json!({"a": "$WORKSPACE/x", "b": [{"c": "$WORKSPACE"}], "n": 1});
    assert_eq!(
        subst(&v, "/w"),
        json!({"a": "/w/x", "b": [{"c": "/w"}], "n": 1})
    );
}
