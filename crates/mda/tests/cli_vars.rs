//! `mda vars …` through the real binary: requests, human output, exit codes, terminal safety.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{copy_tree, json_lines, run};

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");
const FLAT: &str = "a56e89a491ff92348440f538849cd298827e12c81fda3f88b8e4e22f0c273bdd";
const SETS: &str = "c947f0a5b355398e9f282a7d39c6c6da2b63c4c0f16d24b1d3ecbe6447fbfe4e";
const HEAD: &str = "---\nartifactType: Variables\ntitle: Flat\n---\n\n```vks\n";

fn vault(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("mda-cli-vars-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(std::path::Path::new(FIX), &d);
    d.to_str().unwrap().to_owned()
}

fn go(args: &[&str], v: &str) -> std::process::Output {
    let mut a = vec!["--vault", v];
    a.extend_from_slice(args);
    run(&a, b"")
}

fn file(v: &str, p: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(v).join(p)).unwrap()
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn err(o: &std::process::Output) -> String {
    String::from_utf8(o.stderr.clone()).unwrap()
}

#[test]
fn var_set_value_prints_wrote_and_writes_yaml() {
    let v = vault("set");
    let o = go(
        &[
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--value",
            "x",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(
        out(&o).starts_with("wrote Variables/flat.md  "),
        "{}",
        out(&o)
    );
    assert_eq!(
        file(&v, "Variables/flat.md"),
        format!("{HEAD}VK-host: x\nVK-port: '3000'\n```\n")
    );
}

#[test]
fn value_flag_is_always_a_string() {
    let v = vault("numstr");
    let o = go(
        &[
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--value",
            "1",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(file(&v, "Variables/flat.md").contains("VK-host: '1'\n"));
}

#[test]
fn json_value_list_and_malformed() {
    let v = vault("jv");
    let o = go(
        &[
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--json-value",
            r#"["dev","prod"]"#,
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        file(&v, "Variables/flat.md"),
        format!("{HEAD}VK-host:\n  - dev\n  - prod\nVK-port: '3000'\n```\n")
    );
    let v = vault("jvbad");
    let o = go(
        &[
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--json-value",
            "{",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).starts_with("op.bad_request:"), "{}", err(&o));
    assert!(file(&v, "Variables/flat.md").contains("VK-host: localhost"));
}

#[test]
fn clap_refusals_exit_2() {
    let v = vault("clap");
    let cases: [&[&str]; 3] = [
        &[
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--hash",
            FLAT,
        ],
        &[
            "vars",
            "var",
            "add",
            "Variables/flat.md",
            "VK-n",
            "--value",
            "a",
            "--json-value",
            "\"b\"",
            "--hash",
            FLAT,
        ],
        &[
            "vars",
            "var",
            "delete",
            "Variables/sets.md",
            "VK-host",
            "--subset",
            "0",
            "--hash",
            SETS,
        ],
    ];
    for c in cases {
        assert_eq!(go(c, &v).status.code(), Some(2), "{c:?}");
    }
}

#[test]
fn subset_rename_and_describe() {
    let v = vault("subset");
    let o = go(
        &[
            "vars",
            "subset",
            "rename",
            "Variables/sets.md",
            "Live",
            "--subset",
            "1",
            "--heading",
            "Prod",
            "--hash",
            SETS,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(file(&v, "Variables/sets.md").contains("## Live\n"));
    let v = vault("describe");
    let o = go(
        &[
            "vars",
            "subset",
            "describe",
            "Variables/sets.md",
            "",
            "--subset",
            "0",
            "--heading",
            "Dev",
            "--hash",
            SETS,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(!file(&v, "Variables/sets.md").contains("Dev machine."));
}

#[test]
fn file_new_and_tags_clear() {
    let v = vault("new");
    let o = go(&["vars", "file", "new", "Local Dev", "--tag", "env"], &v);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(
        out(&o).starts_with("wrote Variables/local-dev.md  "),
        "{}",
        out(&o)
    );
    assert_eq!(
        file(&v, "Variables/local-dev.md"),
        "---\nartifactType: Variables\ntitle: Local Dev\ntags: [env]\n---\n"
    );
    let o = go(
        &["vars", "meta", "tags", "Variables/sets.md", "--hash", SETS],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(!file(&v, "Variables/sets.md").contains("tags:"));
}

#[test]
fn json_flag_prints_the_response_line() {
    let v = vault("json");
    let o = go(
        &[
            "--json",
            "vars",
            "var",
            "add",
            "Variables/flat.md",
            "VK-new",
            "--value",
            "x",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let lines = json_lines(&o.stdout);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        serde_json::json!({"path": "Variables/flat.md", "hash": "0ea28deeaf6a8ead61e87931d145172a6446762abd6370f856e8ca9029356be6"})
    );
}

#[test]
fn bad_name_is_coded_and_name_is_sent_verbatim() {
    let v = vault("badname");
    let o = go(
        &[
            "vars",
            "var",
            "add",
            "Variables/flat.md",
            "host",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).starts_with("vars.bad_name: "), "{}", err(&o));
    assert!(err(&o).contains(r#"name="host""#), "{}", err(&o));
    assert!(file(&v, "Variables/flat.md").contains("VK-host: localhost"));
}

#[test]
fn esc_in_refused_name_never_reaches_stderr_raw() {
    let v = vault("esc");
    let o = go(
        &[
            "vars",
            "var",
            "add",
            "Variables/flat.md",
            "VK-\x1b[31m",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(!o.stderr.contains(&0x1b), "{:?}", o.stderr);
    assert!(err(&o).contains(r"\u{1b}"), "{}", err(&o));
}

#[test]
fn json_value_number_is_refused_and_file_unchanged() {
    let v = vault("jvnum");
    let before = file(&v, "Variables/flat.md");
    let o = go(
        &[
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--json-value",
            "1",
            "--hash",
            FLAT,
        ],
        &v,
    );
    assert_eq!(o.status.code(), Some(1), "{}", err(&o));
    assert!(err(&o).starts_with("op.bad_request:"), "{}", err(&o));
    assert_eq!(file(&v, "Variables/flat.md"), before);
    assert!(before.contains("VK-host: localhost"));
}
