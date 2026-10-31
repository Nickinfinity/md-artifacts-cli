//! `vars.list` / `vars.search` against temp vaults: hostile trees leak nothing, read failures are
//! entries, walk caps refuse the call, errors carry vault-relative params only.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::vars::{ListRequest, SearchRequest, list, search};
use mda_ops::{Ctx, OpError, Root};

#[cfg(unix)]
const MARKER: &str = "SECRET-MARKER-7731";

fn vault(name: &str) -> (PathBuf, Ctx) {
    let d = std::env::temp_dir().join(format!("mda-varslist-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("vault/Variables")).unwrap();
    fs::create_dir_all(d.join("outside")).unwrap();
    let d = d.canonicalize().unwrap();
    let ctx = Ctx::new(Some(Root::new(&d.join("vault")).unwrap()));
    (d, ctx)
}

fn assert_no_abs(e: &OpError, base: &Path) {
    for v in e.params.values() {
        assert!(!Path::new(v).is_absolute(), "absolute param {v}");
        assert!(!v.contains(base.to_str().unwrap()), "root leaked in {v}");
    }
}

fn json(ctx: &Ctx) -> String {
    serde_json::to_string(&list(ctx, ListRequest {}).unwrap()).unwrap()
}

#[cfg(unix)]
#[test]
fn symlinked_dir_out_is_skipped_and_leaks_nothing() {
    let (d, ctx) = vault("dirlink");
    fs::write(
        d.join("outside/secret.md"),
        format!("---\nartifactType: Variables\ntitle: {MARKER}\n---\n"),
    )
    .unwrap();
    std::os::unix::fs::symlink(d.join("outside"), d.join("vault/Variables/out")).unwrap();
    let j = json(&ctx);
    assert!(!j.contains(MARKER), "{j}");
    assert_eq!(j, r#"{"files":[]}"#);
}

#[cfg(unix)]
#[test]
fn symlinked_file_out_is_an_outside_root_entry() {
    let (d, ctx) = vault("filelink");
    fs::write(d.join("outside/s.md"), MARKER).unwrap();
    std::os::unix::fs::symlink(d.join("outside/s.md"), d.join("vault/Variables/leak.md")).unwrap();
    let j = json(&ctx);
    assert!(!j.contains(MARKER), "{j}");
    assert_eq!(
        j,
        r#"{"files":[{"path":"Variables/leak.md","name":"leak","subsets":[],"error":{"code":"path.outside_root"}}]}"#
    );
    // Search keeps an error entry only when its name hits.
    let kept = |q: &str| {
        search(&ctx, SearchRequest { query: q.into() })
            .unwrap()
            .files
            .len()
    };
    assert_eq!(kept("leak"), 1);
    assert_eq!(kept("zzz"), 0);
}

#[test]
fn deep_chain_is_vars_limit_depth() {
    let (d, ctx) = vault("deep");
    let mut p = d.join("vault/Variables");
    for i in 0..33 {
        p.push(format!("d{i}"));
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("x.md"), "").unwrap();
    for e in [
        list(&ctx, ListRequest {}).unwrap_err(),
        search(&ctx, SearchRequest { query: "x".into() }).unwrap_err(),
    ] {
        assert_eq!(e.code, "walk.limit");
        assert_eq!(e.params["limit"], "depth");
        assert_eq!(e.params["max"], "32");
        assert_no_abs(&e, &d);
    }
}

#[cfg(unix)]
#[test]
fn fifo_is_a_not_regular_entry() {
    let (d, ctx) = vault("fifo");
    let fifo = d.join("vault/Variables/p.md");
    let st = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(st.success());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(json(&ctx)).unwrap());
    let j = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("list hung on a FIFO");
    assert_eq!(
        j,
        r#"{"files":[{"path":"Variables/p.md","name":"p","subsets":[],"error":{"code":"file.not_regular"}}]}"#
    );
}

#[test]
fn oversize_file_is_a_too_large_entry() {
    let (d, ctx) = vault("big");
    fs::write(d.join("vault/Variables/big.md"), vec![b'a'; (1 << 20) + 1]).unwrap();
    let j = json(&ctx);
    assert_eq!(
        j,
        r#"{"files":[{"path":"Variables/big.md","name":"big","subsets":[],"error":{"code":"file.too_large"}}]}"#
    );
}

#[cfg(unix)]
#[test]
fn variables_symlinked_out_is_outside_root() {
    let (d, ctx) = vault("rootlink");
    fs::remove_dir(d.join("vault/Variables")).unwrap();
    fs::write(d.join("outside/s.md"), MARKER).unwrap();
    std::os::unix::fs::symlink(d.join("outside"), d.join("vault/Variables")).unwrap();
    for e in [
        list(&ctx, ListRequest {}).unwrap_err(),
        search(&ctx, SearchRequest { query: "a".into() }).unwrap_err(),
    ] {
        assert_eq!(e.code, "path.outside_root");
        assert_no_abs(&e, &d);
    }
}

#[test]
fn search_keeps_the_subset_index_not_the_filtered_position() {
    let (d, ctx) = vault("index");
    fs::write(
        d.join("vault/Variables/s.md"),
        "---\nartifactType: Variables\ntitle: S\n---\n\n## A\n\n```yaml VKs\nVK-a: 1\n```\n\n## B\n\n```yaml VKs\nVK-b: needle\n```\n",
    )
    .unwrap();
    let r = search(
        &ctx,
        SearchRequest {
            query: "needle".into(),
        },
    )
    .unwrap();
    assert_eq!(r.files.len(), 1);
    assert_eq!(r.files[0].subsets.len(), 1);
    assert_eq!(r.files[0].subsets[0].index, 1);
}
