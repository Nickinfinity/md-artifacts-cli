//! `mda naming output`: human output and exit codes.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::run;

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");

fn out(args: &[&str]) -> (Option<i32>, String, String) {
    let mut a = vec!["--vault", VAULT, "naming", "output"];
    a.extend_from_slice(args);
    let o = run(&a, b"");
    (
        o.status.code(),
        String::from_utf8(o.stdout).unwrap(),
        String::from_utf8(o.stderr).unwrap(),
    )
}

#[test]
fn prints_the_name() {
    assert_eq!(
        out(&["Templates/component.md"]),
        (Some(0), "Button.tsx\n".into(), String::new())
    );
}

#[test]
fn typed_name_wins() {
    let (code, o, _) = out(&["Templates/component.md", "--file-name", "Btn"]);
    assert_eq!((code, o.as_str()), (Some(0), "Btn.tsx\n"));
}

#[test]
fn refusal_is_english_and_nonzero() {
    let (code, o, e) = out(&["Templates/component.md", "--file-name", "../x"]);
    assert_ne!(code, Some(0));
    assert!(o.is_empty(), "{o}");
    assert!(
        e.contains("naming.path_injection: the file name contains a path separator"),
        "{e}"
    );
    assert!(e.contains("fileName"), "{e}");
}

#[test]
fn json_is_the_response() {
    let (_, o, _) = out(&["Templates/component.md", "--json"]);
    assert_eq!(o.trim(), r#"{"fileName":"Button.tsx"}"#);
}
