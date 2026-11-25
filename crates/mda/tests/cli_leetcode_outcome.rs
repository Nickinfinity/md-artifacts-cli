//! `leetcode.record_outcome` hostile sinks that need a temp vault: values the parser reads back
//! differently (`}`, U+2028), a config fence redeclaring `status`, CRLF. Every refusal asserts the
//! file's bytes are unchanged.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)] // reason: tests

mod common;
use common::run_cfg;
use serde_json::{Value, json};

const WRITE: &str = include_str!("fixtures/vault_leetcode/LeetCode/write.md");

struct Env {
    v: std::path::PathBuf,
    cfg: std::path::PathBuf,
}

impl Env {
    fn new(name: &str, body: &str) -> Self {
        let b = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("mda-lc-out-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&b);
        let v = b.join("v");
        std::fs::create_dir_all(v.join(".obsidian")).unwrap();
        std::fs::create_dir_all(v.join("LeetCode")).unwrap();
        std::fs::write(v.join("LeetCode/w.md"), body).unwrap();
        Env {
            v,
            cfg: b.join("c"),
        }
    }
    fn call(&self, op: &str, params: &Value) -> Value {
        let p = params.to_string();
        let a = [
            "--vault",
            self.v.to_str().unwrap(),
            "call",
            op,
            &p,
            "--json",
        ];
        let out = run_cfg(&a, b"", &self.cfg);
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {:?}", out))
    }
    fn hash(&self) -> String {
        let r = self.call("leetcode.read", &json!({"path": "LeetCode/w.md"}));
        r["hash"].as_str().unwrap().to_owned()
    }
    fn outcome(&self, mut p: Value) -> Value {
        p["path"] = "LeetCode/w.md".into();
        p["expectedHash"] = self.hash().into();
        p["language"] = "python".into();
        self.call("leetcode.record_outcome", &p)
    }
    fn bytes(&self) -> String {
        std::fs::read_to_string(self.v.join("LeetCode/w.md")).unwrap()
    }
}

fn refused(e: &Env, p: Value, reason: &str) {
    let before = e.bytes();
    let r = e.outcome(p);
    assert_eq!(r["code"], "leetcode.not_serializable", "{r}");
    assert_eq!(r["params"]["reason"], reason, "{r}");
    assert_eq!(e.bytes(), before, "sink: bytes unchanged");
}

/// T6.1 review (a)/(b): the lazy meta regex backtracks to the `}` before `-->`, and the parser's
/// line splitting keeps U+2028/U+2029 inside the comment, so these values round-trip (the guard
/// compared the re-parse with the request and accepted). The test pins read-back equality.
#[test]
fn brace_and_line_separators_in_meta_round_trip() {
    for dur in ["1m}x", "1m}", "{1m}", "1m\u{2028}x", "1m\u{2029}"] {
        let e = Env::new("meta-rt", WRITE);
        let s = json!({"solvedAt": "a}b\u{2028}", "duration": dur});
        let r = e.outcome(json!({"status": "solved", "solved": s}));
        assert!(r["hash"].is_string(), "{dur:?}: {r}");
        let x = e.call("leetcode.read", &json!({"path": "LeetCode/w.md"}));
        assert_eq!(x["exercise"]["solutions"][0]["duration"], dur);
        assert_eq!(x["exercise"]["solutions"][0]["solvedAt"], "a}b\u{2028}");
    }
}

#[test]
fn brace_and_line_separators_in_attempt_at_round_trip() {
    for at in ["t}x", "t\u{2028}x", "t\u{2029}", "}"] {
        let e = Env::new("at-rt", WRITE);
        let a = json!({"at": at, "duration": "1s", "passed": true, "code": "x"});
        let r = e.outcome(json!({"status": "attempted", "attempt": a}));
        assert!(r["hash"].is_string(), "{at:?}: {r}");
        let x = e.call("leetcode.read", &json!({"path": "LeetCode/w.md"}));
        assert_eq!(x["exercise"]["attempts"][0]["at"], at);
    }
}

#[test]
fn arrow_in_meta_fields_is_refused() {
    let e = Env::new("arrow", WRITE);
    let s = json!({"solvedAt": "t", "duration": "a --> b"});
    refused(&e, json!({"status": "solved", "solved": s}), "meta");
}

#[test]
fn status_redeclared_in_a_config_fence_is_refused() {
    let src = WRITE.replacen("returns: bool\n", "returns: bool\nstatus: unsolved\n", 1);
    assert_ne!(src, WRITE);
    let e = Env::new("status", &src);
    refused(&e, json!({"status": "solved"}), "status");
}

#[test]
fn stale_hash_beats_the_no_op() {
    let e = Env::new("stale-noop", WRITE);
    let before = e.bytes();
    let p = json!({"path": "LeetCode/w.md", "expectedHash": "stale", "status": "attempted", "language": "python"});
    let r = e.call("leetcode.record_outcome", &p);
    assert_eq!(r["code"], "file.conflict", "{r}");
    assert_eq!(e.bytes(), before);
}

#[test]
fn crlf_file_gets_crlf_inserts() {
    let crlf = WRITE.replace('\n', "\r\n");
    let e = Env::new("crlf", &crlf);
    let s = json!({"solvedAt": "2026-11-05T10:00:00Z", "duration": "6m03s"});
    let r = e.outcome(json!({"status": "solved", "solved": s}));
    assert!(r["hash"].is_string(), "{r}");
    let out = e.bytes();
    assert!(out.contains("status: solved\r\n"));
    assert!(out.contains("-->\r\n```python"));
    assert!(!out.replace("\r\n", "").contains('\n'), "no bare LF");
}
