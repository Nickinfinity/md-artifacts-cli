//! One `serve` session that selects a vault and uses it: the session switch over the wire.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;

use serde_json::json;

#[test]
fn select_then_tree_then_config_get() {
    let base = std::env::temp_dir().join(format!("mda-sv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (vault, cfg) = (base.join("v"), base.join("cfg"));
    std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
    let vault = vault.canonicalize().unwrap();
    std::fs::create_dir_all(&cfg).unwrap();
    let cfg = cfg.canonicalize().unwrap();
    let req = |id: u32, m: &str, p: serde_json::Value| {
        json!({"jsonrpc":"2.0","id":id,"method":m,"params":p}).to_string()
    };
    let lines = [
        req(1, "initialize", json!({"protocol": "1.0"})),
        req(2, "vault.select", json!({"path": vault})),
        req(3, "artifact.tree", json!({"type": "Snippet"})),
        req(4, "config.get", json!({})),
    ];
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let input: String = refs.iter().map(|l| format!("{l}\n")).collect();
    let out = common::run_cfg(&["serve"], input.as_bytes(), &cfg);
    let got = common::json_lines(&out.stdout); // every stdout line is JSON
    assert_eq!(got.len(), 4);
    assert_eq!(got[1]["result"]["path"], json!(vault));
    assert_eq!(got[2]["result"]["dirs"], json!([]));
    assert_eq!(got[2]["result"]["files"], json!([]));
    assert_eq!(got[3]["result"]["source"], "default");
    assert!(cfg.join("config.toml").is_file());
}
