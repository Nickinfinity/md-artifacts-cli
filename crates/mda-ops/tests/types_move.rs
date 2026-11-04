//! `types.move.plan|apply` hostile inputs (T2.2): every refusal leaves the source vault, the target
//! vault and the machine config byte-identical (no new file, no `.mda-tmp`).
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use mda_ops::migrate::FileRef;
use mda_ops::types_move::{ApplyRequest, PlanRequest, apply, plan};
use mda_ops::{Ctx, OpError, Root};
use serde_json::{Value, json};

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            fs::copy(e.path(), t).unwrap();
        }
    }
}

/// Source vault (session), target vault B, config dir, a dir outside both.
struct Env {
    src: PathBuf,
    b: PathBuf,
    cd: PathBuf,
    outside: PathBuf,
    ctx: Ctx,
}

fn env(name: &str) -> Env {
    let t = std::env::temp_dir().join(format!("mda-mv-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&t);
    fs::create_dir_all(&t).unwrap();
    let t = t.canonicalize().unwrap();
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mda/tests/fixtures");
    let (src, b) = (t.join("src"), t.join("b"));
    copy_dir(&fx.join("vault_move_src"), &src);
    copy_dir(&fx.join("vault_loc_b"), &b);
    let (cd, outside) = (t.join("cfg"), t.join("outside"));
    fs::create_dir_all(&cd).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(cd.join("config.toml"), "").unwrap();
    fs::write(outside.join("secret.md"), "outside\n").unwrap();
    let ctx = Ctx::new(Some(Root::new(&src).unwrap())).with_config_dir(cd.clone());
    Env {
        src,
        b,
        cd,
        outside,
        ctx,
    }
}

/// Every entry below `d` (symlinks as their target), for before/after equality.
fn tree(d: &Path) -> BTreeMap<String, Vec<u8>> {
    fn go(base: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            let key = p.strip_prefix(base).unwrap().display().to_string();
            let m = fs::symlink_metadata(&p).unwrap();
            if m.file_type().is_symlink() {
                out.insert(
                    key,
                    fs::read_link(&p)
                        .unwrap()
                        .display()
                        .to_string()
                        .into_bytes(),
                );
            } else if m.is_dir() {
                out.insert(key, Vec::new());
                go(base, &p, out);
            } else {
                out.insert(key, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    go(d, d, &mut out);
    out
}

fn snap(e: &Env) -> [BTreeMap<String, Vec<u8>>; 4] {
    [tree(&e.src), tree(&e.b), tree(&e.cd), tree(&e.outside)]
}

fn to_b(e: &Env, dir: &str) -> Value {
    json!({"vault": e.b.to_str().unwrap(), "dir": dir})
}

fn do_plan(e: &Env, to: &Value) -> Result<mda_ops::types_move::PlanResponse, OpError> {
    plan(
        &e.ctx,
        serde_json::from_value::<PlanRequest>(json!({"type": "Snippet", "to": to})).unwrap(),
    )
}

fn do_apply(
    e: &Env,
    to: &Value,
    files: &[FileRef],
) -> Result<mda_ops::types_move::ApplyResponse, OpError> {
    let files = serde_json::to_value(files).unwrap();
    apply(
        &e.ctx,
        serde_json::from_value::<ApplyRequest>(
            json!({"type": "Snippet", "to": to, "files": files}),
        )
        .unwrap(),
    )
}

/// Plan to `to`, apply the plan's files; the result must be `code` and nothing may change.
fn refused(e: &Env, to: &Value, files: &[FileRef], code: &str) -> OpError {
    let before = snap(e);
    let err = do_apply(e, to, files).unwrap_err();
    assert_eq!(err.code, code, "{err:?}");
    assert_eq!(snap(e), before, "a refused move changed the tree");
    err
}

fn planned(e: &Env, dir: &str) -> Vec<FileRef> {
    do_plan(e, &to_b(e, dir)).unwrap().files
}

#[test]
fn bad_vault_values() {
    let e = env("vault");
    let file = e.src.join("Snippets/a.md");
    for v in [
        "rel/vault",
        file.to_str().unwrap(),
        e.src.join("nope").to_str().unwrap(),
    ] {
        let to = json!({"vault": v, "dir": "Lib"});
        let err = refused(&e, &to, &[], "config.location_invalid");
        assert_eq!(err.params["reason"], "vault", "{v}");
    }
}

#[test]
fn bad_dir_values() {
    let e = env("dir");
    for d in ["../x", ".git"] {
        let err = refused(&e, &to_b(&e, d), &[], "config.location_invalid");
        assert_eq!(err.params["reason"], "dir", "{d}");
    }
}

#[test]
fn target_symlinked_outside_or_to_other_type_dir() {
    let e = env("link");
    let files = planned(&e, "New");
    fs::remove_dir_all(e.b.join("Lib")).unwrap();
    symlink(&e.outside, e.b.join("Lib")).unwrap();
    refused(&e, &to_b(&e, "Lib"), &files, "artifact.bad_path");
    assert_eq!(fs::read(e.outside.join("secret.md")).unwrap(), b"outside\n");
    fs::remove_file(e.b.join("Lib")).unwrap();
    fs::create_dir_all(e.b.join("Commands")).unwrap();
    symlink(e.b.join("Commands"), e.b.join("Lib")).unwrap();
    refused(&e, &to_b(&e, "Lib"), &files, "artifact.bad_path");
}

#[test]
fn another_types_dir_or_a_busy_root_conflicts() {
    let e = env("conf");
    let files = planned(&e, "New");
    refused(
        &e,
        &json!({"dir": "commands"}), // session vault: Command keeps Commands/
        &files,
        "config.location_conflict",
    );
    // the session vault holds the other types' default dirs
    refused(
        &e,
        &json!({"root": true}),
        &files,
        "config.location_conflict",
    );
}

#[test]
fn foreign_or_dotted_paths_in_files() {
    let e = env("paths");
    for p in ["Commands/x.md", "Snippets/../x.md", "Snippets/a.md/.."] {
        let f = [FileRef {
            path: p.into(),
            hash: "h".into(),
        }];
        refused(&e, &to_b(&e, "New"), &f, "artifact.bad_path");
    }
}

#[test]
fn file_added_after_plan_is_stale() {
    let e = env("stale");
    let files = planned(&e, "New");
    fs::write(e.src.join("Snippets/new.md"), "# n\n").unwrap();
    let err = refused(&e, &to_b(&e, "New"), &files, "move.plan_stale");
    assert_eq!(err.params["path"], "Snippets/new.md");
}

#[test]
fn source_edited_after_plan_conflicts() {
    let e = env("edit");
    let files = planned(&e, "New");
    fs::write(e.src.join("Snippets/a.md"), "# changed\n").unwrap();
    let err = refused(&e, &to_b(&e, "New"), &files, "file.conflict");
    assert_eq!(err.params["path"], "Snippets/a.md");
}

#[test]
fn symlinked_source_is_never_read_and_is_counted() {
    let e = env("ln");
    symlink(e.outside.join("secret.md"), e.src.join("Snippets/ln.md")).unwrap();
    let p = do_plan(&e, &to_b(&e, "New")).unwrap();
    assert_eq!(p.files.len(), 2);
    assert_eq!(p.warnings[0].code, "move.left_behind");
    assert_eq!(p.warnings[0].params["count"], "1");
    let r = do_apply(&e, &to_b(&e, "New"), &p.files).unwrap();
    assert_eq!(r.moved, 2);
    assert_eq!(r.warnings[0].params["count"], "1");
    assert_eq!(fs::read(e.outside.join("secret.md")).unwrap(), b"outside\n");
    assert!(!e.b.join("New/ln.md").exists());
    assert!(fs::symlink_metadata(e.src.join("Snippets/ln.md")).is_ok());
    assert_eq!(fs::read(e.b.join("New/a.md")).unwrap(), b"# a\n");
    assert!(!e.src.join("Snippets/a.md").exists());
    // the switch landed: the type now resolves into B/New
    assert!(
        fs::read_to_string(e.cd.join("config.toml"))
            .unwrap()
            .contains("New")
    );
}

#[test]
fn root_source_into_a_dir_of_the_same_vault_overlaps() {
    let e = env("overlap");
    // Snippet owns the session vault's root; every other type lives in B.
    let mut cfg = String::from("[types.Snippet]\nroot = true\n");
    for t in mda_ops::TYPES.iter().map(|i| i.artifact_type) {
        if t != mda_ops::ArtifactType::Snippet {
            cfg.push_str(&format!(
                "[types.{}]\nvault = '{}'\n",
                t.as_str(),
                e.b.display()
            ));
        }
    }
    fs::write(e.cd.join("config.toml"), cfg).unwrap();
    let err = refused(&e, &json!({"dir": "Inside"}), &[], "move.overlap");
    assert_eq!(err.params["dir"], "Inside");
}
