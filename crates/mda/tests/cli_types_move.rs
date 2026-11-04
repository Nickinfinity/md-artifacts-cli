//! `mda types move plan|apply` through the real binary: human text, `--files -` from a plan.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::{copy_tree, run_cfg};
use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn plan_then_apply_human_and_json() {
    let t = std::env::temp_dir().join(format!("mda-cli-mv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    let (src, b, cfg) = (t.join("src"), t.join("b"), t.join("cfg"));
    copy_tree(&fixture("vault_move_src"), &src);
    copy_tree(&fixture("vault_loc_b"), &b);
    let (s, bs) = (src.to_str().unwrap(), b.to_str().unwrap());
    let base = ["--vault", s];
    let go = |extra: &[&str], stdin: &[u8]| {
        let args: Vec<&str> = base.iter().chain(extra).copied().collect();
        run_cfg(&args, stdin, &cfg)
    };

    let human = go(
        &[
            "types", "move", "plan", "Snippet", "--at", bs, "--dir", "New",
        ],
        b"",
    );
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("Snippets/a.md  fd99dedae"), "{text}");
    assert!(
        text.ends_with("2 file(s), 8 byte(s), 0 conflict(s)\n"),
        "{text}"
    );

    let plan = go(
        &[
            "--json", "types", "move", "plan", "Snippet", "--at", bs, "--dir", "New",
        ],
        b"",
    );
    let done = go(
        &[
            "types", "move", "apply", "Snippet", "--at", bs, "--dir", "New", "--files", "-",
        ],
        &plan.stdout,
    );
    let text = String::from_utf8(done.stdout).unwrap();
    assert!(text.starts_with("moved 2 file(s) to "), "{text}");
    assert_eq!(std::fs::read(b.join("New/a.md")).unwrap(), b"# a\n");
    assert!(!src.join("Snippets/a.md").exists());
}
