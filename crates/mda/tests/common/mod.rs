//! Shared helpers for the `mda` integration tests. Orchestrator-owned: import with `mod common;`,
//! never edit from a task.
#![allow(dead_code)] // reason: each test binary uses a subset
#![allow(clippy::panic, clippy::expect_used)] // reason: test helpers fail the test loudly

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// The built `mda` binary.
pub fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mda"))
}

/// Run `mda <args>` with `stdin` piped in; wait for it to exit.
pub fn run(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = bin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mda");
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(stdin)
        .expect("write stdin");
    child.wait_with_output().expect("wait for mda")
}

/// Every stdout line parsed as JSON. Panics naming the first line that is not JSON: the
/// stdout-purity sink for `serve` and `--json`.
pub fn json_lines(out: &[u8]) -> Vec<serde_json::Value> {
    let text = std::str::from_utf8(out).expect("stdout is UTF-8");
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("stdout line {} is not JSON ({e}): {line:?}", i + 1))
        })
        .collect()
}

/// Run `mda serve` with `lines` as NDJSON input (each followed by `\n`), then EOF.
pub fn serve_session(lines: &[&str]) -> Output {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    run(&["serve"], input.as_bytes())
}
