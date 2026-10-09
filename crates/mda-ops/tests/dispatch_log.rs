#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: failure is the signal

use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};
use mda_ops::{Ctx, OpError, OpSpec, dispatch_in, typed};
use serde::Deserialize;
use serde_json::{Value, json};

static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
struct Cap;
impl Log for Cap {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, r: &Record) {
        if self.enabled(r.metadata()) {
            LINES
                .lock()
                .unwrap()
                .push(format!("{} {}", r.level(), r.args()));
        }
    }
    fn flush(&self) {}
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Req {
    #[allow(dead_code)] // read only via serde; the test cares about the log
    s: String,
}
fn op(_: &Ctx, _: Req) -> Result<Value, OpError> {
    Ok(json!("ok-resp"))
}
static LIST: &[OpSpec] = &[OpSpec {
    name: "t.log",
    summary: "",
    handler: |c, v| typed(c, v, op),
}];

// One test: the global logger can be installed once per process.
#[test]
fn debug_hides_params_trace_shows_them() {
    log::set_logger(&Cap).unwrap();
    log::set_max_level(LevelFilter::Debug);
    let ctx = Ctx::new(None);
    dispatch_in(LIST, &ctx, "t.log", json!({"s": "secret"})).unwrap();
    // A bad request: serde's reason may echo values, so it must not be logged at debug.
    dispatch_in(LIST, &ctx, "t.log", json!({"secret": 1})).unwrap_err();
    {
        let l = LINES.lock().unwrap();
        assert!(
            l.iter()
                .any(|s| s.contains("t.log") && s.contains("µs") && s.contains("ok"))
        );
        assert!(l.iter().any(|s| s.contains("op.bad_request")));
        assert!(l.iter().all(|s| !s.contains("secret")), "{l:?}");
        assert!(l.iter().all(|s| s.starts_with("DEBUG")), "{l:?}");
    }
    // A hostile op name must not forge lines or inject terminal escapes.
    LINES.lock().unwrap().clear();
    dispatch_in(LIST, &ctx, "x\ny\x1bz", json!({})).unwrap_err();
    {
        let l = LINES.lock().unwrap();
        assert!(!l.is_empty());
        assert!(
            l.iter().all(|s| !s.contains('\n') && !s.contains('\x1b')),
            "{l:?}"
        );
    }
    LINES.lock().unwrap().clear();
    log::set_max_level(LevelFilter::Trace);
    dispatch_in(LIST, &ctx, "t.log", json!({"s": "secret"})).unwrap();
    let l = LINES.lock().unwrap();
    assert!(
        l.iter()
            .any(|s| s.starts_with(&Level::Trace.to_string()) && s.contains("secret"))
    );
    assert!(l.iter().any(|s| s.contains("ok-resp")));
}
