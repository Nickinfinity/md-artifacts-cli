//! `mda migrate …` and `mda vault validate` through the real binary (T6.5).
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::{copy_tree, json_lines, run};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn copy(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-cli-mig-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(&fixture("vault_migrate"), &d);
    d
}

fn go(v: &Path, args: &[&str]) -> (Option<i32>, Vec<String>, String) {
    let mut a = vec!["--vault", v.to_str().unwrap()];
    a.extend_from_slice(args);
    let o = run(&a, b"");
    let out = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    (
        o.status.code(),
        out,
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn frontmatter_plan_apply_plan() {
    let v = copy("fm");
    let before = std::fs::read(v.join("Snippets/legacy.md")).unwrap();
    let (rc, out, _) = go(&v, &["migrate", "frontmatter", "plan"]);
    assert_eq!(rc, Some(0));
    assert_eq!(
        out[0],
        r#"Snippets/bom.md  "type: snippet" -> "artifactType: Snippet"  (normalises bom)"#
    );
    assert_eq!(out.last().unwrap(), "5 to change, 2 skipped");
    assert_eq!(std::fs::read(v.join("Snippets/legacy.md")).unwrap(), before);

    let (rc, out, _) = go(&v, &["migrate", "frontmatter", "apply"]);
    assert_eq!(rc, Some(0));
    assert_eq!(out.iter().filter(|l| l.starts_with("wrote ")).count(), 5);
    assert_eq!(out.last().unwrap(), "5 written, 0 skipped");

    let (_, out, _) = go(&v, &["migrate", "frontmatter", "plan"]);
    assert_eq!(out.last().unwrap(), "0 to change, 2 skipped");
    std::fs::remove_dir_all(&v).unwrap();
}

#[test]
fn vks_apply_and_json_plan() {
    let v = copy("vks");
    let (_, out, _) = go(&v, &["--json", "migrate", "vks", "plan"]);
    assert_eq!(out.len(), 1);
    let got: serde_json::Value = serde_json::from_str(&out[0]).unwrap();
    let case = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/op_cases/migrate.vks.plan/ok.toml"),
    )
    .unwrap();
    let case: toml::Value = toml::from_str(&case).unwrap();
    let want = serde_json::to_value(&case["expect"]).unwrap();
    assert_eq!(got, want);

    let (rc, out, _) = go(&v, &["migrate", "vks", "apply"]);
    assert_eq!(rc, Some(0));
    assert_eq!(out.last().unwrap(), "5 written, 0 skipped");
    std::fs::remove_dir_all(&v).unwrap();
}

#[test]
fn validate_exit_codes() {
    let v = copy("val");
    let (rc, out, _) = go(&v, &["vault", "validate"]);
    assert_eq!(rc, Some(1));
    assert_eq!(out.last().unwrap(), "20 files, 1 errors, 13 warnings");

    let (rc, out, _) = go(&v, &["--json", "vault", "validate"]);
    assert_eq!(rc, Some(1));
    assert_eq!(out.len(), 1);
    assert!(serde_json::from_str::<serde_json::Value>(&out[0]).is_ok());

    let (rc, _, _) = go(&v, &["call", "vault.validate", "{}"]);
    assert_eq!(rc, Some(0));

    let (rc, _, _) = go(&fixture("vault_vars"), &["vault", "validate"]);
    assert_eq!(rc, Some(0));
    std::fs::remove_dir_all(&v).unwrap();
}

#[test]
fn validate_without_vault_fails() {
    let o = run(&["vault", "validate"], b"");
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("vault.not_selected:"));
    assert_eq!(
        json_lines(&run(&["--json", "system", "ops"], b"").stdout).len(),
        1
    );
}

#[cfg(unix)]
#[test]
fn esc_in_path_is_escaped() {
    let v = copy("esc");
    std::fs::write(v.join("Snippets/e\x1bx.md"), "---\ntype: snippet\n---\nb\n").unwrap();
    let o = run(
        &[
            "--vault",
            v.to_str().unwrap(),
            "migrate",
            "frontmatter",
            "plan",
        ],
        b"",
    );
    assert!(o.status.success());
    assert!(!o.stdout.contains(&0x1b), "raw ESC on stdout");
    assert!(String::from_utf8_lossy(&o.stdout).contains("e\\u{1b}x.md"));
    std::fs::remove_dir_all(&v).unwrap();
}
