//! CLI input paths of the W3 commands: stdin, files, relative workspace, usage errors.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::{bin, json_lines, run};

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-cr-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn values_from_stdin() {
    let out = run(
        &[
            "--vault",
            VAULT,
            "artifact",
            "render",
            "Snippets/hello.md",
            "--values",
            "-",
        ],
        br#"{"VK-name":"zed"}"#,
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "echo marker-7f3a zed\n"
    );
}

#[test]
fn code_file_replaces_code() {
    let d = tmp("code");
    let f = d.join("c.txt");
    std::fs::write(&f, "hi <VK-greeting>").unwrap();
    let out = run(
        &[
            "--vault",
            VAULT,
            "artifact",
            "render",
            "Snippets/hello.md",
            "--code-file",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "hi marker-7f3a\n");
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn write_file_resolves_relative_workspace() {
    let d = tmp("ws");
    let out = bin()
        .current_dir(&d)
        .args([
            "--vault",
            VAULT,
            "artifact",
            "write-file",
            "Templates/component.md",
            "--workspace",
            "ws",
            "--dest",
            "out",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    // workspace dir must exist for Root::new
    assert_eq!(
        out.status.code(),
        Some(1),
        "missing workspace is path.not_found"
    );
    std::fs::create_dir_all(d.join("ws")).unwrap();
    let out = bin()
        .current_dir(&d)
        .args([
            "--vault",
            VAULT,
            "artifact",
            "write-file",
            "Templates/component.md",
            "--workspace",
            "ws",
            "--dest",
            "out",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("wrote out/Button.tsx  ")
    );
    assert!(d.join("ws/out/Button.tsx").is_file());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn write_file_without_workspace_is_usage_error() {
    let out = run(
        &[
            "--vault",
            VAULT,
            "artifact",
            "write-file",
            "Templates/component.md",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn prefill_file_builds_target_model() {
    let d = tmp("prefill");
    let f = d.join("t.txt");
    std::fs::write(&f, "rules").unwrap();
    let out = run(
        &[
            "artifact",
            "prefill",
            "--source",
            "file",
            "--type",
            "AIAgentsConfig",
            "--text-file",
            f.to_str().unwrap(),
            "--file-name",
            "CLAUDE.md",
            "--json",
        ],
        b"",
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let l = json_lines(&out.stdout);
    assert_eq!(l[0]["target"], "CLAUDE.md");
    std::fs::remove_dir_all(d).unwrap();
}
