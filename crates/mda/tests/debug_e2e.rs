//! Debug mode through the real binary: stdout stays clean, the log goes where told, secrets stay out.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::{bin, json_lines, run};
use std::process::Stdio;

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"1.0"}}"#;
const OPS: &str = r#"{"jsonrpc":"2.0","id":2,"method":"system.ops"}"#;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn has_op_line(log: &str) -> bool {
    log.lines()
        .any(|l| l.contains(r#"op "system.ops""#) && l.contains("µs"))
}

#[test]
fn debug_does_not_change_stdout() {
    let plain = run(&["system", "ops", "--json"], b"");
    let dbg = run(&["system", "ops", "--json", "--debug"], b"");
    assert!(plain.status.success() && dbg.status.success());
    assert_eq!(plain.stdout, dbg.stdout);
    assert!(plain.stderr.is_empty());
    let err = String::from_utf8(dbg.stderr).unwrap();
    assert!(has_op_line(&err), "stderr: {err}");
}

#[test]
fn trace_serve_stdout_is_protocol_and_warning_is_first() {
    let input = format!("{INIT}\n{OPS}\n");
    let mut child = bin()
        .arg("serve")
        .env("MDA_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(json_lines(&out.stdout).len(), 2);
    let err = String::from_utf8(out.stderr).unwrap();
    let first = err.lines().next().unwrap();
    assert!(
        first.contains("WARN mda::debug trace output may include secrets"),
        "{first}"
    );
    assert!(has_op_line(&err), "stderr: {err}");
}

#[test]
fn log_file_gets_lines_and_stderr_is_empty() {
    let d = tmp("logfile");
    let f = d.join("x.log");
    let out = run(
        &[
            "system",
            "ops",
            "--json",
            "--debug",
            "--log-file",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert!(out.status.success());
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
    assert!(has_op_line(&std::fs::read_to_string(&f).unwrap()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn bad_log_file_dir_exits_3_naming_io_failed() {
    let d = tmp("badlog");
    let f = d.join("missing").join("x.log");
    let out = run(
        &[
            "system",
            "ops",
            "--debug",
            "--log-file",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .starts_with("io.failed: ")
    );
    assert!(!d.join("missing").exists());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn params_secret_never_logged_at_debug() {
    let out = run(
        &["call", "system.ops", r#"{"secret":"s3cr3t"}"#, "--debug"],
        b"",
    );
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("op.bad_request"), "stderr: {err}");
    assert!(!err.contains("s3cr3t"), "stderr: {err}");
    // The CLI's own error line may name the key; the debug log lines must not.
    let log: Vec<&str> = err.lines().filter(|l| l.contains(" DEBUG ")).collect();
    assert!(
        !log.is_empty() && log.iter().all(|l| !l.contains("secret")),
        "{log:?}"
    );
}

#[test]
fn debug_show_keeps_values_out_of_stderr() {
    let vault = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");
    let out = run(
        &[
            "--vault",
            vault,
            "--debug",
            "artifact",
            "show",
            "Snippets/hello.md",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("marker-7f3a")
    );
    assert!(
        !String::from_utf8(out.stderr)
            .unwrap()
            .contains("marker-7f3a")
    );
}
