//! Human (non-`--json`) CLI output: `mda types` / `mda vault` / `mda config` (split from `cli_human.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{run, run_cfg};

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-human-vault-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn out_text(out: std::process::Output) -> String {
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn types_list_prints_rows() {
    let t = out_text(run(&["--vault", VAULT, "types", "list"], b""));
    assert!(t.contains("Command  Commands/  disabled  (no dir)"), "{t}");
    assert!(t.lines().any(|l| l.ends_with("enabled")), "{t}");
}

#[test]
fn vault_inspect_and_config_get_print() {
    let t = out_text(run(&["vault", "inspect", VAULT], b""));
    assert!(t.contains("  vault"), "{t}");
    assert!(t.contains("Commands/  missing"), "{t}");
    let bare = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault_bare");
    let t = out_text(run(&["vault", "inspect", bare], b""));
    assert!(t.contains("missing defaults: "), "{t}");
    let t = out_text(run(&["--vault", VAULT, "config", "get"], b""));
    assert!(t.contains("source flag"), "{t}");
    assert!(t.contains("vault "), "{t}");
}

#[test]
fn vault_list_marks_default_and_unavailable() {
    let cfg = tmpdir("list-cfg");
    let gone = std::env::temp_dir()
        .join(format!("nonexistent-mda-x-{}", std::process::id()))
        .display()
        .to_string();
    std::fs::write(
        cfg.join("config.toml"),
        format!("default = {VAULT:?}\nvaults = [{VAULT:?}, {gone:?}]\n"),
    )
    .unwrap();
    let t = out_text(run_cfg(&["vault", "list"], b"", &cfg));
    assert!(t.contains(&format!("* {VAULT}")), "{t}");
    assert!(t.contains(&format!("  {gone}  (unavailable)")), "{t}");
    let t = out_text(run_cfg(&["vault", "list"], b"", &tmpdir("list-empty")));
    assert_eq!(t.trim(), "no known vaults");
    std::fs::remove_dir_all(cfg).unwrap();
}

#[test]
fn vault_list_escapes_esc_in_stored_path() {
    let cfg = tmpdir("esc-cfg");
    std::fs::write(
        cfg.join("config.toml"),
        "default = \"/x\\u001b[31m\"\nvaults = [\"/x\\u001b[31m\"]\n",
    )
    .unwrap();
    let out = run_cfg(&["vault", "list"], b"", &cfg);
    assert!(!out.stdout.contains(&0x1b), "{:?}", out.stdout);
    assert!(String::from_utf8(out.stdout).unwrap().contains("u{1b}"));
    std::fs::remove_dir_all(cfg).unwrap();
}
