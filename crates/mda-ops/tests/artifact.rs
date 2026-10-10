//! `artifact.read` / `artifact.tree` against temp vaults: error shape, containment sinks, order.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::PathBuf;

use mda_ops::artifact::{ReadRequest, TreeRequest, read, tree};
use mda_ops::{ArtifactType, Ctx, OpError, Root, dispatch};
use serde_json::json;

struct Vault {
    dir: PathBuf,
    ctx: Ctx,
    canon: String,
}

fn vault(name: &str) -> Vault {
    let dir = std::env::temp_dir().join(format!("mda-art-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("Snippets")).unwrap();
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
        let p = self.dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }
    fn outside(&self, name: &str) -> PathBuf {
        let d = self.dir.with_file_name(format!(
            "{}-out",
            self.dir.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&d).unwrap();
        d.join(name)
    }
    /// No error param may contain the machine path of the vault root (A2 #16).
    fn assert_no_abs(&self, e: &OpError) {
        for (k, v) in &e.params {
            assert!(
                !v.contains(&self.canon),
                "param {k}={v} leaks {}",
                self.canon
            );
        }
    }
    fn read_err(&self, path: &str) -> OpError {
        let e = read(&self.ctx, ReadRequest { path: path.into() }).unwrap_err();
        self.assert_no_abs(&e);
        e
    }
    fn tree(&self, dir: Option<&str>) -> Result<mda_ops::artifact::TreeResponse, OpError> {
        tree(
            &self.ctx,
            TreeRequest {
                artifact_type: ArtifactType::Snippet,
                dir: dir.map(Into::into),
            },
        )
    }
}

#[test]
fn read_not_found_names_relative_path() {
    let v = vault("nf");
    let e = v.read_err("Snippets/nope.md");
    assert_eq!(e.code, "path.not_found");
    assert_eq!(e.params["path"], "Snippets/nope.md");
}

#[test]
fn read_too_large() {
    let v = vault("big");
    v.put("Snippets/big.md", &"a".repeat((1 << 20) + 1));
    assert_eq!(v.read_err("Snippets/big.md").code, "file.too_large");
}

#[test]
fn read_not_regular_dir() {
    let v = vault("dir");
    fs::create_dir_all(v.dir.join("Snippets/d.md")).unwrap();
    assert_eq!(v.read_err("Snippets/d.md").code, "file.not_regular");
}

#[cfg(unix)]
#[test]
fn symlink_out_is_refused_and_secret_never_returned() {
    let v = vault("link");
    let secret = v.outside("secret.md");
    fs::write(&secret, "---\ntitle: SECRET-TITLE\n---\n").unwrap();
    std::os::unix::fs::symlink(&secret, v.dir.join("Snippets/out.md")).unwrap();
    assert_eq!(v.read_err("Snippets/out.md").code, "path.outside_root");
    let resp = v.tree(None).unwrap();
    assert_eq!(
        resp.files[0].error.as_ref().unwrap().code,
        "path.outside_root"
    );
    assert!(
        !serde_json::to_string(&resp)
            .unwrap()
            .contains("SECRET-TITLE")
    );
}

#[cfg(unix)]
#[test]
fn symlinked_dir_out_fails_listing_without_leaking_names() {
    let v = vault("linkdir");
    let out = v.outside("d");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("outside-name.md"), "x").unwrap();
    std::os::unix::fs::symlink(&out, v.dir.join("Snippets/linkdir")).unwrap();
    let e = v.tree(Some("linkdir")).unwrap_err();
    v.assert_no_abs(&e);
    assert_eq!(e.code, "path.outside_root");
    assert!(!format!("{e:?}").contains("outside-name"));
}

#[test]
fn tree_missing_dir_errors_but_missing_type_is_empty() {
    let v = vault("missing");
    let e = v.tree(Some("nope")).unwrap_err();
    v.assert_no_abs(&e);
    assert_eq!(
        (e.code, e.params["path"].as_str()),
        ("path.not_found", "Snippets/nope")
    );
    fs::remove_dir(v.dir.join("Snippets")).unwrap();
    assert_eq!(v.tree(None).unwrap(), Default::default());
}

#[cfg(unix)]
#[test]
fn fifo_is_not_regular_and_never_blocks() {
    let v = vault("fifo");
    let fifo = v.dir.join("Snippets/x.md");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let vault_dir = v.dir.clone();
    std::thread::spawn(move || {
        let ctx = Ctx::new(Some(Root::new(&vault_dir).unwrap()));
        let t = dispatch(&ctx, "artifact.tree", json!({"type": "Snippet"}));
        let r = dispatch(&ctx, "artifact.read", json!({"path": "Snippets/x.md"}));
        let _ = tx.send((t, r));
    });
    let (t, r) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("blocked on FIFO");
    assert_eq!(t.unwrap()["files"][0]["error"]["code"], "file.not_regular");
    let e = r.unwrap_err();
    v.assert_no_abs(&e);
    assert_eq!(e.code, "file.not_regular");
}

#[test]
fn record_key_order_survives_serialisation() {
    let v = vault("order");
    v.put(
        "Variables/structured.md",
        "---\nartifactType: Variables\n---\n\n```vks\nVK-db:\n  zeta: z\n  alpha: a\n```\n",
    );
    let r = dispatch(
        &v.ctx,
        "artifact.read",
        json!({"path": "Variables/structured.md"}),
    );
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""zeta":"z","alpha":"a""#), "{s}");
}

#[test]
fn nested_path_reads() {
    let v = vault("nested");
    v.put("Snippets/sub/n.md", "---\ntitle: N\n---\n");
    let req = ReadRequest {
        path: "Snippets/sub/n.md".into(),
    };
    assert_eq!(read(&v.ctx, req).unwrap().artifact.file_name, "n");
}
