//! Op cases as data: every `op_cases/<op>/<case>.toml` runs through `dispatch`, `mda call --json`
//! and a `serve` session. Adding a case needs no Rust. A coverage guard demands, per registered op,
//! one `expect` and one `expect_error` case. The case format and the legs are in `op_harness/`.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: tests fail loudly

mod common;
mod op_harness;

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::ops;
use op_harness::{Case, case_files, check_case, load, subst};
use serde_json::json;

fn real_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/op_cases")
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
fn error_params_mismatch_fails_every_leg() {
    let d = tmp("eparams");
    let case = |p: &str| {
        format!(
            "vault = \"vault\"\nparams = {{ path = \"x\" }}\nexpect_error = \"artifact.bad_path\"\n\
             expect_error_params = {{ path = \"{p}\" }}\n"
        )
    };
    put(&d, "artifact.read", "bad", &case("nope"));
    let fails = run_dir(&d);
    for leg in ["dispatch", "cli", "serve"] {
        let tag = format!("artifact.read/bad [{leg}]");
        assert!(fails.iter().any(|f| f.contains(&tag)), "{leg}: {fails:?}");
    }
    put(&d, "artifact.read", "bad", &case("x"));
    assert_eq!(run_dir(&d), Vec::<String>::new());
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
        subst(&v, &[("$WORKSPACE", "/w")]),
        json!({"a": "/w/x", "b": [{"c": "/w"}], "n": 1})
    );
}

#[test]
fn substitutes_vault_and_config() {
    let v = json!({"v": "$VAULT/x", "c": ["$CONFIG/config.toml"], "w": "$WORKSPACE"});
    let pairs = [("$CONFIG", "/c"), ("$VAULT", "/v"), ("$WORKSPACE", "/w")];
    assert_eq!(
        subst(&v, &pairs),
        json!({"v": "/v/x", "c": ["/c/config.toml"], "w": "/w"})
    );
}

#[test]
fn config_files_mismatch_fails_every_leg() {
    let d = tmp("cfgfiles");
    put(
        &d,
        "system.version",
        "cfg",
        "expect_error = \"op.unknown\"\nexpect_config_files = { \"config.toml\" = \"x\" }\n",
    );
    let fails = run_dir(&d);
    for leg in ["dispatch", "cli", "serve"] {
        let tag = format!("system.version/cfg [{leg}]: expect_config_files config.toml");
        assert!(fails.iter().any(|f| f.contains(&tag)), "{leg}: {fails:?}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn machine_config_reaches_every_leg() {
    let d = tmp("machine");
    put(
        &d,
        "config.get",
        "corrupt",
        "vault = \"vault\"\nvault_flag = false\nmachine_config = \"default = [\"\n\
         expect = { source = \"none\", configDir = \"$CONFIG\", defaultError = { code = \"config.invalid\", \
         params = { path = \"$CONFIG/config.toml\" } } }\n\
         expect_config_files = { \"config.toml\" = \"default = [\" }\n",
    );
    let fails = run_dir(&d);
    assert!(fails.is_empty(), "{}", fails.join("\n"));
    let _ = fs::remove_dir_all(&d);
}
