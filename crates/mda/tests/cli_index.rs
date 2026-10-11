//! `mda index` argument handling through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{bin, json_lines, run};
use std::process::Stdio;

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-idx-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn steps_from_stdin_write_into_workspace() {
    let ws = tmpdir("stdin");
    let out = run(
        &[
            "--vault",
            VAULT,
            "index",
            "run",
            "Templates/index.md",
            "--workspace",
            ws.to_str().unwrap(),
            "--steps",
            "-",
            "--json",
        ],
        br#"[{"relPath":"edit.md","destDir":""}]"#,
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert_eq!(json_lines(&out.stdout)[0]["written"], 1);
    assert!(ws.join("Edit me.ts").is_file());
    std::fs::remove_dir_all(ws).unwrap();
}

#[test]
fn relative_workspace_resolves_against_cwd() {
    let ws = tmpdir("rel");
    let out = bin()
        .current_dir(&ws)
        .args(["--vault", VAULT, "index", "run", "Templates/index.md"])
        .args(["--workspace", ".", "--steps", "-", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(
        &mut out.stdin.as_ref().unwrap(),
        br#"[{"relPath":"edit.md","destDir":""}]"#,
    )
    .unwrap();
    let out = out.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert!(ws.join("Edit me.ts").is_file());
    std::fs::remove_dir_all(ws).unwrap();
}

#[test]
fn bad_arguments() {
    let ws = tmpdir("bad");
    let w = ws.to_str().unwrap();
    let base = [
        "--vault",
        VAULT,
        "index",
        "run",
        "Templates/index.md",
        "--workspace",
        w,
    ];
    let out = run(&base, b"");
    assert_eq!(out.status.code(), Some(2), "missing --steps");
    let mut a = base.to_vec();
    a.extend(["--steps", "-"]);
    let out = run(&a, b"{}");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("op.bad_request"));
    let out = run(
        &[
            "--vault",
            VAULT,
            "index",
            "create",
            "--type",
            "Template",
            "--dir",
            "Templates/g",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(2), "create without paths");
    std::fs::remove_dir_all(ws).unwrap();
}
