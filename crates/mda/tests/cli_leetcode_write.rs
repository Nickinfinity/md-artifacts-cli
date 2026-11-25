//! `mda leetcode write`: the hostile sinks (🔒 symlink out of the vault, oversize, a refused nested
//! create, leftover temps) and a read → write round trip through the binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::run_cfg;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("mda-cli-lcw-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A vault with a `LeetCode/` dir; returns (vault, config dir).
fn vault(name: &str) -> (PathBuf, PathBuf) {
    let v = tmp(&format!("{name}-v"));
    std::fs::create_dir_all(v.join(".obsidian")).unwrap();
    std::fs::create_dir_all(v.join("LeetCode")).unwrap();
    (v, tmp(&format!("{name}-c")))
}

const WRITE: &str = include_str!("fixtures/vault_leetcode/LeetCode/write.md");

fn mda(v: &Path, cfg: &Path, args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut a = vec!["--vault", v.to_str().unwrap(), "leetcode"];
    a.extend_from_slice(args);
    a.push("--json");
    run_cfg(&a, stdin, cfg)
}

/// The `exercise` of `write.md` as `{exercise}` JSON, read through the binary.
fn exercise_json(v: &Path, cfg: &Path) -> serde_json::Value {
    std::fs::write(v.join("LeetCode/src.md"), WRITE).unwrap();
    let out = mda(v, cfg, &["read", "LeetCode/src.md", "--layout"], b"");
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    std::fs::remove_file(v.join("LeetCode/src.md")).unwrap();
    serde_json::json!({ "exercise": r.get("exercise"), "layout": r.get("layout") })
}

fn no_temps(root: &Path) {
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        let n = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!n.contains(".mda-tmp-"), "{p:?}");
        if p.is_dir() {
            no_temps(&p);
        }
    }
}

#[test]
fn read_then_write_is_byte_identical() {
    let (v, cfg) = vault("rt");
    let body = exercise_json(&v, &cfg);
    let out = mda(
        &v,
        &cfg,
        &["write", "LeetCode/copy.md", "--file", "-"],
        body.to_string().as_bytes(),
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stdout);
    // no layout was sent: the canonical form of an exercise already in canonical form is itself
    assert_eq!(
        std::fs::read_to_string(v.join("LeetCode/copy.md")).unwrap(),
        WRITE
    );
    no_temps(&v);
}

#[test]
fn oversize_is_refused_and_nothing_is_created() {
    let (v, cfg) = vault("big");
    let mut body = exercise_json(&v, &cfg);
    body["exercise"]["description"] = "x".repeat(2 << 20).into();
    let out = mda(
        &v,
        &cfg,
        &["write", "LeetCode/deep/big.md", "--file", "-"],
        body.to_string().as_bytes(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("file.too_large")
    );
    assert!(!v.join("LeetCode/deep").exists());
    no_temps(&v);
}

#[test]
fn refused_nested_create_leaves_no_directory() {
    let (v, cfg) = vault("ns");
    let mut body = exercise_json(&v, &cfg);
    body["exercise"]["title"] = "".into();
    let out = mda(
        &v,
        &cfg,
        &["write", "LeetCode/a/b/x.md", "--file", "-"],
        body.to_string().as_bytes(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("leetcode.not_serializable")
    );
    assert!(!v.join("LeetCode/a").exists());
    no_temps(&v);
}

#[cfg(unix)]
#[test]
fn symlink_out_of_the_vault_is_refused_and_outside_file_unchanged() {
    let (v, cfg) = vault("sym");
    let body = exercise_json(&v, &cfg);
    let outside = tmp("sym-out");
    let secret = outside.join("secret.md");
    std::fs::write(&secret, WRITE).unwrap();
    std::os::unix::fs::symlink(&secret, v.join("LeetCode/link.md")).unwrap();
    // replace through the link, and create through a symlinked directory
    std::os::unix::fs::symlink(&outside, v.join("LeetCode/dir")).unwrap();
    let hash = common_hash(WRITE);
    for args in [
        ["write", "LeetCode/link.md", "--file", "-", "--hash", &hash].as_slice(),
        ["write", "LeetCode/dir/new.md", "--file", "-"].as_slice(),
    ] {
        let out = mda(&v, &cfg, args, body.to_string().as_bytes());
        assert_eq!(out.status.code(), Some(1), "{args:?}");
    }
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), WRITE);
    assert!(!outside.join("new.md").exists());
    no_temps(&outside);
}

/// SHA-256 hex of `s` through the engine's own `read` (no hash crate in the test deps).
fn common_hash(s: &str) -> String {
    let (v, cfg) = vault("hash");
    std::fs::write(v.join("LeetCode/h.md"), s).unwrap();
    let out = mda(&v, &cfg, &["read", "LeetCode/h.md"], b"");
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    r.get("hash").and_then(|h| h.as_str()).unwrap().to_owned()
}
