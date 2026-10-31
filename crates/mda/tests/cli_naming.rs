//! `mda naming check`: human output and exit codes (a refused name is an answer, exit 0).
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::run;

fn check(args: &[&str]) -> (Option<i32>, String) {
    let mut a = vec!["naming", "check", "--type", "Snippet"];
    a.extend_from_slice(args);
    let out = run(&a, b"");
    (out.status.code(), String::from_utf8(out.stdout).unwrap())
}

#[test]
fn derives_default_stem() {
    assert_eq!(
        check(&["--title", "Button Group"]),
        (Some(0), "button-group\n".into())
    );
}

#[test]
fn refusal_prints_code_and_exits_zero() {
    let (code, out) = check(&["--file-name", "CON"]);
    assert_eq!(code, Some(0));
    assert!(out.starts_with("CON: naming.reserved"), "{out}");
}

#[test]
fn json_is_the_response() {
    let (code, out) = check(&["--file-name", "a", "--json"]);
    assert_eq!(code, Some(0));
    assert_eq!(out.trim(), r#"{"fileName":"a","ok":true}"#);
}

#[test]
fn unknown_type_fails() {
    let out = run(&["naming", "check", "--type", "Nope"], b"");
    assert_ne!(out.status.code(), Some(0));
}

#[test]
fn echoed_file_name_is_escaped() {
    let (code, out) = check(&["--file-name", "a\u{1b}[31mb\u{85}c"]);
    assert_eq!(code, Some(0));
    assert!(
        !out.contains('\u{1b}') && !out.contains('\u{85}'),
        "{out:?}"
    );
    assert!(
        out.contains("a\\u{1b}[31mb\\u{85}c: naming.control_char"),
        "{out:?}"
    );
}
