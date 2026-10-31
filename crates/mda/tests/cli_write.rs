//! Write commands through the real binary: input paths (`-`, files), usage errors, exit codes.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::{copy_tree, run};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");
const MODEL: &str = r#"{"artifactType":"Template","title":"N","blocks":[{"language":"ts","code":"export const x = 1;"}]}"#;

/// A fresh vault copy; the caller removes it.
fn vault(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-cw-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(std::path::Path::new(FIXTURE), &d);
    d
}

/// The content hash `artifact show --json` reports for `rel`.
fn hash_of(v: &str, rel: &str) -> String {
    let out = run(&["--vault", v, "artifact", "show", rel, "--json"], b"");
    let j: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    j.get("hash").and_then(|h| h.as_str()).unwrap().to_owned()
}

#[test]
fn model_from_stdin_creates_file() {
    let d = vault("stdin");
    let v = d.to_str().unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "new",
            "Templates/n.md",
            "--model",
            "-",
        ],
        MODEL.as_bytes(),
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert!(d.join("Templates/n.md").is_file());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn block_without_code_file_is_a_usage_error() {
    let d = vault("usage");
    let v = d.to_str().unwrap();
    let before = std::fs::read(d.join("Templates/edit.md")).unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "patch",
            "Templates/edit.md",
            "--hash",
            "h",
            "--block",
            "0",
            "--heading",
            "x",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(std::fs::read(d.join("Templates/edit.md")).unwrap(), before);
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn missing_model_file_is_bad_request() {
    let d = vault("nomodel");
    let v = d.to_str().unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "new",
            "Templates/n.md",
            "--model",
            "/no/such/model.json",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).starts_with("op.bad_request:"),
        "{:?}",
        out.stderr
    );
    assert!(!d.join("Templates/n.md").exists());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn patch_code_from_stdin() {
    let d = vault("patch");
    let v = d.to_str().unwrap();
    let h = hash_of(v, "Templates/edit.md");
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "patch",
            "Templates/edit.md",
            "--hash",
            &h,
            "--code-file",
            "-",
        ],
        b"export const <VK-name> = 2;",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let text = std::fs::read_to_string(d.join("Templates/edit.md")).unwrap();
    assert!(text.contains("= 2;"), "{text}");
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn rm_with_wrong_hash_conflicts_and_keeps_file() {
    let d = vault("rm");
    let v = d.to_str().unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "rm",
            "Templates/edit.md",
            "--hash",
            "deadbeef",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).starts_with("file.conflict:"),
        "{:?}",
        out.stderr
    );
    assert!(d.join("Templates/edit.md").is_file());
    std::fs::remove_dir_all(d).unwrap();
}
