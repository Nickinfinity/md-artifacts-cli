//! `artifact.write_file` against temp vaults and workspaces: symlink containment, sinks, error
//! shape (no machine path), and the `actual` hash on `file.exists`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::artifact_write::{CreateRequest, create};
use mda_ops::write_file::{WriteFileRequest, write_file};
use mda_ops::{Ctx, OpError, Root};
use serde_json::json;

const COMPONENT: &str = "---\nartifactType: Template\ntitle: Button\nextension: tsx\n---\n\n```tsx\nexport const <VK-name> = 1;\n```\n\nvars:\n```vks\nVK-name: Widget\n```\n";

struct Env {
    vault: PathBuf,
    ws: PathBuf,
    ctx: Ctx,
}

fn tmp(name: &str, kind: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-wf-{}-{name}-{kind}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn env(name: &str) -> Env {
    let vault = tmp(name, "vault");
    fs::create_dir_all(vault.join("Templates")).unwrap();
    fs::write(vault.join("Templates/component.md"), COMPONENT).unwrap();
    let ctx = Ctx::new(Some(Root::new(&vault).unwrap()));
    Env {
        vault,
        ws: tmp(name, "ws"),
        ctx,
    }
}

/// Sorted entry names of `ws/out`: a leftover `.mda-tmp-` would show here.
fn out_names(e: &Env) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(e.ws.join("out"))
        .unwrap()
        .map(|d| d.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

impl Env {
    fn req(&self, dest: &str, extra: serde_json::Value) -> WriteFileRequest {
        let mut v = json!({"path": "Templates/component.md",
            "workspaceRoot": self.ws.display().to_string(), "destDir": dest});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }
    fn run(&self, dest: &str, extra: serde_json::Value) -> Result<(), OpError> {
        write_file(&self.ctx, self.req(dest, extra)).map(|_| ())
    }
    /// No error may carry a machine path (either canonical root).
    fn assert_no_abs(&self, e: &OpError) {
        let text = format!("{:?}", e.params);
        for r in [&self.vault, &self.ws] {
            assert!(!text.contains(&r.display().to_string()), "{text}");
        }
    }
}

fn outside(name: &str) -> PathBuf {
    tmp(name, "outside")
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) {
    std::os::unix::fs::symlink(target, at).unwrap();
}

#[cfg(unix)]
#[test]
fn dest_dir_symlink_out_is_refused_and_target_untouched() {
    let e = env("destlink");
    let out = outside("destlink");
    link(&out, &e.ws.join("out"));
    let err = e.run("out", json!({})).unwrap_err();
    assert_eq!(err.code, "path.outside_root");
    e.assert_no_abs(&err);
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn file_symlink_out_is_refused_and_secret_unchanged() {
    let e = env("filelink");
    let out = outside("filelink");
    fs::write(out.join("secret"), "s3cret").unwrap();
    fs::create_dir_all(e.ws.join("out")).unwrap();
    link(&out.join("secret"), &e.ws.join("out/Button.tsx"));
    let hash = mda_vault::content_hash(b"s3cret");
    let err = e.run("out", json!({"expectedHash": hash})).unwrap_err();
    assert_eq!(err.code, "path.outside_root");
    e.assert_no_abs(&err);
    assert_eq!(fs::read_to_string(out.join("secret")).unwrap(), "s3cret");
}

#[test]
fn workspace_equal_to_vault_root_writes_inside_it() {
    let e = env("wsvault");
    let req: WriteFileRequest = serde_json::from_value(json!({"path": "Templates/component.md",
        "workspaceRoot": e.vault.display().to_string(), "destDir": "gen"}))
    .unwrap();
    let r = write_file(&e.ctx, req).unwrap();
    assert_eq!(r.path, "gen/Button.tsx");
    assert_eq!(
        fs::read_to_string(e.vault.join("gen/Button.tsx")).unwrap(),
        "export const Widget = 1;"
    );
}

#[test]
fn render_limit_creates_nothing() {
    let e = env("limit");
    let body = "<VK-a>".repeat(120_000);
    fs::write(
        e.vault.join("Templates/big.md"),
        format!("---\nartifactType: Template\ntitle: Big\nextension: txt\n---\n\n```txt\n{body}\n```\n\nvars:\n```vks\nVK-a: 0123456789abcdef\n```\n"),
    )
    .unwrap();
    let mut req = e.req("out", json!({}));
    req.path = "Templates/big.md".into();
    let err = write_file(&e.ctx, req).unwrap_err();
    assert_eq!(err.code, "render.limit");
    e.assert_no_abs(&err);
    assert_eq!(fs::read_dir(&e.ws).unwrap().count(), 0, "nothing created");
}

#[test]
fn exists_carries_actual_hash() {
    let e = env("exists");
    fs::create_dir_all(e.ws.join("out")).unwrap();
    fs::write(e.ws.join("out/Button.tsx"), "old\n").unwrap();
    let err = e.run("out", json!({})).unwrap_err();
    assert_eq!(err.code, "file.exists");
    assert_eq!(err.params["path"], "out/Button.tsx");
    assert_eq!(err.params["actual"], mda_vault::content_hash(b"old\n"));
    e.assert_no_abs(&err);
    assert_eq!(
        fs::read_to_string(e.ws.join("out/Button.tsx")).unwrap(),
        "old\n"
    );
    assert_eq!(out_names(&e), ["Button.tsx"]);
}

#[test]
fn stale_hash_conflicts_and_leaves_no_temp() {
    let e = env("conflict");
    fs::create_dir_all(e.ws.join("out")).unwrap();
    fs::write(e.ws.join("out/Button.tsx"), "old\n").unwrap();
    let err = e.run("out", json!({"expectedHash": "stale"})).unwrap_err();
    assert_eq!(err.code, "file.conflict");
    e.assert_no_abs(&err);
    assert_eq!(
        fs::read_to_string(e.ws.join("out/Button.tsx")).unwrap(),
        "old\n"
    );
    assert_eq!(out_names(&e), ["Button.tsx"]);
}

#[test]
fn artifact_create_over_existing_carries_actual_too() {
    let e = env("createactual");
    let req: CreateRequest = serde_json::from_value(json!({"path": "Templates/component.md",
        "model": {"artifactType": "Template", "title": "x", "blocks": [{"code": "y"}]}}))
    .unwrap();
    let err = create(&e.ctx, req).unwrap_err();
    assert_eq!(err.code, "file.exists");
    assert_eq!(
        err.params["actual"],
        mda_vault::content_hash(COMPONENT.as_bytes())
    );
}

#[test]
fn errors_carry_no_machine_path() {
    let e = env("noabs");
    for (dest, extra, code) in [
        ("../x", json!({}), "path.outside_root"),
        ("", json!({"fileName": "CON.txt"}), "naming.reserved"),
        ("", json!({"expectedHash": "h"}), "path.not_found"),
    ] {
        let err = e.run(dest, extra).unwrap_err();
        assert_eq!(err.code, code);
        e.assert_no_abs(&err);
    }
}
