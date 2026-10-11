//! `index.plan` / `index.run` against temp vaults: linked reads stay in the index's own directory,
//! limits fire before work, errors carry no machine path, duplicate links plan identically.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::PathBuf;

use mda_ops::index::{BatchResponse, PlanResponse, StepResult, plan, run};
use mda_ops::{Ctx, OpError, Root};
use serde_json::json;

const AGENT: &str =
    "---\nartifactType: AIAgentsConfig\ntitle: Reviewer\ntarget: CLAUDE.md\n---\n\nhi\n";
const TPL: &str = "---\nartifactType: Template\ntitle: A\nextension: ts\n---\n\n```ts\nx\n```\n";

struct Env {
    vault: PathBuf,
    ws: PathBuf,
    ctx: Ctx,
}

fn tmp(name: &str, kind: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-ix-{}-{name}-{kind}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn env(name: &str, index_body: &str) -> Env {
    env_paths(name, index_body, "")
}

fn env_paths(name: &str, index_body: &str, paths_line: &str) -> Env {
    let vault = tmp(name, "vault");
    fs::create_dir_all(vault.join("Templates")).unwrap();
    fs::create_dir_all(vault.join("AIAgentsConf")).unwrap();
    fs::write(vault.join("AIAgentsConf/reviewer.md"), AGENT).unwrap();
    fs::write(vault.join("Templates/a.md"), TPL).unwrap();
    let idx = format!(
        "---\nartifactType: Template\ntitle: I\nindex: true\n{paths_line}---\n\n{index_body}"
    );
    fs::write(vault.join("Templates/index.md"), idx).unwrap();
    let ctx = Ctx::new(Some(Root::new(&vault).unwrap()));
    Env {
        vault,
        ws: tmp(name, "ws"),
        ctx,
    }
}

impl Env {
    fn plan(&self) -> Result<PlanResponse, OpError> {
        plan(
            &self.ctx,
            serde_json::from_value(json!({"path": "Templates/index.md"})).unwrap(),
        )
    }
    fn run(&self, steps: serde_json::Value) -> Result<BatchResponse, OpError> {
        let v = json!({"path": "Templates/index.md", "workspaceRoot": self.ws.display().to_string(), "steps": steps});
        run(&self.ctx, serde_json::from_value(v).unwrap())
    }
    fn assert_no_abs(&self, params: &impl std::fmt::Debug) {
        let text = format!("{params:?}");
        for r in [&self.vault, &self.ws] {
            assert!(!text.contains(&r.display().to_string()), "{text}");
        }
    }
    fn ws_empty(&self) {
        assert_eq!(fs::read_dir(&self.ws).unwrap().count(), 0);
    }
}

#[cfg(unix)]
fn escape_link(e: &Env) {
    std::os::unix::fs::symlink(e.vault.join("AIAgentsConf"), e.vault.join("Templates/out"))
        .unwrap();
}

#[cfg(unix)]
#[test]
fn run_symlink_out_of_index_dir_is_skipped_and_sink_empty() {
    let e = env("runlink", "[[out/reviewer]]");
    escape_link(&e);
    let b = e
        .run(json!([{"relPath": "out/reviewer.md", "destDir": ""}]))
        .unwrap();
    match &b.results[0] {
        StepResult::Skipped {
            rel_path,
            code,
            params,
        } => {
            assert_eq!(
                (rel_path.as_str(), *code),
                ("out/reviewer.md", "path.outside_root")
            );
            assert_eq!(params["path"], "out/reviewer.md");
            e.assert_no_abs(params);
        }
        r => panic!("{r:?}"),
    }
    e.ws_empty();
}

#[cfg(unix)]
#[test]
fn plan_symlink_out_of_index_dir_is_skip_outside_root() {
    let e = env("planlink", "[[out/reviewer]]");
    escape_link(&e);
    let p = e.plan().unwrap();
    assert_eq!(p.steps[0].skip.as_ref().unwrap().code, "path.outside_root");
    e.assert_no_abs(&p.steps[0].skip);
}

#[test]
fn too_many_links_is_limit() {
    let body = "[[a]]\n".repeat(1001);
    let e = env("links", &body);
    let err = e.plan().unwrap_err();
    assert_eq!(
        (err.code, err.params["limit"].as_str()),
        ("index.limit", "links")
    );
}

#[test]
fn too_many_steps_is_limit_and_sink_empty() {
    let e = env("steps", "[[a]]");
    let steps = json!(
        (0..1001)
            .map(|_| json!({"relPath": "a.md", "destDir": ""}))
            .collect::<Vec<_>>()
    );
    let err = e.run(steps).unwrap_err();
    assert_eq!(
        (err.code, err.params["limit"].as_str()),
        ("index.limit", "steps")
    );
    e.ws_empty();
}

#[test]
fn oversized_linked_file_is_skipped_too_large() {
    let e = env("big", "[[big]]");
    fs::write(e.vault.join("Templates/big.md"), vec![b'a'; (1 << 20) + 1]).unwrap();
    let b = e
        .run(json!([{"relPath": "big.md", "destDir": ""}]))
        .unwrap();
    assert!(
        matches!(&b.results[0], StepResult::Skipped { code: "file.too_large", params, .. } if { e.assert_no_abs(params); true })
    );
    e.ws_empty();
}

#[test]
fn duplicate_links_plan_identically() {
    let e = env("dups", "[[a]] [[a]] [[a]]");
    let p = e.plan().unwrap();
    assert_eq!(p.steps.len(), 3);
    assert!(
        p.steps
            .iter()
            .all(|s| s.skip == p.steps[0].skip && s.rel_path == "a.md")
    );
}

fn paths_line(n: usize) -> String {
    let v: Vec<String> = (0..n).map(|i| format!("p{i}")).collect();
    format!("paths: [{}]\n", v.join(", "))
}

#[test]
fn too_many_paths_is_limit_before_any_linked_read() {
    // [[a]] would plan a step; [[../x]] a rejection: neither may be produced.
    let e = env_paths("paths65", "[[a]]", &paths_line(65));
    let err = e.plan().unwrap_err();
    assert_eq!(
        (
            err.code,
            err.params["limit"].as_str(),
            err.params["max"].as_str()
        ),
        ("index.limit", "paths", "64")
    );
}

#[test]
fn max_paths_times_max_links_stays_bounded() {
    let e = env_paths("paths64", &"[[a]]\n".repeat(1000), &paths_line(64));
    let p = e.plan().unwrap();
    assert_eq!(p.steps.len(), 1000);
    let total: usize = p.steps.iter().map(|s| s.candidates.len()).sum();
    assert!(total <= 1000 * 65, "{total}");
}
