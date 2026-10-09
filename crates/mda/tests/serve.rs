//! `serve` in-process: every test asserts every output line is JSON (stdout-purity sink) and the
//! expected line count.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: tests fail loudly

mod common;

use std::io::Cursor;

use mda::serve::{self, MAX_LINE_BYTES};
use mda_ops::Ctx;
use serde_json::{Value, json};

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"1.0"}}"#;
const OPS: &str = r#"{"jsonrpc":"2.0","id":7,"method":"system.ops"}"#;

fn session(input: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    serve::run(&Ctx::new(None), Cursor::new(input.to_vec()), &mut out).expect("run ok");
    common::json_lines(&out)
}

fn lines(l: &[&str]) -> Vec<Value> {
    session(
        l.iter()
            .map(|s| format!("{s}\n"))
            .collect::<String>()
            .as_bytes(),
    )
}

fn code(v: &Value) -> i64 {
    v["error"]["code"].as_i64().expect("error code")
}

#[test]
fn request_before_initialize_is_32002() {
    let out = lines(&[OPS]);
    assert_eq!(out.len(), 1);
    assert_eq!(code(&out[0]), -32002);
    assert_eq!(out[0]["id"], 7);
}

#[test]
fn initialize_answers_protocol_and_engine() {
    let out = lines(&[INIT]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["id"], 1);
    assert_eq!(out[0]["result"]["protocol"], "1.0");
    let engine = mda_ops::system::version(&Ctx::new(None), mda_ops::system::VersionRequest {})
        .unwrap()
        .engine;
    assert_eq!(out[0]["result"]["engine"], engine);
}

#[test]
fn other_major_is_mismatch() {
    let out = lines(&[r#"{"id":1,"method":"initialize","params":{"protocol":"2.0"}}"#]);
    assert_eq!(out.len(), 1);
    assert_eq!(code(&out[0]), -32000);
    assert_eq!(out[0]["error"]["data"]["code"], "protocol.version_mismatch");
    assert_eq!(out[0]["error"]["data"]["params"]["client"], "2.0");
    assert_eq!(out[0]["error"]["data"]["params"]["server"], "1.0");
}

#[test]
fn initialize_without_protocol_is_bad_request() {
    for p in [r#"{}"#, r#"{"protocol":5}"#] {
        let line = format!(r#"{{"id":1,"method":"initialize","params":{p}}}"#);
        let out = lines(&[&line]);
        assert_eq!(out.len(), 1);
        assert_eq!(code(&out[0]), -32000);
        assert_eq!(out[0]["error"]["data"]["code"], "op.bad_request");
    }
}

#[test]
fn op_after_initialize_keeps_id_and_order() {
    let out = lines(&[INIT, OPS, r#"{"id":"b","method":"system.version"}"#]);
    assert_eq!(out.len(), 3);
    assert_eq!(out[1]["id"], 7);
    assert!(out[1]["result"]["ops"].is_array());
    assert_eq!(out[2]["id"], "b");
    assert_eq!(out[2]["result"]["protocol"], "1.0");
}

#[test]
fn unknown_method_is_32601() {
    let out = lines(&[INIT, r#"{"id":2,"method":"nope"}"#]);
    assert_eq!(out.len(), 2);
    assert_eq!(code(&out[1]), -32601);
    assert_eq!(out[1]["error"]["data"]["code"], "op.unknown");
}

#[test]
fn non_json_is_32700_and_next_request_answered() {
    let out = lines(&["not json", INIT]);
    assert_eq!(out.len(), 2);
    assert_eq!(code(&out[0]), -32700);
    assert_eq!(out[0]["id"], Value::Null);
    assert_eq!(out[1]["result"]["protocol"], "1.0");
}

#[test]
fn over_long_line_is_32600_and_next_request_answered() {
    let mut input = vec![b'x'; MAX_LINE_BYTES + 1];
    input.push(b'\n');
    input.extend_from_slice(INIT.as_bytes());
    input.push(b'\n');
    let out = session(&input);
    assert_eq!(out.len(), 2);
    assert_eq!(code(&out[0]), -32600);
    assert_eq!(out[1]["result"]["protocol"], "1.0");
}

#[test]
fn line_of_exactly_the_cap_is_accepted() {
    // valid JSON padded to the cap: parsed (id-less, so no reply) rather than rejected
    let pad = MAX_LINE_BYTES - r#"{"method":"x","p":""}"#.len();
    let mut input = format!(r#"{{"method":"x","p":"{}"}}"#, "a".repeat(pad)).into_bytes();
    assert_eq!(input.len(), MAX_LINE_BYTES);
    input.push(b'\n');
    assert!(session(&input).is_empty());
}

#[test]
fn second_initialize_is_32600_and_next_request_answered() {
    let out = lines(&[INIT, INIT, OPS]);
    assert_eq!(out.len(), 3);
    assert_eq!(code(&out[1]), -32600);
    assert_eq!(out[2]["id"], 7);
    assert!(out[2]["result"].is_object());
}

#[test]
fn message_without_id_gets_no_response() {
    let out = lines(&[INIT, r#"{"method":"system.ops"}"#, r#"{"method":"nope"}"#]);
    assert_eq!(out.len(), 1);
}

#[test]
fn crlf_and_blank_lines_and_eof() {
    let input = format!("\n\r\n{INIT}\r\n{OPS}"); // last line has no newline
    let out = session(input.as_bytes());
    assert_eq!(out.len(), 2);
    assert_eq!(out[1]["id"], 7);
    assert!(session(b"").is_empty());
}

#[test]
fn non_object_is_32600() {
    let out = lines(&["[1]", r#"{"id":3,"method":5}"#]);
    assert_eq!(out.len(), 2);
    assert_eq!(code(&out[0]), -32600);
    assert_eq!(out[0]["id"], Value::Null);
    assert_eq!(code(&out[1]), -32600);
    assert_eq!(out[1]["id"], json!(3));
}
