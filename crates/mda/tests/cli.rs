//! CLI contract: ops map to subcommands, `--json` is the exact response, exit codes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use clap::CommandFactory;
use mda_ops::{Ctx, OpError, dispatch, error};
use serde_json::{Value, json};

mod common;
use common::{json_lines, run};

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn version_json_equals_dispatch() {
    let out = run(&["system", "version", "--json"], b"");
    assert_eq!(out.status.code(), Some(0), "{}", s(&out.stderr));
    let lines = json_lines(&out.stdout);
    assert_eq!(lines.len(), 1);
    let want = dispatch(&Ctx::new(None), "system.version", json!({})).unwrap();
    assert_eq!(lines[0], want);
}

#[test]
fn ops_lists_every_op_json() {
    let out = run(&["system", "ops", "--json"], b"");
    let v: Value = json_lines(&out.stdout).remove(0);
    let names: Vec<&str> = v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    for op in mda_ops::ops() {
        assert!(names.contains(&op.name), "{}", op.name);
    }
}

#[test]
fn unknown_subcommand_exits_2() {
    assert_eq!(run(&["nope"], b"").status.code(), Some(2));
}

#[test]
fn call_bad_request_json() {
    let out = run(&["call", "system.ops", r#"{"x":1}"#, "--json"], b"");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json_lines(&out.stdout)[0]["code"], "op.bad_request");
}

#[test]
fn call_unknown_human_stderr() {
    let out = run(&["call", "nope"], b"");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        s(&out.stderr).starts_with("op.unknown"),
        "{}",
        s(&out.stderr)
    );
}

#[test]
fn call_invalid_json_is_bad_request() {
    let out = run(&["call", "system.ops", "{", "--json"], b"");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json_lines(&out.stdout)[0]["code"], "op.bad_request");
}

#[test]
fn missing_vault_path_not_found() {
    let out = run(
        &["--vault", "/definitely/not/here/mda", "system", "ops"],
        b"",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        s(&out.stderr).starts_with("path.not_found"),
        "{}",
        s(&out.stderr)
    );
}

#[test]
fn help_and_version_exit_0() {
    assert_eq!(run(&["--help"], b"").status.code(), Some(0));
    assert_eq!(run(&["--version"], b"").status.code(), Some(0));
}

#[test]
fn exit_code_mapping() {
    assert_eq!(mda::cli::exit_code(&OpError::new(error::IO_FAILED)), 3);
    assert_eq!(mda::cli::exit_code(&OpError::new(error::OP_UNKNOWN)), 1);
}

#[test]
fn every_op_has_a_cli_path() {
    let root = mda::cli::Cli::command();
    for op in mda_ops::ops() {
        let mut cmd = &root;
        for seg in op.name.split('.') {
            let seg = seg.replace('_', "-");
            cmd = cmd
                .find_subcommand(&seg)
                .unwrap_or_else(|| panic!("{} has no CLI path at {seg}", op.name));
        }
    }
}

#[test]
fn every_code_has_english() {
    for c in error::ALL_CODES {
        assert!(mda::cli::print::english(c).is_some(), "{c}");
    }
}

#[test]
fn every_warning_has_english() {
    for c in error::RENDER_WARNING_CODES {
        assert!(mda::cli::print::english(c).is_some(), "{c}");
    }
}

#[test]
fn log_file_failure_is_an_op_error() {
    let log = std::env::temp_dir()
        .join(format!("mda-{}-nolog", std::process::id()))
        .join("no-such-dir/x.log");
    let log = log.to_str().unwrap();
    let out = run(
        &["--debug", "--json", "--log-file", log, "system", "ops"],
        b"",
    );
    assert_eq!(out.status.code(), Some(3));
    let lines = json_lines(&out.stdout);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["code"], "io.failed");
    let out = run(&["--debug", "--log-file", log, "system", "ops"], b"");
    assert_eq!(out.status.code(), Some(3));
    assert!(
        s(&out.stderr).starts_with("io.failed: "),
        "{}",
        s(&out.stderr)
    );
}
