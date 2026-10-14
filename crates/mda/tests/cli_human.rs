//! Human (non-`--json`) CLI output.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{copy_tree, run};

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

fn temp_vault(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-hw-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(std::path::Path::new(VAULT), &d);
    d
}

const MODEL: &str = r#"{"artifactType":"Template","title":"N","blocks":[{"language":"ts","code":"export const x = 1;"}]}"#;

#[test]
fn new_prints_wrote_path_and_hash() {
    let d = temp_vault("new");
    let v = d.to_str().unwrap();
    let m = d.join("m.json");
    std::fs::write(&m, MODEL).unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "new",
            "Templates/n.md",
            "--model",
            m.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("wrote Templates/n.md  "), "{text}");
    std::fs::remove_dir_all(d).unwrap();
}

// A path with ESC reaches stdout only through the printer.
#[test]
fn new_with_escape_in_path_prints_no_raw_escape() {
    let d = temp_vault("esc");
    let v = d.to_str().unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "new",
            "Templates/e\x1bvil.md",
            "--model",
            "-",
        ],
        MODEL.as_bytes(),
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert!(!out.stdout.contains(&0x1b), "{:?}", out.stdout);
    assert!(!out.stderr.contains(&0x1b), "{:?}", out.stderr);
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn render_prints_output_verbatim() {
    let d = temp_vault("render");
    let v = d.to_str().unwrap();
    let f = d.join("v.json");
    std::fs::write(&f, r#"{"VK-name":"bob"}"#).unwrap();
    let out = run(
        &[
            "--vault",
            v,
            "artifact",
            "render",
            "Snippets/hello.md",
            "--values",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "echo marker-7f3a bob\n"
    );
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
    std::fs::remove_dir_all(d).unwrap();
}

// Sink: a rendered ESC never reaches the terminal; --json still carries it.
#[test]
fn render_with_escape_is_refused_in_human_mode() {
    let out = run(
        &["--vault", VAULT, "artifact", "render", "Templates/esc.md"],
        b"",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.starts_with("render.contains_escape:"), "{err}");
    assert!(!err.contains('\u{1b}'));
    let j = run(
        &[
            "--vault",
            VAULT,
            "artifact",
            "render",
            "Templates/esc.md",
            "--json",
        ],
        b"",
    );
    assert_eq!(j.status.code(), Some(0));
    let lines = common::json_lines(&j.stdout);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["containsEscape"], true);
}

// Warnings go to stderr, human mode only.
#[test]
fn render_warnings_go_to_stderr() {
    let out = run(
        &["--vault", VAULT, "artifact", "render", "Snippets/hello.md"],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("warning: render.unknown_var:"), "{err}");
    assert!(String::from_utf8(out.stdout).unwrap().contains("<VK-name>"));
    let j = run(
        &[
            "--vault",
            VAULT,
            "artifact",
            "render",
            "Snippets/hello.md",
            "--json",
        ],
        b"",
    );
    assert!(j.stderr.is_empty(), "{:?}", j.stderr);
}
