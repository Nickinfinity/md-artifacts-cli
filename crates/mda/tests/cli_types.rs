//! `mda types set-location`: human output and the symlink sink (🔒 D-5: a `dir` symlinked out of
//! the vault is refused and the machine file stays untouched).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::run_cfg;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("mda-cli-types-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn vault(name: &str) -> std::path::PathBuf {
    let v = tmp(name);
    std::fs::create_dir(v.join(".obsidian")).unwrap();
    v
}

#[test]
fn set_location_prints_the_new_place() {
    let (v, cfg) = (vault("human-v"), tmp("human-c"));
    let out = run_cfg(
        &[
            "--vault",
            v.to_str().unwrap(),
            "types",
            "set-location",
            "Snippet",
            "--dir",
            "Lib",
        ],
        b"",
        &cfg,
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let t = String::from_utf8(out.stdout).unwrap();
    assert_eq!(t.trim(), format!("location → {}/Lib", v.display()));
}

#[cfg(unix)]
#[test]
fn set_location_refuses_a_dir_symlinked_out() {
    let (v, cfg, outside) = (vault("sym-v"), tmp("sym-c"), tmp("sym-out"));
    std::os::unix::fs::symlink(&outside, v.join("Evil")).unwrap();
    let seed = "[types.AIPrompt]\ndir=\"P\"";
    std::fs::write(cfg.join("config.toml"), seed).unwrap();
    let out = run_cfg(
        &[
            "--vault",
            v.to_str().unwrap(),
            "types",
            "set-location",
            "Snippet",
            "--dir",
            "Evil",
            "--json",
        ],
        b"",
        &cfg,
    );
    assert_eq!(out.status.code(), Some(1));
    let e: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(e["code"], "artifact.bad_path");
    assert_eq!(e["params"]["path"], "Snippets");
    assert_eq!(
        std::fs::read_to_string(cfg.join("config.toml")).unwrap(),
        seed
    );
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
}

/// D-17: the dir is made only when the target vault's own file enables the type.
fn d17(name: &str, vault_cfg: Option<&str>) -> (std::path::PathBuf, std::path::PathBuf) {
    let (v, cfg) = (vault(&format!("{name}-v")), tmp(&format!("{name}-c")));
    if let Some(c) = vault_cfg {
        std::fs::create_dir(v.join(".mda")).unwrap();
        std::fs::write(v.join(".mda/config.toml"), c).unwrap();
    }
    let out = run_cfg(
        &[
            "--vault",
            v.to_str().unwrap(),
            "types",
            "set-location",
            "Snippet",
            "--dir",
            "Lib",
        ],
        b"",
        &cfg,
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    (v, cfg)
}

#[test]
fn set_location_creates_dir_when_enabled() {
    let (v, cfg) = d17("d17-on", Some("[types.Snippet]\nenabled = true\n"));
    assert!(v.join("Lib").is_dir());
    let m = std::fs::read_to_string(cfg.join("config.toml")).unwrap();
    assert!(
        m.contains("[types.Snippet]") && m.contains("dir = \"Lib\""),
        "{m}"
    );
}

#[test]
fn set_location_creates_no_dir_unless_enabled() {
    for (n, c) in [
        ("d17-none", None),
        ("d17-off", Some("[types.Snippet]\nenabled = false\n")),
    ] {
        let (v, cfg) = d17(n, c);
        assert!(!v.join("Lib").exists(), "{n}");
        let m = std::fs::read_to_string(cfg.join("config.toml")).unwrap();
        assert!(
            m.contains("[types.Snippet]") && m.contains("dir = \"Lib\""),
            "{n}: {m}"
        );
    }
}
