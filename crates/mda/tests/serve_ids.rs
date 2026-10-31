//! `serve` request ids, id-less messages and the 🔒 state sinks (init gate, writes on disk).
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: tests fail loudly

mod common;

use std::io::Cursor;
use std::path::{Path, PathBuf};

use mda::serve;
use mda_ops::{Ctx, Root, dispatch};
use serde_json::{Value, json};

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"2.0"}}"#;
const OPS: &str = r#"{"jsonrpc":"2.0","id":7,"method":"system.ops"}"#;
const HELLO: &str = "Snippets/hello.md";

fn run_in(ctx: &Ctx, l: &[&str]) -> Vec<u8> {
    let input: String = l.iter().map(|s| format!("{s}\n")).collect();
    let mut out = Vec::new();
    serve::run(ctx, Cursor::new(input.into_bytes()), &mut out).expect("run ok");
    out
}

fn lines(l: &[&str]) -> Vec<Value> {
    common::json_lines(&run_in(&Ctx::new(None), l))
}

fn assert_invalid(v: &Value) {
    assert_eq!(v["id"], Value::Null, "{v}");
    assert!(v.get("result").is_none(), "{v}");
    assert_eq!(v["error"]["code"], -32600, "{v}");
    assert_eq!(
        v["error"]["data"]["code"], "protocol.invalid_request",
        "{v}"
    );
}

fn copy_vault(tag: &str) -> (PathBuf, Ctx, String) {
    let dir = std::env::temp_dir().join(format!("mda-serveids-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault");
    common::copy_tree(&src, &dir);
    let ctx = Ctx::new(Some(Root::new(&dir).unwrap()));
    let hash = dispatch(&ctx, "artifact.read", json!({"path": HELLO})).unwrap()["hash"]
        .as_str()
        .unwrap()
        .to_owned();
    (dir, ctx, hash)
}

#[test]
fn bad_ids_are_invalid_request() {
    for id in ["null", "{}", "[1]", "true"] {
        let line = format!(r#"{{"id":{id},"method":"system.version"}}"#);
        let out = lines(&[INIT, &line]);
        assert_eq!(out.len(), 2);
        assert_invalid(&out[1]);
    }
    let out = lines(&[
        r#"{"id":{},"method":"system.ops"}"#,
        r#"{"id":{},"method":5}"#,
    ]);
    assert_eq!(out.len(), 2);
    assert_invalid(&out[0]);
    assert_invalid(&out[1]);
    // (a) invalid-id initialize does not initialize
    let out = lines(&[
        r#"{"id":{},"method":"initialize","params":{"protocol":"2.0"}}"#,
        OPS,
    ]);
    assert_invalid(&out[0]);
    assert_eq!(out[1]["error"]["code"], -32002);
    assert_eq!(out[1]["error"]["data"]["code"], "protocol.not_initialized");
}

#[test]
fn null_id_write_leaves_file() {
    let (dir, ctx, hash) = copy_vault("nullid");
    let del =
        json!({"id":null,"method":"artifact.delete","params":{"path":HELLO,"expectedHash":hash}})
            .to_string();
    let out = common::json_lines(&run_in(&ctx, &[INIT, &del]));
    assert_eq!(out.len(), 2);
    assert_invalid(&out[1]);
    assert!(dir.join(HELLO).exists(), "null-id request must not execute");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn good_ids_echo() {
    for raw in [
        "-1",
        "0",
        "-9223372036854775808",
        "9223372036854775807",
        "18446744073709551615",
        r#""""#,
        r#""b""#,
    ] {
        let line = format!(r#"{{"id":{raw},"method":"system.version"}}"#);
        let out = String::from_utf8(run_in(&Ctx::new(None), &[INIT, &line])).unwrap();
        let reply = out.lines().nth(1).unwrap();
        assert!(
            reply.contains(&format!(r#""id":{raw},"#))
                || reply.contains(&format!(r#""id":{raw}}}"#)),
            "{raw} not echoed byte-exact: {reply}"
        );
    }
    for raw in [
        "1.5",
        "1e3",
        "1.0",
        "-0",
        "-9223372036854775809",
        "18446744073709551616",
    ] {
        let line = format!(r#"{{"id":{raw},"method":"system.version"}}"#);
        let out = lines(&[INIT, &line]);
        assert_invalid(&out[1]);
    }
}

#[test]
fn invalid_id_not_echoed() {
    let big = "a".repeat(1 << 20);
    let line = format!(r#"{{"id":{{"k":"{big}"}},"method":"system.version"}}"#);
    let out = run_in(&Ctx::new(None), &[INIT, &line]);
    let reply = String::from_utf8(out)
        .unwrap()
        .lines()
        .nth(1)
        .unwrap()
        .to_owned();
    assert!(reply.len() < 256, "reply {} bytes", reply.len());
    assert_invalid(&serde_json::from_str(&reply).unwrap());
}

#[test]
fn id_less() {
    let out = lines(&["{}", r#"{"method":5}"#]);
    assert_eq!(out.len(), 2);
    out.iter().for_each(assert_invalid);
    // notifications: no line; the initialize notification does not initialize
    let out = lines(&[INIT, r#"{"method":"system.ops"}"#, OPS]);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1]["id"], 7);
    let out = lines(&[
        r#"{"method":"initialize","params":{"protocol":"2.0"}}"#,
        OPS,
    ]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["error"]["data"]["code"], "protocol.not_initialized");
}

#[test]
fn array_params_refused_over_wire() {
    let out = lines(&[
        INIT,
        r#"{"id":4,"method":"artifact.read","params":["Snippets/hello.md"]}"#,
    ]);
    assert_eq!(out[1]["error"]["code"], -32000);
    assert_eq!(out[1]["error"]["data"]["code"], "op.bad_request");
    let out = lines(&[r#"{"id":1,"method":"initialize","params":["1.0"]}"#, OPS]);
    assert_eq!(out[0]["error"]["code"], -32000);
    assert_eq!(out[0]["error"]["data"]["code"], "op.bad_request");
    assert!(out[0]["error"]["data"]["params"].get("reason").is_none());
    assert_eq!(out[1]["error"]["data"]["code"], "protocol.not_initialized");
}

#[test]
fn array_params_delete_leaves_file() {
    let (dir, ctx, hash) = copy_vault("arrdel");
    let del = json!({"id":5,"method":"artifact.delete","params":[HELLO, hash]}).to_string();
    let out = common::json_lines(&run_in(&ctx, &[INIT, &del]));
    assert_eq!(out[1]["error"]["data"]["code"], "op.bad_request");
    assert!(dir.join(HELLO).exists(), "array params must not delete");
    std::fs::remove_dir_all(&dir).unwrap();
}
