//! `vault.validate` against temp vaults: hostile trees are entries or refusals, nothing leaks, the
//! issue list is capped, "pending" agrees with the migrations.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::validate::{ValidateRequest, ValidateResponse, validate};
use mda_ops::{Ctx, OpError, Root};

const MARKER: &str = "SECRET-MARKER-5512";
const FM: &str = "---\nartifactType: Variables\ntitle: T\n---\n";

fn vault(name: &str) -> (PathBuf, Ctx) {
    let d = std::env::temp_dir().join(format!("mda-validate-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("vault/Variables")).unwrap();
    fs::create_dir_all(d.join("vault/Snippets")).unwrap();
    fs::create_dir_all(d.join("vault/Commands")).unwrap();
    fs::create_dir_all(d.join("outside")).unwrap();
    let d = d.canonicalize().unwrap();
    let ctx = Ctx::new(Some(Root::new(&d.join("vault")).unwrap()));
    (d, ctx)
}

fn run(ctx: &Ctx) -> ValidateResponse {
    validate(ctx, ValidateRequest {}).unwrap()
}

fn assert_no_abs(r: &ValidateResponse, base: &Path) {
    for i in &r.issues {
        for v in i.params.values() {
            assert!(!Path::new(v).is_absolute(), "absolute param {v}");
            assert!(!v.contains(base.to_str().unwrap()), "root leaked in {v}");
        }
    }
    assert!(!serde_json::to_string(r).unwrap().contains(MARKER));
}

fn codes(r: &ValidateResponse, path: &str) -> Vec<&'static str> {
    r.issues
        .iter()
        .filter(|i| i.path == path)
        .map(|i| i.code)
        .collect()
}

#[cfg(unix)]
#[test]
fn fifo_is_an_error_entry_and_returns() {
    let (d, ctx) = vault("fifo");
    let fifo = d.join("vault/Snippets/p.md");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(run(&ctx)).unwrap());
    let r = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert_eq!(codes(&r, "Snippets/p.md"), ["file.not_regular"]);
    assert_eq!((r.files, r.errors), (1, 1));
}

#[cfg(unix)]
#[test]
fn file_link_out_is_an_error_and_leaks_nothing() {
    let (d, ctx) = vault("filelink");
    fs::write(
        d.join("outside/s.md"),
        format!("---\ntype: snippet\n---\n{MARKER}\n"),
    )
    .unwrap();
    std::os::unix::fs::symlink(d.join("outside/s.md"), d.join("vault/Snippets/leak.md")).unwrap();
    let r = run(&ctx);
    assert_eq!(codes(&r, "Snippets/leak.md"), ["path.outside_root"]);
    assert_eq!(r.errors, 1);
    assert_no_abs(&r, &d);
}

#[cfg(unix)]
#[test]
fn dir_loop_is_bounded() {
    let (d, ctx) = vault("loop");
    std::os::unix::fs::symlink("..", d.join("vault/Variables/loop")).unwrap();
    fs::write(d.join("vault/Variables/a.md"), FM).unwrap();
    let r = run(&ctx);
    assert_eq!((r.files, r.errors), (1, 0));
}

#[test]
fn deep_chain_refuses_the_call() {
    let (d, ctx) = vault("deep");
    let mut p = d.join("vault/Commands");
    for _ in 0..33 {
        p = p.join("d");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("x.md"), "x").unwrap();
    let e: OpError = validate(&ctx, ValidateRequest {}).unwrap_err();
    assert_eq!(e.code, "walk.limit");
    assert_eq!(e.params["limit"], "depth");
}

#[test]
fn oversize_file_is_an_error_entry() {
    let (d, ctx) = vault("big");
    fs::write(d.join("vault/Snippets/big.md"), vec![b'a'; 1_048_577]).unwrap();
    let r = run(&ctx);
    assert_eq!(codes(&r, "Snippets/big.md"), ["file.too_large"]);
    assert_no_abs(&r, &d);
}

#[test]
fn sets_file_with_two_bad_blocks_reports_file_once_and_block_one() {
    let (d, ctx) = vault("blocks");
    let body = format!(
        "{FM}\n## A\n\n```yaml VKs\nVK-a: 'x\u{1}y'\n```\n\n## B\n\n```yaml VKs\nVK-b: ok\nVK-c: 'x\u{1}y'\n```\n"
    );
    fs::write(d.join("vault/Variables/s.md"), body).unwrap();
    let r = run(&ctx);
    let ours: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i.path == "Variables/s.md")
        .collect();
    assert_eq!(ours.len(), 2, "{ours:?}");
    assert!(ours.iter().all(|i| i.code == "vks.control_char"));
    assert!(!ours[0].params.contains_key("block"));
    assert_eq!(ours[1].params["block"], "1");
    assert_eq!(r.errors, 2);
}

#[test]
fn issue_list_is_capped_but_counts_are_exact() {
    let (d, ctx) = vault("cap");
    fs::write(
        d.join("vault/Variables/many.md"),
        format!("{FM}\n{}", "## a\n".repeat(10_001)),
    )
    .unwrap();
    let r = run(&ctx);
    assert_eq!(r.warnings, 10_001);
    assert_eq!(r.issues.len(), 10_000);
    assert!(r.truncated);
}

#[test]
fn type_key_in_a_foreign_dir_is_still_pending() {
    let (d, ctx) = vault("foreign");
    fs::write(
        d.join("vault/Commands/c.md"),
        "---\ntype: snippet\n---\nx\n",
    )
    .unwrap();
    let r = run(&ctx);
    assert_eq!(codes(&r, "Commands/c.md"), ["migrate.pending"]);
    assert_eq!(r.issues[0].params["kind"], "frontmatter");
}

#[test]
fn comment_only_body_is_not_pending() {
    let (d, ctx) = vault("comment");
    fs::write(
        d.join("vault/Variables/c.md"),
        format!("{FM}\n```yaml VKs\n# todo\n```\n"),
    )
    .unwrap();
    assert!(run(&ctx).issues.is_empty());
}

#[test]
fn identical_block_errors_are_each_reported() {
    let (d, ctx) = vault("identical");
    let body = format!(
        "{FM}\n## A\n\n```yaml VKs\nVK-a: 'x\u{1}y'\n```\n\n## B\n\n```yaml VKs\nVK-a: 'x\u{1}y'\n```\n"
    );
    fs::write(d.join("vault/Variables/s.md"), body).unwrap();
    let r = run(&ctx);
    let ours: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i.path == "Variables/s.md")
        .collect();
    assert_eq!(ours.len(), 2, "{ours:?}");
    assert!(ours.iter().all(|i| i.code == "vks.control_char"));
    assert!(!ours[0].params.contains_key("block"));
    assert_eq!(ours[1].params["block"], "1");
    assert_eq!(r.errors, 2);
}
