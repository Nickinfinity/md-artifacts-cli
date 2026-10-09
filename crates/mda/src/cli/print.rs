//! Human rendering: the one English error table and per-op success printers.

use mda_ops::OpError;
use serde_json::Value;

/// English text for an error code; `None` for a code this table does not know.
///
/// # Examples
///
/// ```
/// assert!(mda::cli::print::english("op.unknown").is_some());
/// assert!(mda::cli::print::english("no.such").is_none());
/// ```
pub fn english(code: &str) -> Option<&'static str> {
    use mda_ops::error::*;
    Some(match code {
        OP_UNKNOWN => "no such operation",
        OP_BAD_REQUEST => "the request parameters are invalid",
        PROTOCOL_VERSION_MISMATCH => "client and engine protocol versions are incompatible",
        PATH_OUTSIDE_ROOT => "path is outside the allowed root",
        PATH_NOT_FOUND => "path does not exist",
        FILE_TOO_LARGE => "file exceeds the size limit",
        FILE_NOT_REGULAR => "not a regular file",
        IO_FAILED => "I/O failure",
        OP_INTERNAL => "internal engine error",
        _ => return None,
    })
}

/// `<code>: <English>` plus params. Values are Debug-escaped: they can echo client text, and a raw
/// ESC or newline must not reach the terminal.
pub fn error_line(e: &OpError) -> String {
    let mut s = format!("{}: {}", e.code, english(e.code).unwrap_or("unknown error"));
    for (k, v) in &e.params {
        s.push_str(&format!(" {k}={v:?}"));
    }
    s
}

/// Human text for a successful response of `op`; pretty JSON when no printer exists.
/// Today's printers print only the engine's static tables; the pretty-JSON fallback escapes
/// control characters. Future printers of vault-derived text must escape it (standing rule).
pub fn success(op: &str, v: &Value) -> String {
    match op {
        "system.version" => format!(
            "mda {}\nprotocol {}",
            v["engine"].as_str().unwrap_or("?"),
            v["protocol"].as_str().unwrap_or("?")
        ),
        "system.ops" => v["ops"]
            .as_array()
            .map(|ops| {
                ops.iter()
                    .map(|o| {
                        format!(
                            "{}  {}",
                            o["name"].as_str().unwrap_or("?"),
                            o["summary"].as_str().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        _ => serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}
