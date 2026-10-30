//! `vault.*` / `config.get` / the resolver against temp vaults and temp config dirs: session
//! switching and the "nothing written on any refusal" sinks. Never touches the real user config.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::vault::{
    ConfigGetRequest, SelectRequest, UseRequest, get, resolve, select, use_vault,
};
use mda_ops::{Ctx, Source, dispatch};
use serde_json::json;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-vt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A bare Obsidian vault (`.obsidian/` only), returned canonical in its plain form.
fn bare(name: &str) -> PathBuf {
    let d = tmp(name);
    fs::create_dir_all(d.join(".obsidian")).unwrap();
    // plain form: what the engine returns (D-33); `Root::new` canonicalizes again
    mda_ops::plain_path(&d.canonicalize().unwrap())
}

fn sel(path: &Path) -> SelectRequest {
    SelectRequest {
        path: path.display().to_string(),
        create_defaults: true,
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

#[test]
fn select_switches_the_session() {
    let (v, cfg) = (bare("sw-v"), tmp("sw-cfg"));
    let ctx = Ctx::new(None).with_config_dir(cfg);
    assert_eq!(
        dispatch(&ctx, "artifact.tree", json!({"type": "Snippet"}))
            .unwrap_err()
            .code,
        "vault.not_selected"
    );
    select(&ctx, sel(&v)).unwrap();
    let t = dispatch(&ctx, "artifact.tree", json!({"type": "Snippet"})).unwrap();
    assert_eq!(t["files"], json!([]));
    assert_eq!(ctx.source.get(), Source::Default);
}

#[test]
fn select_twice_does_not_panic() {
    let (a, b, cfg) = (bare("tw-a"), bare("tw-b"), tmp("tw-cfg"));
    let ctx = Ctx::new(None).with_config_dir(cfg);
    select(&ctx, sel(&a)).unwrap();
    select(&ctx, sel(&b)).unwrap();
    let r = get(&ctx, ConfigGetRequest {}).unwrap();
    assert_eq!(r.vault.as_deref(), b.to_str());
}

#[test]
fn use_under_flag_switches_root_and_source() {
    let (a, b, cfg) = (bare("us-a"), bare("us-b"), tmp("us-cfg"));
    let setup = Ctx::new(None).with_config_dir(cfg.clone());
    select(&setup, sel(&a)).unwrap();
    select(&setup, sel(&b)).unwrap();
    let ctx = resolve(Some(&a), Some(cfg)).unwrap();
    assert_eq!(ctx.source.get(), Source::Flag);
    use_vault(
        &ctx,
        UseRequest {
            path: b.display().to_string(),
        },
    )
    .unwrap();
    assert_eq!(ctx.source.get(), Source::Default);
    assert_eq!(ctx.root.borrow().as_ref().unwrap().path(), b.as_path());
}

#[test]
fn resolve_flag_errors_are_fatal() {
    assert_eq!(
        resolve(Some(Path::new("rel")), None).err().unwrap().code,
        "op.bad_request"
    );
    let plain = tmp("rf-plain");
    assert_eq!(
        resolve(Some(&plain), None).err().unwrap().code,
        "vault.not_obsidian"
    );
}

#[test]
fn stale_default_defers_and_writes_nothing() {
    let cfg = tmp("st-cfg");
    // forward slashes: a Windows `C:\..` path would be an invalid TOML escape
    let gone = std::env::temp_dir()
        .join(format!("nonexistent-mda-w4c-{}", std::process::id()))
        .to_string_lossy()
        .replace('\\', "/");
    let body = format!("default = \"{gone}\"\nvaults = [\"{gone}\"]\n");
    fs::write(cfg.join("config.toml"), &body).unwrap();
    let ctx = resolve(None, Some(cfg.clone())).unwrap();
    assert_eq!(
        dispatch(&ctx, "artifact.tree", json!({"type": "Snippet"}))
            .unwrap_err()
            .code,
        "vault.unavailable"
    );
    assert_eq!(names(&cfg), ["config.toml"]);
    assert_eq!(fs::read_to_string(cfg.join("config.toml")).unwrap(), body);
}

#[test]
fn select_blocked_type_dir_persists_nothing() {
    let (v, cfg) = (bare("bl-v"), tmp("bl-cfg"));
    fs::write(v.join("Snippets"), "x").unwrap();
    let ctx = Ctx::new(None).with_config_dir(cfg.clone());
    assert_eq!(select(&ctx, sel(&v)).unwrap_err().code, "path.not_dir");
    assert!(names(&cfg).is_empty());
    assert!(ctx.root.borrow().is_none());
}

#[test]
fn select_creates_only_default_dirs() {
    let (v, cfg) = (bare("cd-v"), tmp("cd-cfg"));
    let ctx = Ctx::new(None).with_config_dir(cfg);
    select(&ctx, sel(&v)).unwrap();
    for d in ["Snippets", "AIAgentsConf", "AIPrompts"] {
        assert!(v.join(d).is_dir(), "{d}");
    }
    assert!(!v.join("Commands").exists());
}

#[test]
fn refused_machine_file_creates_no_dir() {
    let full = format!(
        "vaults = [{}]\n",
        (0..64)
            .map(|i| format!("\"/o{i}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (tag, body) in [("corrupt", "vaults = 1\n".to_owned()), ("full", full)] {
        let (v, cfg) = (bare(&format!("rf-{tag}-v")), tmp(&format!("rf-{tag}-cfg")));
        fs::write(cfg.join("config.toml"), &body).unwrap();
        let ctx = Ctx::new(None).with_config_dir(cfg.clone());
        assert_eq!(select(&ctx, sel(&v)).unwrap_err().code, "config.invalid");
        assert_eq!(names(&v), [".obsidian"], "{tag}: a type dir was created");
        assert_eq!(fs::read_to_string(cfg.join("config.toml")).unwrap(), body);
    }
}

#[test]
fn config_dir_that_is_a_file_is_refused_untouched() {
    let (v, base) = (bare("cf-v"), tmp("cf-base"));
    let cfg = base.join("cfgfile");
    fs::write(&cfg, "keep").unwrap();
    let ctx = Ctx::new(None).with_config_dir(cfg.clone());
    // Observed: `Root::new` on a file is `NotDir`, surfaced as `path.not_dir`.
    assert_eq!(select(&ctx, sel(&v)).unwrap_err().code, "path.not_dir");
    assert_eq!(fs::read_to_string(&cfg).unwrap(), "keep");
    assert_eq!(names(&v), [".obsidian"]);
}

#[test]
fn no_config_dir_is_config_invalid() {
    let v = bare("nc-v");
    let ctx = Ctx::new(None);
    let e = select(&ctx, sel(&v)).unwrap_err();
    assert_eq!((e.code, e.params["path"].as_str()), ("config.invalid", ""));
    assert_eq!(names(&v), [".obsidian"]);
}

/// H4.2 cross-op: select a bare vault, enable `Command`, and every listing agrees.
#[test]
fn select_then_enable_then_list_agree() {
    let (v, cfg) = (bare("x-v"), tmp("x-cfg"));
    let ctx = Ctx::new(None).with_config_dir(cfg);
    let path = v.display().to_string();
    dispatch(&ctx, "vault.select", json!({"path": path})).unwrap();
    let r = dispatch(
        &ctx,
        "types.set_enabled",
        json!({"type": "Command", "enabled": true}),
    )
    .unwrap();
    assert_eq!(
        r,
        json!({"type": "Command", "enabled": true, "created": true})
    );
    let rows = dispatch(&ctx, "types.list", json!({})).unwrap();
    let cmd = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["type"] == "Command")
        .unwrap();
    assert_eq!(cmd["enabled"], true);
    assert_eq!(cmd["dirExists"], true);
    assert!(v.join("Commands").is_dir());
    let list = dispatch(&ctx, "vault.list", json!({})).unwrap();
    assert_eq!(list["default"], json!(path));
    assert_eq!(list["vaults"], json!([{"path": path, "available": true}]));
}

/// An old config holding the canonical (on Windows `\\?\`) spelling: `list` shows the plain form,
/// `use` finds it, `select` does not add a duplicate. Trivially equal off Windows.
#[test]
fn stored_canonical_spelling_lists_plain_and_matches() {
    let v = bare("stored-canon");
    let cfg = tmp("stored-canon-cfg");
    let stored = v.canonicalize().unwrap().display().to_string();
    fs::write(
        cfg.join("config.toml"),
        format!("default = '{stored}'\nvaults = ['{stored}']\n"),
    )
    .unwrap();
    let ctx = Ctx::new(None).with_config_dir(cfg.clone());
    let plain = v.display().to_string(); // `bare` is plain already
    let l = dispatch(&ctx, "vault.list", json!({})).unwrap();
    assert_eq!(l["default"], plain);
    assert_eq!(l["vaults"][0]["path"], plain);
    dispatch(&ctx, "vault.use", json!({"path": plain})).unwrap();
    dispatch(&ctx, "vault.select", json!({"path": plain})).unwrap();
    let l = dispatch(&ctx, "vault.list", json!({})).unwrap();
    assert_eq!(l["vaults"].as_array().unwrap().len(), 1);
}
