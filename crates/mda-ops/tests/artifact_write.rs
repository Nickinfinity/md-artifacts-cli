//! `artifact.create/update/patch/delete` against temp vaults: sinks, symlink containment, error
//! shape (no machine path), size cap, read→update→read hash agreement.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::PathBuf;

use mda_ops::artifact::{ReadRequest, read};
use mda_ops::artifact_write::{
    CreateRequest, DeleteRequest, PatchEdit, PatchRequest, UpdateRequest, create, delete, patch,
    update,
};
use mda_ops::{Ctx, OpError, Root};
use serde_json::json;

struct Vault {
    dir: PathBuf,
    ctx: Ctx,
    canon: String,
}

fn vault(name: &str) -> Vault {
    let dir = std::env::temp_dir().join(format!("mda-aw-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("Templates")).unwrap();
    let root = Root::new(&dir).unwrap();
    let canon = root.path().display().to_string();
    Vault {
        dir,
        ctx: Ctx::new(Some(root)),
        canon,
    }
}

impl Vault {
    fn put(&self, rel: &str, body: &str) {
        fs::write(self.dir.join(rel), body).unwrap();
    }
    fn get(&self, rel: &str) -> String {
        fs::read_to_string(self.dir.join(rel)).unwrap()
    }
    /// A file outside the vault, plus a `Templates/out.md` symlink to it.
    fn link_out(&self, body: &str) -> PathBuf {
        let out = self.dir.with_file_name(format!(
            "{}-out.md",
            self.dir.file_name().unwrap().to_string_lossy()
        ));
        fs::write(&out, body).unwrap();
        std::os::unix::fs::symlink(&out, self.dir.join("Templates/out.md")).unwrap();
        out
    }
    /// No error param may contain the machine path of the vault root.
    fn no_abs(&self, e: OpError) -> OpError {
        for (k, v) in &e.params {
            assert!(!v.contains(&self.canon), "param {k}={v} leaks the root");
        }
        e
    }
}

fn hash(s: &str) -> String {
    mda_vault::content_hash(s.as_bytes())
}

fn model(code: &str) -> mda_core::model::ArtifactModel {
    serde_json::from_value(json!({"artifactType":"Template","title":"T","blocks":[{"code":code}]}))
        .unwrap()
}

fn upd(
    v: &Vault,
    path: &str,
    h: String,
    code: &str,
) -> Result<mda_ops::artifact_write::WriteResponse, OpError> {
    update(
        &v.ctx,
        UpdateRequest {
            path: path.into(),
            expected_hash: h,
            model: model(code),
        },
    )
    .map_err(|e| v.no_abs(e))
}

fn pat(
    v: &Vault,
    path: &str,
    h: String,
) -> Result<mda_ops::artifact_write::WriteResponse, OpError> {
    let edit = PatchEdit::Title { value: "X".into() };
    patch(
        &v.ctx,
        PatchRequest {
            path: path.into(),
            expected_hash: h,
            edit,
        },
    )
    .map_err(|e| v.no_abs(e))
}

const BODY: &str = "---\nartifactType: Template\ntitle: T\n---\n\n```\nold\n```\n";

#[test]
fn symlink_target_is_never_written_or_deleted() {
    let v = vault("link");
    let out_body = "---\nartifactType: Template\ntitle: Out\n---\n\n```\nsecret\n```\n";
    let out = v.link_out(out_body);
    let h = hash(out_body);
    // update/patch resolve through contain(): the target is outside the root.
    for e in [
        upd(&v, "Templates/out.md", h.clone(), "pwn").unwrap_err(),
        pat(&v, "Templates/out.md", h.clone()).unwrap_err(),
        delete(
            &v.ctx,
            DeleteRequest {
                path: "Templates/out.md".into(),
                expected_hash: h,
            },
        )
        .map_err(|e| v.no_abs(e))
        .unwrap_err(),
    ] {
        assert!(
            ["path.outside_root", "file.not_regular"].contains(&e.code),
            "{}",
            e.code
        );
    }
    assert_eq!(fs::read_to_string(out).unwrap(), out_body);
}

#[test]
fn error_params_hold_no_machine_path() {
    let v = vault("noabs");
    v.put("Templates/a.md", BODY);
    let e = upd(&v, "Templates/a.md", "0".repeat(64), "x").unwrap_err();
    assert_eq!(e.code, "file.conflict");
    assert_eq!(e.params["path"], "Templates/a.md");
    let e = upd(&v, "Templates/gone.md", "0".repeat(64), "x").unwrap_err();
    assert_eq!(e.code, "path.not_found");
    let e = create(
        &v.ctx,
        CreateRequest {
            path: "Templates/a.md".into(),
            model: model("x"),
        },
    )
    .unwrap_err();
    assert_eq!(v.no_abs(e).code, "file.exists");
    assert_eq!(v.get("Templates/a.md"), BODY);
}

#[test]
fn oversize_output_is_refused_and_nothing_written() {
    let v = vault("big");
    let big = "x".repeat(1 << 20);
    let e = create(
        &v.ctx,
        CreateRequest {
            path: "Templates/big.md".into(),
            model: model(&big),
        },
    )
    .unwrap_err();
    assert_eq!(v.no_abs(e.clone()).code, "file.too_large");
    assert_eq!(e.params["path"], "Templates/big.md");
    assert!(!v.dir.join("Templates/big.md").exists());
    v.put("Templates/a.md", BODY);
    let e = upd(&v, "Templates/a.md", hash(BODY), &big).unwrap_err();
    assert_eq!(e.code, "file.too_large");
    assert_eq!(v.get("Templates/a.md"), BODY);
}

#[test]
fn read_update_read_hashes_agree() {
    let v = vault("rur");
    v.put("Templates/a.md", BODY);
    let r1 = read(
        &v.ctx,
        ReadRequest {
            path: "Templates/a.md".into(),
        },
    )
    .unwrap();
    let w = upd(&v, "Templates/a.md", r1.hash, "new").unwrap();
    let r2 = read(
        &v.ctx,
        ReadRequest {
            path: "Templates/a.md".into(),
        },
    )
    .unwrap();
    assert_eq!(r2.hash, w.hash);
}

#[test]
fn conflict_is_reported_before_refusals() {
    // A flagged file with a stale hash reports the conflict, not `not_serializable`.
    let v = vault("order");
    v.put(
        "Templates/f.md",
        "---\nartifactType: Template\n---\n%%oa:start%%\nx\n%%oa:end%%\n",
    );
    let e = upd(&v, "Templates/f.md", "0".repeat(64), "x").unwrap_err();
    assert_eq!(e.code, "file.conflict");
}

fn patch_edit(
    v: &Vault,
    path: &str,
    h: String,
    edit: PatchEdit,
) -> Result<mda_ops::artifact_write::WriteResponse, OpError> {
    patch(
        &v.ctx,
        PatchRequest {
            path: path.into(),
            expected_hash: h,
            edit,
        },
    )
    .map_err(|e| v.no_abs(e))
}

#[test]
fn oversize_patch_is_refused_and_file_unchanged() {
    let v = vault("bigpatch");
    v.put("Templates/a.md", BODY);
    let code = "x".repeat(1 << 20);
    let e = patch_edit(
        &v,
        "Templates/a.md",
        hash(BODY),
        PatchEdit::Code {
            block: None,
            heading: None,
            code,
        },
    )
    .unwrap_err();
    assert_eq!(e.code, "file.too_large");
    assert_eq!(e.params["path"], "Templates/a.md");
    assert_eq!(v.get("Templates/a.md"), BODY);
}

// The pre-check bounds the work (content + value); the post-check bounds what lands: an
// inserted `description: …\n` line adds bytes the pre-check cannot see (reviewer, T2.5 r2).
#[test]
fn patch_output_over_the_cap_is_refused_and_file_unchanged() {
    let v = vault("bigpatch2");
    v.put("Templates/a.md", BODY);
    let max = usize::try_from(mda_ops::artifact::MAX_ARTIFACT_BYTES).unwrap();
    let value = "x".repeat(max - BODY.len());
    let e = patch_edit(
        &v,
        "Templates/a.md",
        hash(BODY),
        PatchEdit::Description { value },
    )
    .unwrap_err();
    assert_eq!(e.code, "file.too_large");
    assert_eq!(v.get("Templates/a.md"), BODY);
}

#[test]
fn bom_survives_a_patch() {
    let v = vault("bom");
    let raw = format!("\u{feff}{BODY}");
    v.put("Templates/a.md", &raw);
    let w = pat(&v, "Templates/a.md", hash(&raw)).unwrap();
    let got = fs::read(v.dir.join("Templates/a.md")).unwrap();
    assert!(got.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert_eq!(
        String::from_utf8(got.clone()).unwrap(),
        format!("\u{feff}{}", BODY.replace("title: T", "title: X"))
    );
    assert_eq!(w.hash, mda_vault::content_hash(&got));
}

#[test]
fn invalid_utf8_is_refused_untouched() {
    let v = vault("badutf");
    let mut raw = BODY.as_bytes().to_vec();
    raw.extend_from_slice(&[0xFF, 0xFE]);
    fs::write(v.dir.join("Templates/a.md"), &raw).unwrap();
    let e = pat(&v, "Templates/a.md", mda_vault::content_hash(&raw)).unwrap_err();
    assert_eq!(e.code, "artifact.not_serializable");
    assert_eq!(e.params["reason"], "not_utf8");
    assert_eq!(fs::read(v.dir.join("Templates/a.md")).unwrap(), raw);
}
