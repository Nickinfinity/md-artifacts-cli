//! `mda vars list|search`, `mda varsets …`, `mda render text` through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{copy_tree, json_lines, run};

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault_vars");

fn vault(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("mda-cli-varsets-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(std::path::Path::new(FIX), &d);
    d.to_str().unwrap().to_owned()
}

fn go(v: &str, args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut a = vec!["--vault", v];
    a.extend_from_slice(args);
    run(&a, stdin)
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn err(o: &std::process::Output) -> String {
    String::from_utf8(o.stderr.clone()).unwrap()
}

fn values_file(v: &str, body: &str) -> String {
    let p = std::path::Path::new(v).with_extension("values.json");
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_owned()
}

#[test]
fn list_first_line_is_dev_flat_with_tag() {
    let v = vault("list");
    let o = go(&v, &["vars", "list"], b"");
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let text = out(&o);
    assert_eq!(
        text.lines().next().unwrap(),
        "Variables/dev.md  Dev  [flat]  #api"
    );
    assert!(text.contains("\n  VK-host = localhost\n"), "{text}");
    assert!(text.contains("\n  ## Stage Staging box.\n    VK-host = stage.example.com\n"));
    assert!(text.lines().all(|l| l == l.trim_end()), "{text:?}");
}

#[test]
fn search_shows_only_the_matching_var() {
    let v = vault("search");
    let o = go(&v, &["vars", "search", "eu-west"], b"");
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let lines: Vec<String> = out(&o).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0], "Variables/envs.md  Envs  [sets]");
    assert_eq!(lines[1], "  ## Prod");
    assert!(lines[2].starts_with("    VK-db = "), "{lines:?}");
    assert!(lines[2].contains("eu-west"));
}

#[test]
fn match_prints_four_ranked_lines() {
    let v = vault("match");
    let o = go(&v, &["varsets", "match", "Snippets/api.md"], b"");
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let t = out(&o);
    let lines: Vec<&str> = t.lines().collect();
    assert_eq!(lines.len(), 4, "{t}");
    assert_eq!(lines[0], "0.767  Variables/dev.md  Dev  (2/3 vars, 1 tags)");
}

#[test]
fn match_json_is_the_op_response() {
    let v = vault("matchjson");
    let o = go(&v, &["--json", "varsets", "match", "Snippets/api.md"], b"");
    let lines = json_lines(&o.stdout);
    assert_eq!(lines.len(), 1);
    let expect = serde_json::json!({
        "artifactVars": ["VK-host", "VK-port", "VK-path"],
        "paths": ["Variables/dev.md", "Variables/envs.md", "Variables/team/ops.md"],
        "matches": [
            {"pathIndex": 0, "subset": {"index": 0, "heading": ""}, "label": "Dev", "score": 0.7666666666666666, "matchRatio": 0.6666666666666666, "tagMatches": 1, "matchedVars": ["VK-host", "VK-port"], "extraVars": []},
            {"pathIndex": 2, "subset": {"index": 0, "heading": ""}, "label": "Ops", "score": 0.3, "matchRatio": 0.0, "tagMatches": 1, "matchedVars": [], "extraVars": ["VK-user"]},
            {"pathIndex": 1, "subset": {"index": 0, "heading": "Stage"}, "label": "Stage", "score": 0.2333333333333333, "matchRatio": 0.3333333333333333, "tagMatches": 0, "matchedVars": ["VK-host"], "extraVars": []},
            {"pathIndex": 1, "subset": {"index": 1, "heading": "Prod"}, "label": "Prod", "score": 0.2333333333333333, "matchRatio": 0.3333333333333333, "tagMatches": 0, "matchedVars": ["VK-host"], "extraVars": ["VK-db"]}
        ]
    });
    assert_eq!(lines[0], expect);
}

#[test]
fn apply_prints_overridden_with_was() {
    let v = vault("apply");
    let f = values_file(&v, r#"{"VK-host":"x"}"#);
    let o = go(
        &v,
        &["varsets", "apply", "Variables/dev.md", "--values", &f],
        b"",
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let t = out(&o);
    assert!(
        t.contains("overridden  VK-host = localhost  (was x)"),
        "{t}"
    );
    assert!(t.contains("filled      VK-port = 8080"), "{t}");
}

#[test]
fn apply_subset_without_heading_is_a_usage_error() {
    let v = vault("applyusage");
    let o = go(
        &v,
        &["varsets", "apply", "Variables/envs.md", "--subset", "0"],
        b"",
    );
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn save_prints_wrote_and_creates_the_file() {
    let v = vault("save");
    let f = values_file(&v, r#"{"VK-a":"1"}"#);
    let o = go(
        &v,
        &[
            "varsets",
            "save",
            "Local Dev",
            "--tag",
            "env",
            "--values",
            &f,
        ],
        b"",
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let t = out(&o);
    assert!(t.starts_with("wrote Variables/local-dev.md  "), "{t}");
    let body =
        std::fs::read_to_string(std::path::Path::new(&v).join("Variables/local-dev.md")).unwrap();
    assert!(body.contains("VK-a"), "{body}");
    assert!(body.contains("env"), "{body}");
}

#[test]
fn render_text_fills_from_stdin() {
    let v = vault("rt");
    let f = values_file(&v, r#"{"VK-a":"x"}"#);
    let o = run(
        &["render", "text", "-", "--values", &f],
        b"hi <VK-a></VK-a>",
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(out(&o).trim_end(), "hi x");
}

#[test]
fn render_text_refuses_escape() {
    let o = run(&["render", "text", "-"], b"\x1b[1m");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("render.contains_escape:"), "{}", err(&o));
    assert!(o.stdout.is_empty());
}

#[test]
fn render_text_bad_values_file_is_bad_request() {
    let v = vault("rtbad");
    let f = values_file(&v, "{");
    let o = run(&["render", "text", "-", "--values", &f], b"hi");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("op.bad_request:"), "{}", err(&o));
}

#[test]
fn render_text_both_stdin_is_bad_request() {
    let o = run(&["render", "text", "-", "--values", "-"], b"{}");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("op.bad_request:"), "{}", err(&o));
}

// Pin: a vault-authored ESC in a title never reaches the terminal.
#[test]
fn list_never_prints_raw_esc() {
    let v = vault("esc");
    std::fs::write(
        std::path::Path::new(&v).join("Variables/esc.md"),
        "---\nartifactType: Variables\ntitle: a\x1b[31mb\n---\n",
    )
    .unwrap();
    // The ESC survives parsing: the raw response carries it.
    let j = go(&v, &["--json", "vars", "list"], b"");
    let files = &json_lines(&j.stdout)[0]["files"];
    let esc = files
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "Variables/esc.md")
        .unwrap();
    assert_eq!(esc["title"], "a\u{1b}[31mb");
    let o = go(&v, &["vars", "list"], b"");
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let t = out(&o);
    assert!(!o.stdout.contains(&0x1b), "{t:?}");
    assert!(t.contains("a\\u{1b}[31mb"), "{t:?}");
}
