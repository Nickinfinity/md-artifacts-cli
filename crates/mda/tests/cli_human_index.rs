//! Human (non-`--json`) CLI output: `mda index` (split from `cli_human.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{copy_tree, json_lines, run};

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-human-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn index_plan_prints_steps_and_rejections() {
    let out = run(
        &["--vault", VAULT, "index", "plan", "Templates/index.md"],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\"edit.md\"  -> \"\""), "{text}");
    assert!(
        text.contains("rejected \"../x\"  path.parent_segment"),
        "{text}"
    );
    assert!(text.contains("  skip artifact.not_whole_file"), "{text}");
}

#[test]
fn index_run_prints_wrote_and_skipped() {
    let ws = tmpdir("run-ws");
    let steps = ws.join("s.json");
    std::fs::write(
        &steps,
        r#"[{"relPath":"edit.md","destDir":""},{"relPath":"sub/widget.md","destDir":"sub"}]"#,
    )
    .unwrap();
    let out = run(
        &[
            "--vault",
            VAULT,
            "index",
            "run",
            "Templates/index.md",
            "--workspace",
            ws.to_str().unwrap(),
            "--steps",
            steps.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("wrote Edit me.ts  "), "{text}");
    assert!(text.contains("wrote sub/Widget.ts  "), "{text}");
    assert!(
        text.ends_with("2 written, 0 skipped") || text.trim_end().ends_with("2 written, 0 skipped"),
        "{text}"
    );

    let out = run(
        &[
            "--vault",
            VAULT,
            "index",
            "run",
            "Templates/index.md",
            "--workspace",
            ws.to_str().unwrap(),
            "--steps",
            "-",
        ],
        br#"[{"relPath":"missing.md","destDir":""}]"#,
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(
            "skipped \"missing.md\"  path.not_found: path does not exist path=\"Templates/missing.md\""
        ),
        "{text}"
    );
    assert!(text.contains("0 written, 1 skipped"), "{text}");
    std::fs::remove_dir_all(ws).unwrap();
}

#[test]
fn index_create_prints_wrote_lines() {
    let vault = tmpdir("create-vault");
    copy_tree(std::path::Path::new(VAULT), &vault);
    let out = run(
        &[
            "--vault",
            vault.to_str().unwrap(),
            "index",
            "create",
            "--type",
            "Template",
            "--dir",
            "Templates/gen",
            "a.ts",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("wrote Templates/gen/a.md  "), "{text}");
    assert!(text.contains("wrote Templates/gen/index.md  "), "{text}");
    assert!(text.contains("2 written, 0 skipped"), "{text}");
    std::fs::remove_dir_all(vault).unwrap();
}

// Terminal safety: a vault-authored ESC in link text never reaches stdout raw.
#[test]
fn index_plan_escapes_esc_in_links() {
    let vault = tmpdir("esc-vault");
    copy_tree(std::path::Path::new(VAULT), &vault);
    std::fs::write(
        vault.join("Templates/escidx.md"),
        "---\nartifactType: Template\ntitle: E\nindex: true\n---\n\n[[a\u{1b}b]]\n",
    )
    .unwrap();
    let v = vault.to_str().unwrap();
    let out = run(&["--vault", v, "index", "plan", "Templates/escidx.md"], b"");
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stdout.contains(&0x1b), "{:?}", out.stdout);
    assert!(String::from_utf8(out.stdout).unwrap().contains("a\\u{1b}b"));
    let out = run(
        &[
            "--vault",
            v,
            "index",
            "plan",
            "Templates/escidx.md",
            "--json",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stdout.contains(&0x1b));
    let j = json_lines(&out.stdout);
    assert_eq!(j[0]["rejected"][0]["raw"], "a\u{1b}b");
    std::fs::remove_dir_all(vault).unwrap();
}
