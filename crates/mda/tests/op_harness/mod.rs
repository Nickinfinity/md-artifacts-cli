//! The op-case harness (split from `op_cases.rs`, H4.0). Orchestrator-owned.
//!
//! A case file `op_cases/<op>/<case>.toml` runs through `dispatch`, `mda call --json` and a `serve`
//! session. Each leg runs on a **fresh copy** of the case's fixture vault, and `expect_files`
//! (vault-relative path → exact bytes, or `false` for absent) is asserted on that copy after the
//! leg (W-13). Hash literals in cases are `shasum -a 256` of fixture bytes. Note: `= false` on a
//! **directory** is vacuous (`fs::read` fails on a dir) — assert directories in-process.
//!
//! Workspace (R-24): a per-leg temp workspace when a case sets `workspace`,
//! `expect_workspace_files`, or writes `$WORKSPACE` in params; asserted by `expect_workspace_files`.
//!
//! Machine config (W4, V-16): every leg gets a fresh, **created**, canonical config dir, passed as
//! `MDA_CONFIG_DIR` (CLI, serve) or to the resolver (dispatch). `machine_config` is written to
//! `<cfg>/config.toml` before the leg; `expect_config_files` (cfg-relative) is asserted after it.
//! `vault_flag = false` copies the fixture (and sets `$VAULT`) without passing it as `--vault`.
//! `$VAULT` (canonical vault copy), `$CONFIG` (canonical config dir) and `$WORKSPACE` are replaced in
//! `params`, `expect`, `machine_config`, `expect_error_params` and every expected file's bytes.
//!
//! Error params (W7, S-23): `expect_error_params` (needs `expect_error`) is compared for equality
//! with the error's `params` (`data.params` on the wire) on every leg.
#![allow(
    dead_code,
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: test harness; fails loudly

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::error::{ALL_CODES, OpError};
use mda_ops::{dispatch, ops};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::common;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    #[serde(default = "empty")]
    pub params: Value,
    pub vault: Option<String>,
    #[serde(default = "yes")]
    pub vault_flag: bool,
    pub expect: Option<Value>,
    pub expect_error: Option<String>,
    #[serde(default)]
    pub expect_error_params: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub expect_files: BTreeMap<String, FileExpect>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub expect_workspace_files: BTreeMap<String, FileExpect>,
    #[serde(default)]
    pub machine_config: Option<String>,
    #[serde(default)]
    pub expect_config_files: BTreeMap<String, FileExpect>,
}

/// One expected-file entry: the file's exact bytes, or `false` (the file must not exist).
#[derive(Deserialize)]
#[serde(untagged)]
pub enum FileExpect {
    Bytes(String),
    Absent(bool),
}

fn empty() -> Value {
    json!({})
}

fn yes() -> bool {
    true
}

/// A fixture under `tests/fixtures/`.
pub fn fixture(v: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(v)
}

/// Sorted `.toml` files in `dir` (empty if it does not exist).
pub fn case_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|x| x == "toml"));
    v.sort();
    v
}

pub fn load(path: &Path) -> Result<Case, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let case: Case = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if case
        .expect_files
        .values()
        .chain(case.expect_workspace_files.values())
        .chain(case.expect_config_files.values())
        .any(|f| matches!(f, FileExpect::Absent(true)))
    {
        return Err(format!(
            "{}: expected files take bytes or false",
            path.display()
        ));
    }
    if case.expect_error_params.is_some() && case.expect_error.is_none() {
        return Err(format!(
            "{}: expect_error_params needs expect_error",
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

/// The per-leg environment: the vault copy, whether it is passed as the flag, the config dir.
struct Leg<'a> {
    vault: Option<&'a Path>,
    flag: bool,
    cfg: &'a Path,
}

impl Leg<'_> {
    fn flag(&self) -> Option<&Path> {
        self.vault.filter(|_| self.flag)
    }

    /// `--vault <abs>` when the vault is passed as the flag.
    fn args(&self) -> Vec<String> {
        self.flag()
            .map(|v| vec!["--vault".to_owned(), v.display().to_string()])
            .unwrap_or_default()
    }
}

fn err_seen(e: &OpError) -> Seen {
    Seen {
        body: serde_json::to_value(e).unwrap(),
        is_err: true,
        exit: None,
    }
}

fn leg_dispatch(op: &str, params: &Value, leg: &Leg) -> Seen {
    let ctx = match mda_ops::vault::resolve(leg.flag(), Some(leg.cfg.to_path_buf())) {
        Ok(c) => c,
        Err(e) => return err_seen(&e),
    };
    match dispatch(&ctx, op, params.clone()) {
        Ok(body) => Seen {
            body,
            is_err: false,
            exit: None,
        },
        Err(e) => err_seen(&e),
    }
}

fn leg_cli(op: &str, params: &Value, leg: &Leg) -> Seen {
    let mut args = leg.args();
    args.extend([
        "call".to_owned(),
        op.to_owned(),
        params.to_string(),
        "--json".to_owned(),
    ]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = common::run_cfg(&args, b"", leg.cfg);
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

fn leg_serve(op: &str, params: &Value, leg: &Leg) -> Seen {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"1.0"}}"#;
    let req = json!({"jsonrpc":"2.0","id":2,"method":op,"params":params}).to_string();
    let mut args = leg.args();
    args.push("serve".to_owned());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let input = format!("{init}\n{req}\n");
    let out = common::run_cfg(&args, input.as_bytes(), leg.cfg);
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

/// A fresh temp dir for one leg (`kind` = vault copy, `ws`, `cfg`), canonical.
fn leg_dir(op: &str, name: &str, leg: &str, kind: &str, seed: Option<&Path>) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "mda-oc-{}-{op}-{name}-{leg}{kind}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    match seed {
        Some(s) => common::copy_tree(s, &d),
        None => fs::create_dir_all(&d).unwrap(),
    }
    d.canonicalize().unwrap()
}

