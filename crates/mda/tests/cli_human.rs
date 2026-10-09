//! Human (non-`--json`) CLI output.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::run;

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
