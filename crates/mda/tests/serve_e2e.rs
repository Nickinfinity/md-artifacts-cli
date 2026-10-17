//! `mda serve` end to end through the real binary: a long traced session, backpressure, EOF in
//! the middle of a request, and SIGKILL during a write. Expected hashes and bytes come from
//! in-process `dispatch` on a scratch vault, computed before the binary runs.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: tests fail loudly

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

use mda_ops::{Ctx, Root, dispatch};
use serde_json::{Value, json};

const SIZE: usize = 150_000;
const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"1.0"}}"#;

/// Removes this test's `mda-e2e-<pid>-<tag>*` temp dirs on drop (also on panic).
struct Clean(&'static str);
impl Drop for Clean {
    fn drop(&mut self) {
        let prefix = format!("mda-e2e-{}-{}", std::process::id(), self.0);
        for e in std::fs::read_dir(std::env::temp_dir())
            .into_iter()
            .flatten()
            .flatten()
        {
            if e.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-e2e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

/// A vault dir (with `.obsidian/`) and an in-process context on it.
fn vault(name: &str) -> (PathBuf, Ctx) {
    let v = tmp(name);
    std::fs::create_dir_all(v.join(".obsidian")).unwrap();
    let ctx = Ctx::new(Some(Root::new(&v).unwrap()));
    (v, ctx)
}

fn model(title: &str, code: &str) -> Value {
    json!({"artifactType": "Template", "title": title, "blocks": [{"code": code}]})
}

fn hash_of(r: &Value) -> String {
    r["hash"].as_str().unwrap().to_owned()
}

fn req(id: u32, method: &str, params: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

fn spawn_serve(args: &[&str], cfg: &Path) -> Child {
    let mut cmd = common::bin();
    cmd.env("MDA_CONFIG_DIR", cfg)
        .args(args)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().expect("spawn mda serve")
}

/// Params for `Templates/a.md` plus `extra`.
fn pa(extra: Value) -> Value {
    let mut v = json!({"path": "Templates/a.md"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    v
}

/// (H1, H2): the create's hash and the first update's, computed in-process on a scratch vault.
fn precompute_h1_h2() -> (String, String) {
    let (_scratch, ctx) = vault("long-scratch");
    let h1 = hash_of(
        &dispatch(
            &ctx,
            "artifact.create",
            pa(json!({"model": model("A", "one")})),
        )
        .unwrap(),
    );
    let upd = pa(json!({"expectedHash": h1, "model": model("A", "two")}));
    let h2 = hash_of(&dispatch(&ctx, "artifact.update", upd).unwrap());
    assert_ne!(h1, h2);
    (h1, h2)
}

#[test]
fn long_session_hashes_select_under_trace() {
    let _clean = Clean("long");
    let (h1, h2) = precompute_h1_h2();
    let (v, _) = vault("long-live");
    let lines = [
        INIT.to_owned(),
        req(2, "vault.select", &json!({"path": v})),
        req(
            3,
            "artifact.create",
            &pa(json!({"model": model("A", "one")})),
        ),
        req(4, "artifact.read", &pa(json!({}))),
        req(
            5,
            "artifact.update",
            &pa(json!({"expectedHash": h1, "model": model("A", "two")})),
        ),
        req(
            6,
            "artifact.update",
            &pa(json!({"expectedHash": h1, "model": model("A", "three")})),
        ),
        req(7, "artifact.read", &pa(json!({}))),
        req(8, "system.ops", &json!({})),
    ];
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let out = common::run_cfg(
        &["--debug=trace", "serve"],
        input.as_bytes(),
        &tmp("long-cfg"),
    );
    assert!(out.status.success());
    assert!(out.stdout.ends_with(b"\n"));
    let got = common::json_lines(&out.stdout);
    assert_eq!(got.len(), 8);
    for (i, g) in got.iter().enumerate() {
        assert_eq!(g["id"], json!(i + 1));
    }
    assert_eq!(got[2]["result"]["hash"], json!(h1));
    assert_eq!(got[3]["result"]["hash"], json!(h1));
    assert_eq!(got[4]["result"]["hash"], json!(h2));
    assert_eq!(got[5]["error"]["code"], -32000);
    assert_eq!(
        got[5]["error"]["data"],
        json!({"code": "file.conflict", "params": {"path": "Templates/a.md", "expected": h1, "actual": h2}})
    );
    assert_eq!(got[6]["result"]["hash"], json!(h2));
    // Disk holds the first update's bytes, not the refused second write.
    let disk = std::fs::read_to_string(v.join("Templates/a.md")).unwrap();
    assert!(disk.contains("two") && !disk.contains("three"), "{disk}");
    let err = String::from_utf8(out.stderr).unwrap();
    let first = err.lines().next().unwrap();
    assert!(
        first.contains("WARN mda::debug trace output may include secrets"),
        "{first}"
    );
}

#[test]
fn pipelined_under_backpressure() {
    let _clean = Clean("bp");
    const N: u32 = 2000;
    let mut child = spawn_serve(&[], &tmp("bp-cfg"));
    let mut stdin = child.stdin.take().unwrap();
    // A writer thread: writing all 2 000 requests before reading would fill both pipes and deadlock.
    let writer = std::thread::spawn(move || {
        writeln!(stdin, "{INIT}").unwrap();
        for id in 2..=N + 1 {
            writeln!(stdin, "{}", req(id, "system.version", &json!({}))).unwrap();
        }
    });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert!(out.status.success());
    let got = common::json_lines(&out.stdout);
    assert_eq!(got.len(), (N + 1) as usize);
    for (i, g) in got.iter().enumerate() {
        assert_eq!(g["id"], json!(i + 1));
    }
}

#[test]
fn eof_mid_request() {
    let _clean = Clean("eof");
    let (v, ctx) = vault("eof");
    let m = model("A", "one");
    let h = hash_of(
        &dispatch(
            &ctx,
            "artifact.create",
            json!({"path": "Templates/a.md", "model": m}),
        )
        .unwrap(),
    );
    let before = std::fs::read(v.join("Templates/a.md")).unwrap();
    let full = req(
        2,
        "artifact.update",
        &json!({"path": "Templates/a.md", "expectedHash": h, "model": model("A", "two")}),
    );
    let cut = &full[..full.len() - 10]; // no trailing newline
    let input = format!("{INIT}\n{cut}");
    let out = common::run_cfg(
        &["--vault", v.to_str().unwrap(), "serve"],
        input.as_bytes(),
        &tmp("eof-cfg"),
    );
    assert_eq!(out.status.code(), Some(0));
    let got = common::json_lines(&out.stdout);
    assert_eq!(got.len(), 2);
    assert_eq!(got[1]["error"]["code"], -32700);
    assert_eq!(got[1]["error"]["data"]["code"], "protocol.parse_error");
    assert_eq!(std::fs::read(v.join("Templates/a.md")).unwrap(), before);
}

const KILL_PATH: &str = "Templates/big.md";

/// Bytes and hash of `Templates/big.md` as the A-file (all 'a') and the B-file (all 'b'), via
/// in-process dispatch; leaves A on disk.
fn seed_ab(ctx: &Ctx, abs: &Path) -> (Vec<u8>, String, Vec<u8>, String) {
    // ponytail: 150 000 bytes, not 900 000 — a debug-build parse costs ~0.55 s/MB, so the larger
    // payload cannot fit 40 kill points in 10 s; still far above one pipe buffer.
    let read = || hash_of(&dispatch(ctx, "artifact.read", json!({"path": KILL_PATH})).unwrap());
    let m = |c: char| model("Big", &c.to_string().repeat(SIZE));
    dispatch(
        ctx,
        "artifact.create",
        json!({"path": KILL_PATH, "model": m('a')}),
    )
    .unwrap();
    let (bytes_a, hash_a) = (std::fs::read(abs).unwrap(), read());
    let upd = json!({"path": KILL_PATH, "expectedHash": hash_a, "model": m('b')});
    dispatch(ctx, "artifact.update", upd).unwrap();
    let (bytes_b, hash_b) = (std::fs::read(abs).unwrap(), read());
    assert_ne!(bytes_a, bytes_b);
    std::fs::write(abs, &bytes_a).unwrap(); // seed A
    (bytes_a, hash_a, bytes_b, hash_b)
}

/// Spawn serve on `v`, send init + one update, kill after `delay_ms`, reap.
fn kill_once(v: &Path, cfg: &Path, update: &Value, delay_ms: u64) {
    let line = format!("{INIT}\n{}\n", req(2, "artifact.update", update));
    let mut child = spawn_serve(&["--vault", v.to_str().unwrap()], cfg);
    let mut stdin = child.stdin.take().unwrap();
    let w = std::thread::spawn(move || {
        let _ = stdin.write_all(line.as_bytes()); // broken pipe after the kill is expected
    });
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    child.kill().unwrap();
    let _ = child.wait_with_output(); // drains pipes, reaps
    w.join().unwrap();
}

/// Characterization pin of the crash sink (bytes in {A, B} + the `read_dir` rule); atomicity holds
/// by construction in `write.rs` `with_temp` (temp, fsync, rename), pinned by its unit tests.
#[test]
fn kill_during_write_leaves_old_or_new() {
    let _clean = Clean("kill");
    let started = std::time::Instant::now();
    let (v, ctx) = vault("kill");
    let abs = v.join(KILL_PATH);
    let (bytes_a, hash_a, bytes_b, hash_b) = seed_ab(&ctx, &abs);
    let cfg = tmp("kill-cfg");
    let (mut new, mut temps) = (0, 0);
    for i in 0..40u64 {
        let a_now = std::fs::read(&abs).unwrap() == bytes_a;
        let (to, h, other) = if a_now {
            ('b', &hash_a, &bytes_b)
        } else {
            ('a', &hash_b, &bytes_a)
        };
        let upd = json!({"path": KILL_PATH, "expectedHash": h, "model": model("Big", &to.to_string().repeat(SIZE))});
        kill_once(&v, &cfg, &upd, i * 5);
        let now = std::fs::read(&abs).unwrap();
        assert!(
            now == bytes_a || now == bytes_b,
            "iteration {i}: torn file ({} bytes)",
            now.len()
        );
        new += usize::from(now == *other);
        // Sink: every other directory entry is a hidden temp, never an .md file.
        for e in std::fs::read_dir(abs.parent().unwrap()).unwrap() {
            let n = e.unwrap().file_name().into_string().unwrap();
            let temp = n.starts_with(".big.md.mda-tmp-") && !n.ends_with(".md");
            assert!(n == "big.md" || temp, "iteration {i}: stray entry {n}");
            temps += usize::from(temp);
        }
    }
    // Characterization: the tree lists only .md names, so temps never show.
    let tree = dispatch(&ctx, "artifact.tree", json!({"type": "Template"})).unwrap();
    let paths: Vec<_> = tree["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].clone())
        .collect();
    assert_eq!(paths, vec![json!(KILL_PATH)]);
    eprintln!(
        "kill test: {new} landed new, {} stayed old, {temps} temp sightings (summed per iteration)",
        40 - new
    );
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
}
