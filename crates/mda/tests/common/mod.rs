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

/// Run `mda <args> serve` with `lines` as NDJSON input (each followed by `\n`), then EOF.
pub fn serve_session(args: &[&str], lines: &[&str]) -> Output {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let mut all = args.to_vec();
    all.push("serve");
    run(&all, input.as_bytes())
}

/// Copy the directory tree `src` to `dst` (created): regular files and directories only, std only.
/// Write op cases run each leg on a fresh copy so no leg sees another's writes (W-13).
pub fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("create copy dir");
    for e in std::fs::read_dir(src).expect("read fixture dir") {
        let e = e.expect("dir entry");
        let ty = e.file_type().expect("file type");
        let to = dst.join(e.file_name());
        if ty.is_dir() {
            copy_tree(&e.path(), &to);
        } else if ty.is_file() {
            std::fs::copy(e.path(), &to).expect("copy fixture file");
        }
    }
}
