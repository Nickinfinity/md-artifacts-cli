//! `mda vault …` through the real binary: paths, stdout purity, and where the machine file lands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{bin, copy_tree, json_lines, run_cfg, run_with};

const BARE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault_bare");

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-clivault-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn select_relative_path_stores_absolute() {
    let parent = tmp("rel");
    copy_tree(std::path::Path::new(BARE), &parent.join("v"));
    let cfg = tmp("rel-cfg");
    let mut cmd = bin();
    cmd.env("MDA_CONFIG_DIR", &cfg).current_dir(&parent);
    let out = run_with(cmd, &["vault", "select", "v"], b"");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let toml = std::fs::read_to_string(cfg.join("config.toml")).unwrap();
    let want = parent.join("v").canonicalize().unwrap();
    assert!(toml.contains(want.to_str().unwrap()), "{toml}");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("selected ")
    );
}

#[test]
fn json_select_is_one_json_line() {
    let v = tmp("json");
    copy_tree(std::path::Path::new(BARE), &v);
    let out = run_cfg(
        &["--json", "vault", "select", v.to_str().unwrap()],
        b"",
        &tmp("json-cfg"),
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json_lines(&out.stdout).len(), 1);
}

#[test]
fn os_config_dir_lands_inside_temp_home() {
    let home = tmp("home");
    let v = tmp("os-vault");
    copy_tree(std::path::Path::new(BARE), &v);
    let mut cmd = bin();
    cmd.env_remove("MDA_CONFIG_DIR")
        .env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("APPDATA");
    let out = run_with(cmd, &["vault", "select", v.to_str().unwrap()], b"");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let rel = if cfg!(target_os = "macos") {
        "Library/Application Support/mda/config.toml"
    } else {
        ".config/mda/config.toml"
    };
    assert!(home.join(rel).is_file(), "missing {rel}");
}

#[test]
fn explicit_config_dir_leaves_home_untouched() {
    let home = tmp("home2");
    let cfg = tmp("cfg2");
    let v = tmp("v2");
    copy_tree(std::path::Path::new(BARE), &v);
    let mut cmd = bin();
    cmd.env("MDA_CONFIG_DIR", &cfg).env("HOME", &home);
    let out = run_with(cmd, &["vault", "select", v.to_str().unwrap()], b"");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
    assert!(cfg.join("config.toml").is_file());
}

#[test]
fn relative_config_dir_is_refused() {
    let cwd = tmp("relcfg");
    let v = tmp("v3");
    copy_tree(std::path::Path::new(BARE), &v);
    let mut cmd = bin();
    cmd.env("MDA_CONFIG_DIR", "rel").current_dir(&cwd);
    let out = run_with(cmd, &["vault", "select", v.to_str().unwrap()], b"");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).starts_with("config.invalid"),
        "{}",
        stderr(&out)
    );
    assert!(!cwd.join("rel").exists());
}

#[test]
fn bad_bool_is_a_clap_error_and_unknown_vault_exits_1() {
    let cfg = tmp("cfg4");
    let out = run_cfg(&["types", "set-enabled", "Command", "maybe"], b"", &cfg);
    assert_eq!(out.status.code(), Some(2));
    let v = tmp("v4");
    copy_tree(std::path::Path::new(BARE), &v);
    let out = run_cfg(&["vault", "use", v.to_str().unwrap()], b"", &cfg);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).starts_with("vault.unknown:"),
        "{}",
        stderr(&out)
    );
}
