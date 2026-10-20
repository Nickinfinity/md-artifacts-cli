//! `mda serve`: JSON-RPC 2.0 over stdio, one JSON object per line. stdout is the protocol, so
//! everything is written through the single `out` writer and flushed per message; frames are
//! logged at `trace` only (they carry params, which may be secrets).

mod framing;

pub use framing::MAX_LINE_BYTES;

use std::io::{BufRead, Write};

use framing::{Line, read_line};
use mda_ops::error::{
    OP_BAD_REQUEST, OP_INTERNAL, OP_UNKNOWN, PROTOCOL_ALREADY_INITIALIZED,
    PROTOCOL_INVALID_REQUEST, PROTOCOL_LINE_TOO_LONG, PROTOCOL_NOT_INITIALIZED,
    PROTOCOL_PARSE_ERROR, PROTOCOL_VERSION_MISMATCH,
};
use mda_ops::system::{VersionRequest, version};
use mda_ops::{Ctx, OpError, PROTOCOL};
use serde_json::{Value, json};

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const OP_ERROR: i64 = -32000;
const NOT_INITIALIZED: i64 = -32002;

/// Serve requests read from `input`, writing one response line per request to `out`, until EOF.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// let mut out = Vec::new();
/// let input = b"{\"id\":1,\"method\":\"initialize\",\"params\":{\"protocol\":\"1.0\"}}\n";
/// mda::serve::run(&ctx, &input[..], &mut out).unwrap();
/// assert!(String::from_utf8(out).unwrap().contains("\"protocol\":\"1.1\""));
/// ```
pub fn run(ctx: &Ctx, mut input: impl BufRead, mut out: impl Write) -> std::io::Result<()> {
    let mut initialized = false;
    loop {
        let reply = match read_line(&mut input)? {
            Line::Eof => return Ok(()),
            Line::TooLong => Some(transport(
                &Value::Null,
                INVALID_REQUEST,
                PROTOCOL_LINE_TOO_LONG,
            )),
            Line::Text(t) if t.is_empty() => None,
            Line::Text(t) => {
                log::trace!("serve in: {}", String::from_utf8_lossy(&t));
                handle(ctx, &t, &mut initialized)
            }
        };
        if let Some(r) = reply {
            let line = r.to_string();
            log::trace!("serve out: {line}");
            writeln!(out, "{line}")?;
            out.flush()?;
        }
    }
}

/// One frame to at most one response. `None` for client notifications (no `id`, string `method`).
fn handle(ctx: &Ctx, line: &[u8], initialized: &mut bool) -> Option<Value> {
    let Ok(msg) = serde_json::from_slice::<Value>(line) else {
        return Some(transport(&Value::Null, PARSE_ERROR, PROTOCOL_PARSE_ERROR));
    };
    let invalid = |id: &Value| Some(transport(id, INVALID_REQUEST, PROTOCOL_INVALID_REQUEST));
    let Value::Object(mut obj) = msg else {
        return invalid(&Value::Null);
    };
    let method = obj.remove("method");
    let id = match obj.remove("id") {
        None if matches!(method, Some(Value::String(_))) => return None,
        None => return invalid(&Value::Null),
        // An invalid id is never echoed: a hostile one could be megabytes.
        Some(id) if !valid_id(&id) => return invalid(&Value::Null),
        Some(id) => id,
    };
    let id = &id;
    let Some(Value::String(method)) = method else {
        return invalid(id);
    };
    // Moved, not cloned: a hostile params tree is already large.
    let params = obj.remove("params").unwrap_or(Value::Null);
    let method = method.as_str();
    if method == "initialize" {
        if *initialized {
            return Some(transport(id, INVALID_REQUEST, PROTOCOL_ALREADY_INITIALIZED));
        }
        let result = initialize(ctx, &params);
        *initialized = result.is_ok();
        return Some(respond(id, result));
    }
    if !*initialized {
        return Some(transport(id, NOT_INITIALIZED, PROTOCOL_NOT_INITIALIZED));
    }
    Some(respond(id, mda_ops::dispatch(ctx, method, params)))
}

/// A JSON string, or an integer in i64/u64 range: anything else would not echo byte-exact.
fn valid_id(v: &Value) -> bool {
    match v {
        Value::String(_) => true,
        Value::Number(n) => n.is_i64() || n.is_u64(),
        _ => false,
    }
}

fn initialize(ctx: &Ctx, params: &Value) -> Result<Value, OpError> {
    let client = params
        .get("protocol")
        .and_then(Value::as_str)
        .ok_or_else(|| OpError::new(OP_BAD_REQUEST))?;
    let major = |v: &str| v.split('.').next().map(str::to_owned);
    if major(client) != major(PROTOCOL) {
        return Err(OpError::new(PROTOCOL_VERSION_MISMATCH)
            .with("client", client)
            .with("server", PROTOCOL));
    }
    let engine = version(ctx, VersionRequest {})?.engine;
    let types = serde_json::to_value(&mda_ops::TYPES).map_err(|_| OpError::new(OP_INTERNAL))?;
    Ok(json!({ "protocol": PROTOCOL, "engine": engine, "types": types }))
}

fn respond(id: &Value, result: Result<Value, OpError>) -> Value {
    match result {
        Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
        Err(e) => {
            let rpc = if e.code == OP_UNKNOWN {
                METHOD_NOT_FOUND
            } else {
                OP_ERROR
            };
            rpc_error(id, rpc, &e)
        }
    }
}

/// A transport-level error (`protocol.*`) with no params, except `line_too_long{max}`.
fn transport(id: &Value, rpc: i64, code: &'static str) -> Value {
    let e = if code == PROTOCOL_LINE_TOO_LONG {
        OpError::new(code).with("max", MAX_LINE_BYTES.to_string())
    } else {
        OpError::new(code)
    };
    rpc_error(id, rpc, &e)
}

/// The one error shape: `message` = `data.code`, `data` = the serialized `OpError`.
fn rpc_error(id: &Value, rpc: i64, e: &OpError) -> Value {
    // OpError serializes infallibly (string map); fall back to the bare code regardless.
    let data = serde_json::to_value(e).unwrap_or_else(|_| json!({ "code": OP_INTERNAL }));
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": rpc, "message": e.code, "data": data } })
}
