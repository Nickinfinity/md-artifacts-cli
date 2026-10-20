//! `index.create` against temp vaults: limits, containment sinks, long names, mid-batch collisions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::index::StepResult;
use mda_ops::index_create::{CreateIndexRequest, create};
use mda_ops::{Ctx, OpError, Root};
use serde_json::json;

struct Env {
    vault: PathBuf,
    ctx: Ctx,
}

fn tmp(name: &str, kind: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-ic-{}-{name}-{kind}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn env(name: &str) -> Env {
    let vault = tmp(name, "vault");
    fs::create_dir_all(vault.join("Templates")).unwrap();
    let ctx = Ctx::new(Some(Root::new(&vault).unwrap()));
    Env { vault, ctx }
}

fn run(e: &Env, dir: &str, paths: &[String]) -> Result<mda_ops::index::BatchResponse, OpError> {
    let req: CreateIndexRequest =
        serde_json::from_value(json!({"type": "Template", "dir": dir, "paths": paths})).unwrap();
    create(&e.ctx, req)
}

fn assert_no_abs(e: &Env, err: &OpError) {
    assert!(!format!("{:?}", err.params).contains(&e.vault.display().to_string()));
}

fn names(dir: &Path) -> usize {
    fs::read_dir(dir).unwrap().count()
}

#[test]
fn too_many_paths_is_refused_and_nothing_written() {
    let e = env("limit");
    let paths: Vec<String> = (0..1_001).map(|i| format!("f{i}.ts")).collect();
    let err = run(&e, "Templates", &paths).unwrap_err();
    assert_eq!(err.code, "index.limit");
    assert_eq!(err.params["limit"], "steps");
    assert_eq!(err.params["max"], "1000");
    assert_eq!(names(&e.vault.join("Templates")), 0);
}

#[cfg(unix)]
#[test]
fn dir_through_symlink_out_is_refused_and_target_untouched() {
    let e = env("link");
    let out = tmp("link", "outside");
    std::os::unix::fs::symlink(&out, e.vault.join("Templates/out")).unwrap();
    let err = run(&e, "Templates/out", &["a.ts".into()]).unwrap_err();
    assert_eq!(err.code, "path.outside_root");
    assert_no_abs(&e, &err);
    assert_eq!(names(&out), 0);
}

#[test]
fn overlong_segment_is_refused_and_nothing_written() {
    let e = env("long");
    let long = "a".repeat(300);
    let err = run(&e, "Templates/g", &["ok.ts".into(), format!("{long}.ts")]).unwrap_err();
    assert_eq!(err.code, "naming.too_long");
    assert_no_abs(&e, &err);
    assert_eq!(err.params["max"], "255");
    assert_eq!(names(&e.vault.join("Templates")), 0);
}

#[test]
fn error_params_are_exact() {
    let e = env("params");
    let err = run(&e, "Templates/g", &["a.ts".into(), "../x".into()]).unwrap_err();
    assert_eq!(err.code, "path.parent_segment");
    assert_eq!(err.params.len(), 1);
    assert_eq!(err.params["path"], "../x");
    let err = run(&e, "Templates/g", &["!!!/b.ts".into()]).unwrap_err();
    assert_eq!(err.code, "index.unsluggable");
    assert_eq!(err.params.len(), 1);
    assert_eq!(err.params["segment"], "!!!");
    assert_eq!(names(&e.vault.join("Templates")), 0);
}

#[test]
fn hostile_paths_carry_no_machine_path() {
    let e = env("hostile");
    for p in ["../x", "/etc/passwd", "a\\b", "C:/x", "!!!"] {
        let err = run(&e, "Templates/g", &[p.into()]).unwrap_err();
        assert_no_abs(&e, &err);
    }
    assert_eq!(names(&e.vault.join("Templates")), 0);
}

#[test]
fn preexisting_file_skips_only_itself() {
    let e = env("mid");
    fs::create_dir_all(e.vault.join("Templates/g")).unwrap();
    fs::write(e.vault.join("Templates/g/b.md"), "mine").unwrap();
    let r = run(
        &e,
        "Templates/g",
        &["a.ts".into(), "b.ts".into(), "c.ts".into()],
    )
    .unwrap();
    assert_eq!((r.written, r.skipped), (3, 1));
    assert!(matches!(&r.results[1], StepResult::Skipped { code, .. } if *code == "file.exists"));
    assert_eq!(
        fs::read_to_string(e.vault.join("Templates/g/b.md")).unwrap(),
        "mine"
    );
    for f in ["a", "c", "index"] {
        assert!(e.vault.join(format!("Templates/g/{f}.md")).is_file());
    }
}

// H3b.2 cross-op check (TS `create-index.service.test.ts:62-75`): an index made by `index.create` plans back
// to exactly its siblings — every link resolves, nothing is skipped or rejected.
#[test]
fn created_index_plans_back() {
    let e = env("planback");
    let out = run(&e, "Templates/gen", &["a.ts".into(), "sub/b.ts".into()]).unwrap();
    assert_eq!((out.written, out.skipped), (3, 0));
    let req: mda_ops::index::PlanRequest =
        serde_json::from_value(json!({"path": "Templates/gen/index.md"})).unwrap();
    let plan = mda_ops::index::plan(&e.ctx, req).unwrap();
    let rels: Vec<&str> = plan.steps.iter().map(|s| s.rel_path.as_str()).collect();
    assert_eq!(rels, ["a.md", "sub/b.md"]);
    assert!(
        plan.steps.iter().all(|s| s.skip.is_none()),
        "{:?}",
        plan.steps
    );
    assert!(plan.rejected.is_empty());
    let StepResult::Written { path, .. } = &out.results[2] else {
        panic!("index not written: {:?}", out.results[2]);
    };
    assert_eq!(path, "Templates/gen/index.md");
}
