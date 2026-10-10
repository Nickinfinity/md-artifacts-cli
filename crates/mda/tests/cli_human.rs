//! Human (non-`--json`) CLI output.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::run;

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");

#[test]
fn ops_one_per_line() {
    let out = run(&["system", "ops"], b"");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    for op in mda_ops::ops() {
        assert!(
            text.lines()
                .any(|l| l.split_whitespace().next() == Some(op.name)),
            "{}",
            op.name
        );
    }
}

#[test]
fn version_shows_engine_and_protocol() {
    let out = run(&["system", "version"], b"");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
    assert!(text.contains(mda_ops::PROTOCOL), "{text}");
}

#[test]
fn error_params_have_no_raw_escape() {
    let out = run(&["call", "x\x1by"], b"");
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.contains(&0x1b), "{:?}", out.stderr);
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("op.unknown"));
}

#[test]
fn show_prints_title_and_code() {
    let out = run(
        &["--vault", VAULT, "artifact", "show", "Snippets/hello.md"],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Hello"), "{text}");
    assert!(text.contains("echo <VK-greeting>"), "{text}");
}

#[test]
fn ls_lists_dirs_and_titles() {
    let out = run(&["--vault", VAULT, "artifact", "ls", "Snippet"], b"");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.lines().any(|l| l == "sub/"), "{text}");
    assert!(text.lines().any(|l| l == "hello  Hello"), "{text}");
}

#[test]
fn vault_text_never_reaches_stdout_with_escape() {
    let d = std::env::temp_dir().join(format!("mda-esc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join(".obsidian")).unwrap();
    std::fs::create_dir_all(d.join("Snippets")).unwrap();
    std::fs::write(
        d.join("Snippets/e\x1bvil.md"),
        "---\ntitle: t\x1b[31m\n---\n\n```bash\nx\x1b[1m\n```\n",
    )
    .unwrap();
    let v = d.to_str().unwrap();
    for args in [
        ["--vault", v, "artifact", "ls", "Snippet"],
        ["--vault", v, "artifact", "show", "Snippets/e\x1bvil.md"],
    ] {
        let out = run(&args, b"");
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert!(!out.stdout.contains(&0x1b), "{:?}", out.stdout);
        assert!(!out.stdout.is_empty());
    }
}