/// Failure strings (each naming case and leg) for one case.
pub fn check_case(op: &str, name: &str, case: &Case) -> Vec<String> {
    if !ops().iter().any(|o| o.name == op) {
        return vec![format!("{op}/{name}: op.unknown (no such registered op)")];
    }
    let want_exit = case.expect_error.as_deref().map(|c| {
        let code = ALL_CODES.iter().find(|a| **a == c);
        code.map_or(-1, |c| i32::from(mda::cli::exit_code(&OpError::new(c))))
    });
    let mut fails = Vec::new();
    for leg in ["dispatch", "cli", "serve"] {
        let copy = case
            .vault
            .as_deref()
            .map(|v| leg_dir(op, name, leg, "", Some(&fixture(v))));
        let ws = workspace_for(op, name, leg, case);
        let cfg = leg_dir(op, name, leg, "-cfg", None);
        let s = |p: &Option<PathBuf>| p.as_ref().map(|p| p.display().to_string());
        let (vs, ws_s, cs) = (s(&copy), s(&ws), cfg.display().to_string());
        let mut pairs: Vec<(&str, &str)> = vec![("$CONFIG", &cs)];
        pairs.extend(vs.as_deref().map(|v| ("$VAULT", v)));
        pairs.extend(ws_s.as_deref().map(|w| ("$WORKSPACE", w)));
        if let Some(m) = &case.machine_config {
            fs::write(cfg.join("config.toml"), subst_str(m, &pairs)).unwrap();
        }
        let params = subst(&case.params, &pairs);
        let env = Leg {
            vault: copy.as_deref(),
            flag: case.vault_flag,
            cfg: &cfg,
        };
        let seen = match leg {
            "dispatch" => leg_dispatch(op, &params, &env),
            "cli" => leg_cli(op, &params, &env),
            _ => leg_serve(op, &params, &env),
        };
        let expect = case.expect.as_ref().map(|e| subst(e, &pairs));
        let ok = match (&expect, &case.expect_error) {
            (Some(e), _) => !seen.is_err && seen.body == *e && seen.exit.is_none_or(|x| x == 0),
            (_, Some(c)) => {
                seen.is_err
                    && seen.body["code"] == *c
                    && ALL_CODES.contains(&c.as_str())
                    && seen.exit.is_none_or(|x| Some(x) == want_exit)
                    && case.expect_error_params.as_ref().is_none_or(|p| {
                        let want: BTreeMap<_, _> = p
                            .iter()
                            .map(|(k, v)| (k.clone(), subst_str(v, &pairs)))
                            .collect();
                        seen.body["params"] == json!(want)
                    })
            }
            _ => false,
        };
        if !ok {
            fails.push(format!(
                "{op}/{name} [{leg}]: got {} (exit {:?}), want {:?}{:?}{:?}",
                seen.body, seen.exit, expect, case.expect_error, case.expect_error_params
            ));
        }
        let tag = format!("{op}/{name} [{leg}]");
        for (label, map, dir) in [
            ("expect_files", &case.expect_files, copy.as_deref()),
            (
                "expect_workspace_files",
                &case.expect_workspace_files,
                ws.as_deref(),
            ),
            (
                "expect_config_files",
                &case.expect_config_files,
                Some(cfg.as_path()),
            ),
        ] {
            fails.extend(check_files(&tag, label, map, dir, &pairs));
        }
        for d in [copy, ws, Some(cfg)].into_iter().flatten() {
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
    let seed = case.workspace.as_deref().map(fixture);
    Some(leg_dir(op, name, leg, "-ws", seed.as_deref()))
}

/// `s` with every placeholder replaced.
pub fn subst_str(s: &str, pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .fold(s.to_owned(), |acc, (k, v)| acc.replace(k, v))
}

/// `v` with every placeholder replaced in every JSON string, at any depth.
pub fn subst(v: &Value, pairs: &[(&str, &str)]) -> Value {
    match v {
        Value::String(s) => Value::String(subst_str(s, pairs)),
        Value::Array(a) => Value::Array(a.iter().map(|x| subst(x, pairs)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| (k.clone(), subst(x, pairs)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// A file-expectation sink (`label` = which map), asserted on `dir`.
fn check_files(
    tag: &str,
    label: &str,
    map: &BTreeMap<String, FileExpect>,
    dir: Option<&Path>,
    pairs: &[(&str, &str)],
) -> Vec<String> {
    let mut fails = Vec::new();
    for (rel, want) in map {
        let got = dir.and_then(|v| fs::read(v.join(rel)).ok());
        let ok = match want {
            FileExpect::Bytes(b) => got.as_deref() == Some(subst_str(b, pairs).as_bytes()),
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
