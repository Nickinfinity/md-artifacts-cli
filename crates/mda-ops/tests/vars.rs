//! The `vars.*` ops against temp vaults (T5.3): symlink containment, limits, encodings, the
//! atomic-refusal sink. Every refusal asserts the bytes on disk, not only the error.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_core::vks::VksValue;
use mda_ops::artifact::{ReadRequest, read};
use mda_ops::vars::*;
use mda_ops::{Ctx, OpError, Root};

const FLAT: &str =
    "---\nartifactType: Variables\ntitle: Flat\n---\n\n```yaml VKs\nVK-host: localhost\n```\n";

struct V {
    dir: PathBuf,
    #[cfg(unix)]
    out: PathBuf,
    ctx: Ctx,
}

fn vault(name: &str) -> V {
    let base = std::env::temp_dir().join(format!("mda-vars-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let dir = base.join("v");
    #[cfg(unix)]
    let out = base.join("outside");
    fs::create_dir_all(dir.join(".obsidian")).unwrap();
    fs::create_dir_all(dir.join("Variables")).unwrap();
    #[cfg(unix)]
    fs::create_dir_all(&out).unwrap();
    let dir = dir.canonicalize().unwrap();
    #[cfg(unix)]
    let out = out.canonicalize().unwrap();
    let ctx = Ctx::new(Some(Root::new(&dir).unwrap()));
    V {
        dir,
        #[cfg(unix)]
        out,
        ctx,
    }
}

impl V {
    fn put(&self, rel: &str, bytes: &[u8]) {
        let p = self.dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
    fn get(&self, rel: &str) -> Vec<u8> {
        fs::read(self.dir.join(rel)).unwrap()
    }
    fn hash(&self, rel: &str) -> String {
        read(&self.ctx, ReadRequest { path: rel.into() })
            .unwrap()
            .hash
    }
    fn set(
        &self,
        rel: &str,
        hash: String,
        v: &str,
    ) -> Result<mda_ops::artifact_write::WriteResponse, OpError> {
        var_set(
            &self.ctx,
            set_req(rel, hash, "VK-host", VksValue::Str(v.into())),
        )
    }
}

fn set_req(path: &str, hash: String, name: &str, value: VksValue) -> VarSetRequest {
    VarSetRequest {
        path: path.into(),
        expected_hash: hash,
        subset: None,
        name: name.into(),
        value,
    }
}

fn assert_no_abs(e: &OpError, root: &Path) {
    for v in e.params.values() {
        assert!(!Path::new(v).is_absolute(), "absolute param {v}");
        assert!(!v.contains(root.to_str().unwrap()), "root leaked in {v}");
    }
}

#[cfg(unix)]
#[test]
fn symlinked_dir_never_receives_a_new_file() {
    let v = vault("dir");
    std::os::unix::fs::symlink(&v.out, v.dir.join("Variables/sub")).unwrap();
    let req = NewFileRequest {
        title: "X".into(),
        dir: Some("Variables/sub".into()),
        description: String::new(),
        tags: vec![],
    };
    let e = file_new(&v.ctx, req).unwrap_err();
    assert_eq!(e.code, "path.outside_root");
    assert_no_abs(&e, &v.dir);
    assert_eq!(
        fs::read_dir(&v.out).unwrap().count(),
        0,
        "file landed outside the vault"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_file_is_never_rewritten() {
    let v = vault("file");
    fs::write(v.out.join("t.md"), FLAT).unwrap();
    std::os::unix::fs::symlink(v.out.join("t.md"), v.dir.join("Variables/out.md")).unwrap();
    let e = v.set("Variables/out.md", "0".repeat(64), "x").unwrap_err();
    assert_eq!(e.code, "path.outside_root");
    assert_no_abs(&e, &v.dir);
    assert_eq!(fs::read_to_string(v.out.join("t.md")).unwrap(), FLAT);
}

#[test]
fn map_keys_limit_refuses_and_keeps_bytes() {
    let v = vault("keys");
    let body: String = (0..255).map(|i| format!("VK-k{i}: x\n")).collect();
    let text = format!("---\nartifactType: Variables\ntitle: Big\n---\n\n```yaml VKs\n{body}```\n");
    v.put("Variables/big.md", text.as_bytes());
    let req = VarAddRequest {
        path: "Variables/big.md".into(),
        expected_hash: v.hash("Variables/big.md"),
        subset: None,
        name: "VK-extra".into(),
        value: VksValue::Str("x".into()),
    };
    let e = var_add(&v.ctx, req).unwrap_err();
    assert_eq!(
        (e.code, e.params["limit"].as_str()),
        ("vks.limit", "map_keys")
    );
    assert_eq!(v.get("Variables/big.md"), text.as_bytes());
}

#[test]
fn body_bytes_limit_refuses_and_keeps_bytes() {
    let v = vault("body");
    v.put("Variables/f.md", FLAT.as_bytes());
    let e = v
        .set(
            "Variables/f.md",
            v.hash("Variables/f.md"),
            &"a".repeat(70_000),
        )
        .unwrap_err();
    assert_eq!(
        (e.code, e.params["limit"].as_str()),
        ("vks.limit", "body_bytes")
    );
    assert_eq!(v.get("Variables/f.md"), FLAT.as_bytes());
}

#[test]
fn response_hash_is_the_next_reads_hash() {
    let v = vault("hash");
    v.put("Variables/f.md", FLAT.as_bytes());
    let w = v
        .set("Variables/f.md", v.hash("Variables/f.md"), "other")
        .unwrap();
    assert_eq!(w.hash, v.hash("Variables/f.md"));
}

#[test]
fn bom_and_crlf_are_normalised() {
    let v = vault("bom");
    let mut b = b"\xEF\xBB\xBF".to_vec();
    b.extend_from_slice(FLAT.replace('\n', "\r\n").as_bytes());
    v.put("Variables/f.md", &b);
    v.set("Variables/f.md", v.hash("Variables/f.md"), "x")
        .unwrap();
    let out = v.get("Variables/f.md");
    assert!(!out.starts_with(b"\xEF\xBB\xBF") && !out.contains(&b'\r'));
}

#[test]
fn invalid_utf8_becomes_replacement_char() {
    // Pin (#35): lossy decode, the edit still lands.
    let v = vault("lossy");
    let mut b = b"---\nartifactType: Variables\ntitle: A".to_vec();
    b.push(0xFF);
    b.extend_from_slice(b"B\n---\n\n```yaml VKs\nVK-host: l\n```\n");
    v.put("Variables/f.md", &b);
    v.set("Variables/f.md", v.hash("Variables/f.md"), "x")
        .unwrap();
    assert!(
        String::from_utf8(v.get("Variables/f.md"))
            .unwrap()
            .contains("title: A\u{FFFD}B")
    );
}

#[test]
fn unknown_frontmatter_keys_are_dropped() {
    // Pin (X-2 ceiling): the model has no slot for them.
    let v = vault("alias");
    let t = FLAT.replace("title: Flat\n", "title: Flat\naliases: [x]\n");
    v.put("Variables/f.md", t.as_bytes());
    v.set("Variables/f.md", v.hash("Variables/f.md"), "x")
        .unwrap();
    assert!(
        !String::from_utf8(v.get("Variables/f.md"))
            .unwrap()
            .contains("aliases")
    );
}

#[test]
fn flagged_file_is_refused() {
    let v = vault("flag");
    let t = "---\nartifactType: Variables\ntitle: F\n---\n%%oa:start%%\nx\n%%oa:end%%\n";
    v.put("Variables/f.md", t.as_bytes());
    let e = v
        .set("Variables/f.md", v.hash("Variables/f.md"), "x")
        .unwrap_err();
    assert_eq!(e.code, "artifact.not_serializable");
    assert_eq!(e.params["reason"], "flagged");
    assert_eq!(v.get("Variables/f.md"), t.as_bytes());
}

#[test]
fn failed_edit_writes_nothing_and_the_retry_succeeds() {
    let v = vault("retry");
    v.put("Variables/f.md", FLAT.as_bytes());
    let req = |h: String| SubsetAddRequest {
        path: "Variables/f.md".into(),
        expected_hash: h,
        heading: "P".into(),
    };
    let e = subset_add(&v.ctx, req("0".repeat(64))).unwrap_err();
    assert_eq!(e.code, "file.conflict");
    assert_no_abs(&e, &v.dir);
    assert_eq!(v.get("Variables/f.md"), FLAT.as_bytes());
    subset_add(&v.ctx, req(v.hash("Variables/f.md"))).unwrap();
}

// H5.2 cross-op: blank → flat → sets through four ops; every response hash is the next read's.
#[test]
fn new_file_then_var_then_subset_then_set_agree_with_read() {
    let v = vault("cross");
    let svc: NewFileRequest = serde_json::from_value(serde_json::json!({"title": "Svc"})).unwrap();
    let r = file_new(&v.ctx, svc).unwrap();
    assert_eq!(r.path, "Variables/svc.md");
    assert_eq!(r.hash, v.hash(&r.path));
    let add: VarAddRequest = serde_json::from_value(serde_json::json!(
        {"path": r.path, "expectedHash": r.hash, "name": "VK-a", "value": "1"}))
    .unwrap();
    let r = var_add(&v.ctx, add).unwrap();
    assert_eq!(r.hash, v.hash(&r.path));
    let sub: SubsetAddRequest = serde_json::from_value(serde_json::json!(
        {"path": r.path, "expectedHash": r.hash, "heading": "Prod"}))
    .unwrap();
    let r = subset_add(&v.ctx, sub).unwrap();
    assert_eq!(r.hash, v.hash(&r.path));
    let set: VarSetRequest = serde_json::from_value(serde_json::json!(
        {"path": r.path, "expectedHash": r.hash, "subset": {"index": 1, "heading": "Prod"},
         "name": "VK-a", "value": "2"}))
    .unwrap();
    assert_eq!(
        var_set(&v.ctx, set).unwrap_err().code,
        "vars.var_not_found",
        "Prod starts empty: set needs add first"
    );
    let add: VarAddRequest = serde_json::from_value(serde_json::json!(
        {"path": r.path, "expectedHash": r.hash, "subset": {"index": 1, "heading": "Prod"},
         "name": "VK-a", "value": "2"}))
    .unwrap();
    let r = var_add(&v.ctx, add).unwrap();
    let doc = read(
        &v.ctx,
        ReadRequest {
            path: r.path.clone(),
        },
    )
    .unwrap();
    assert_eq!(doc.hash, r.hash);
    let heads: Vec<_> = doc
        .artifact
        .blocks
        .iter()
        .map(|b| b.heading.as_str())
        .collect();
    assert_eq!(heads, ["Default", "Prod"]);
    assert_eq!(
        String::from_utf8(v.get(&r.path)).unwrap(),
        "---\nartifactType: Variables\ntitle: Svc\n---\n\n## Default\n\n```yaml VKs\nVK-a: '1'\n```\n\n## Prod\n\n```yaml VKs\nVK-a: '2'\n```\n"
    );
}
