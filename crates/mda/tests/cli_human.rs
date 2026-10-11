//! Human (non-`--json`) CLI output.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{copy_tree, json_lines, run, run_cfg};

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

fn out_text(out: std::process::Output) -> String {
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn types_list_prints_rows() {
    let t = out_text(run(&["--vault", VAULT, "types", "list"], b""));
    assert!(t.contains("Command  Commands/  disabled  (no dir)"), "{t}");
    assert!(t.lines().any(|l| l.ends_with("enabled")), "{t}");
}

#[test]
fn vault_inspect_and_config_get_print() {
    let t = out_text(run(&["vault", "inspect", VAULT], b""));
    assert!(t.contains("  vault"), "{t}");
    assert!(t.contains("Commands/  missing"), "{t}");
    let bare = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault_bare");
    let t = out_text(run(&["vault", "inspect", bare], b""));
    assert!(t.contains("missing defaults: "), "{t}");
    let t = out_text(run(&["--vault", VAULT, "config", "get"], b""));
    assert!(t.contains("source flag"), "{t}");
    assert!(t.contains("vault "), "{t}");
}

#[test]
fn vault_list_marks_default_and_unavailable() {
    let cfg = tmpdir("list-cfg");
    std::fs::write(
        cfg.join("config.toml"),
        format!("default = {VAULT:?}\nvaults = [{VAULT:?}, \"/nonexistent-mda-x\"]\n"),
    )
    .unwrap();
    let t = out_text(run_cfg(&["vault", "list"], b"", &cfg));
    assert!(t.contains(&format!("* {VAULT}")), "{t}");
    assert!(t.contains("  /nonexistent-mda-x  (unavailable)"), "{t}");
    let t = out_text(run_cfg(&["vault", "list"], b"", &tmpdir("list-empty")));
    assert_eq!(t.trim(), "no known vaults");
    std::fs::remove_dir_all(cfg).unwrap();
}

#[test]
fn vault_list_escapes_esc_in_stored_path() {
    let cfg = tmpdir("esc-cfg");
    std::fs::write(
        cfg.join("config.toml"),
        "default = \"/x\\u001b[31m\"\nvaults = [\"/x\\u001b[31m\"]\n",
    )
    .unwrap();
    let out = run_cfg(&["vault", "list"], b"", &cfg);
    assert!(!out.stdout.contains(&0x1b), "{:?}", out.stdout);
    assert!(String::from_utf8(out.stdout).unwrap().contains("u{1b}"));
    std::fs::remove_dir_all(cfg).unwrap();
}
