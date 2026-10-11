//! `types.list` / `types.set_enabled` against temp vaults: refusals leave config and dirs untouched,
//! no path in an error is absolute, disable never deletes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::types::{ListTypesRequest, SetEnabledRequest, list, set_enabled};
use mda_ops::{ArtifactType, Ctx, OpError, Root};

fn vault(name: &str) -> (PathBuf, Ctx) {
    let d = std::env::temp_dir().join(format!("mda-types-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join(".obsidian")).unwrap();
    let d = d.canonicalize().unwrap();
    let ctx = Ctx::new(Some(Root::new(&d).unwrap()));
    (d, ctx)
}

fn req(t: ArtifactType, enabled: bool) -> SetEnabledRequest {
    SetEnabledRequest {
        artifact_type: t,
        enabled,
    }
}

fn assert_no_abs(e: &OpError, root: &Path) {
    for v in e.params.values() {
        assert!(!Path::new(v).is_absolute(), "absolute param {v}");
        assert!(!v.contains(root.to_str().unwrap()), "root leaked in {v}");
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
fn mda_a_file_is_not_dir_and_bytes_kept() {
    let (v, ctx) = vault("mdafile");
    fs::write(v.join(".mda"), "keep").unwrap();
    for e in [
        set_enabled(&ctx, req(ArtifactType::Snippet, true)).unwrap_err(),
        list(&ctx, ListTypesRequest {}).unwrap_err(),
    ] {
        assert_eq!(e.code, "path.not_dir");
        assert_eq!(e.params["path"], ".mda");
        assert_no_abs(&e, &v);
    }
    assert_eq!(fs::read_to_string(v.join(".mda")).unwrap(), "keep");
}

#[test]
fn commands_a_file_refuses_enable_and_writes_no_config() {
    let (v, ctx) = vault("cmdfile");
    fs::write(v.join("Commands"), "keep").unwrap();
    let e = set_enabled(&ctx, req(ArtifactType::Command, true)).unwrap_err();
    assert_eq!(e.code, "path.not_dir");
    assert_eq!(e.params["path"], "Commands");
    assert_no_abs(&e, &v);
    assert!(!v.join(".mda").exists(), "config written despite refusal");
    assert_eq!(fs::read_to_string(v.join("Commands")).unwrap(), "keep");
}

#[cfg(unix)]
#[test]
fn commands_symlink_out_is_outside_root() {
    let (v, ctx) = vault("cmdlink");
    let out = vault("cmdlink-out").0;
    symlink(&out, v.join("Commands")).unwrap();
    let e = set_enabled(&ctx, req(ArtifactType::Command, true)).unwrap_err();
    assert_eq!(e.code, "path.outside_root");
    assert_no_abs(&e, &v);
    assert!(!v.join(".mda").exists());
    assert_eq!(names(&out), [".obsidian"]);
}

#[cfg(unix)]
#[test]
fn mda_symlink_out_is_outside_root() {
    let (v, ctx) = vault("mdalink");
    let out = vault("mdalink-out").0;
    symlink(&out, v.join(".mda")).unwrap();
    let e = set_enabled(&ctx, req(ArtifactType::Snippet, true)).unwrap_err();
    assert_eq!(e.code, "path.outside_root");
    assert_no_abs(&e, &v);
    assert_eq!(names(&out), [".obsidian"]);
}

#[test]
fn disable_keeps_dir_and_files() {
    let (v, ctx) = vault("disable");
    fs::create_dir(v.join("Snippets")).unwrap();
    fs::write(v.join("Snippets/a.md"), "x").unwrap();
    let r = set_enabled(&ctx, req(ArtifactType::Snippet, false)).unwrap();
    assert!(!r.enabled && !r.created);
    assert_eq!(fs::read_to_string(v.join("Snippets/a.md")).unwrap(), "x");
}

#[test]
fn corrupt_config_refuses_enable_and_creates_no_dir() {
    let (v, ctx) = vault("corrupt");
    fs::create_dir(v.join(".mda")).unwrap();
    fs::write(v.join(".mda/config.toml"), "[types\n").unwrap();
    let e = set_enabled(&ctx, req(ArtifactType::Snippet, true)).unwrap_err();
    assert_eq!(e.code, "config.invalid");
    assert_eq!(e.params["path"], ".mda/config.toml");
    assert!(
        !v.join("Snippets").exists(),
        "dir created despite corrupt config"
    );
    assert_eq!(
        fs::read_to_string(v.join(".mda/config.toml")).unwrap(),
        "[types\n"
    );
}
