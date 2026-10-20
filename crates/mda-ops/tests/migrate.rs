//! `migrate.*.plan|apply` against temp vaults: hostile paths and links leak and write nothing, the
//! apply budget and entry cap hold, plans carry no values, both migrations are idempotent.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::migrate::{
    ApplyRequest, ApplyResponse, ApplyResult, FileRef, PlanRequest, frontmatter_apply,
    frontmatter_plan, vks_apply, vks_plan,
};
use mda_ops::{Ctx, Root};
use mda_vault::content_hash;

const FM: &str = "---\ntype: snippet\n---\nbody\n";

fn vault(name: &str) -> (PathBuf, Ctx) {
    let d = std::env::temp_dir().join(format!("mda-migrate-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    for sub in ["vault/Snippets", "vault/Variables", "outside"] {
        fs::create_dir_all(d.join(sub)).unwrap();
    }
    let d = d.canonicalize().unwrap();
    let ctx = Ctx::new(Some(Root::new(&d.join("vault")).unwrap()));
    (d, ctx)
}

fn fref(path: &str, bytes: &[u8]) -> FileRef {
    FileRef {
        path: path.into(),
        hash: content_hash(bytes),
    }
}

fn apply_fm(ctx: &Ctx, files: Vec<FileRef>) -> Result<ApplyResponse, mda_ops::OpError> {
    frontmatter_apply(ctx, ApplyRequest { files })
}

fn skipped_code(r: &ApplyResult) -> (&'static str, &BTreeMap<String, String>) {
    match r {
        ApplyResult::Skipped { code, params, .. } => (code, params),
        ApplyResult::Written { path, .. } => panic!("expected skip, wrote {path}"),
    }
}

fn assert_no_abs(params: &BTreeMap<String, String>, base: &Path) {
    for v in params.values() {
        assert!(!Path::new(v).is_absolute(), "absolute param {v}");
        assert!(!v.contains(base.to_str().unwrap()), "root leaked in {v}");
    }
}

#[test]
fn bad_paths_write_nothing() {
    let (d, ctx) = vault("badpaths");
    fs::write(d.join("vault/Snippets/ok.md"), FM).unwrap();
    fs::write(d.join("x.md"), FM).unwrap();
    for bad in [
        "../x.md",
        "/abs.md",
        "Snippets/../../x.md",
        "Notes/x.md",
        "Snippets/x.txt",
    ] {
        let e = apply_fm(
            &ctx,
            vec![
                fref("Snippets/ok.md", FM.as_bytes()),
                fref(bad, FM.as_bytes()),
            ],
        )
        .unwrap_err();
        assert_eq!(e.code, "artifact.bad_path", "{bad}");
        assert_eq!(e.params["path"], bad);
        assert_eq!(
            fs::read_to_string(d.join("vault/Snippets/ok.md")).unwrap(),
            FM
        );
        assert_eq!(fs::read_to_string(d.join("x.md")).unwrap(), FM);
    }
}

#[cfg(unix)]
#[test]
fn dir_link_out() {
    let (d, ctx) = vault("dirlink");
    fs::write(d.join("outside/x.md"), FM).unwrap();
    std::os::unix::fs::symlink(d.join("outside"), d.join("vault/Snippets/out")).unwrap();
    let p = frontmatter_plan(&ctx, PlanRequest {}).unwrap();
    assert!(p.changes.is_empty() && p.skipped.is_empty());
    let r = apply_fm(&ctx, vec![fref("Snippets/out/x.md", FM.as_bytes())]).unwrap();
    let (code, params) = skipped_code(&r.results[0]);
    assert_eq!(code, "path.outside_root");
    assert_no_abs(params, &d);
    assert_eq!(fs::read_to_string(d.join("outside/x.md")).unwrap(), FM);
}

#[cfg(unix)]
#[test]
fn file_link_out() {
    let (d, ctx) = vault("filelink");
    let body = "---\ntype: snippet\n---\nLEAK-MARKER-9\n";
    fs::write(d.join("outside/s.md"), body).unwrap();
    std::os::unix::fs::symlink(d.join("outside/s.md"), d.join("vault/Snippets/leak.md")).unwrap();
    let p = frontmatter_plan(&ctx, PlanRequest {}).unwrap();
    assert!(p.changes.is_empty());
    assert_eq!(p.skipped[0].code, "path.outside_root");
    let r = apply_fm(&ctx, vec![fref("Snippets/leak.md", body.as_bytes())]).unwrap();
    assert_eq!(skipped_code(&r.results[0]).0, "path.outside_root");
    let json = serde_json::to_string(&(&p, &r)).unwrap();
    assert!(!json.contains("LEAK-MARKER-9") && !json.contains(&body.len().to_string()));
    assert_eq!(fs::read_to_string(d.join("outside/s.md")).unwrap(), body);
}

#[cfg(unix)]
#[test]
fn two_links_one_target() {
    let (d, ctx) = vault("twolinks");
    fs::write(d.join("vault/Snippets/real.md"), FM).unwrap();
    std::os::unix::fs::symlink("real.md", d.join("vault/Snippets/link.md")).unwrap();
    let p = frontmatter_plan(&ctx, PlanRequest {}).unwrap();
    assert_eq!(p.changes.len(), 2);
    assert_eq!(p.changes[0].hash, p.changes[1].hash);
    let r = apply_fm(
        &ctx,
        p.changes
            .iter()
            .map(|c| FileRef {
                path: c.path.clone(),
                hash: c.hash.clone(),
            })
            .collect(),
    )
    .unwrap();
    // walk order: link.md before real.md
    assert!(
        matches!(&r.results[0], ApplyResult::Written { path, .. } if path == "Snippets/link.md")
    );
    assert_eq!(skipped_code(&r.results[1]).0, "file.conflict");
    assert_eq!(
        fs::read_to_string(d.join("vault/Snippets/real.md")).unwrap(),
        "---\nartifactType: Snippet\n---\nbody\n"
    );
    assert!(
        fs::symlink_metadata(d.join("vault/Snippets/link.md"))
            .unwrap()
            .is_symlink()
    );
}

#[test]
fn budget() {
    let (d, ctx) = vault("budget");
    let pad = fs::File::create(d.join("vault/Snippets/pad.md")).unwrap();
    pad.set_len(1 << 20).unwrap();
    drop(pad);
    fs::write(d.join("vault/Snippets/z.md"), FM).unwrap();
    // 64 entries on the same 1 MiB file: each counts its size against the call.
    let pad_hash = content_hash(&fs::read(d.join("vault/Snippets/pad.md")).unwrap());
    let mut files: Vec<FileRef> = (0..64)
        .map(|_| FileRef {
            path: "Snippets/pad.md".into(),
            hash: pad_hash.clone(),
        })
        .collect();
    files.push(fref("Snippets/z.md", FM.as_bytes()));
    let r = apply_fm(&ctx, files).unwrap();
    for res in &r.results[..64] {
        assert_eq!(skipped_code(res).0, "migrate.up_to_date");
    }
    let (code, params) = skipped_code(&r.results[64]);
    assert_eq!(code, "walk.limit");
    assert_eq!(params["limit"], "bytes");
    assert_eq!(params["max"], "67108864");
    assert_eq!(
        fs::read_to_string(d.join("vault/Snippets/z.md")).unwrap(),
        FM
    );
    let r = apply_fm(&ctx, vec![fref("Snippets/z.md", FM.as_bytes())]).unwrap();
    assert!(matches!(r.results[0], ApplyResult::Written { .. }));
}

#[test]
fn too_many() {
    let (_d, ctx) = vault("toomany");
    let mk = |n| {
        (0..n)
            .map(|_| fref("Snippets/none.md", b""))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        apply_fm(&ctx, mk(10_001)).unwrap_err().code,
        "op.bad_request"
    );
    let r = apply_fm(&ctx, mk(10_000)).unwrap();
    assert_eq!((r.written, r.skipped), (0, 10_000));
    assert!(
        r.results
            .iter()
            .all(|x| skipped_code(x).0 == "path.not_found")
    );
}

#[test]
fn too_large_output() {
    let (d, ctx) = vault("toolarge");
    let head = "---\ntype: agent\n---\n";
    let mut s = String::from(head);
    s.push_str(&"a".repeat(1_048_576 - 5 - head.len()));
    assert_eq!(s.len(), 1_048_576 - 5);
    fs::write(d.join("vault/Snippets/big.md"), &s).unwrap();
    let r = apply_fm(&ctx, vec![fref("Snippets/big.md", s.as_bytes())]).unwrap();
    let (code, params) = skipped_code(&r.results[0]);
    assert_eq!(code, "file.too_large");
    assert_eq!(params["max"], "1048576");
    assert_eq!(
        fs::read_to_string(d.join("vault/Snippets/big.md")).unwrap(),
        s
    );
}

fn all_refs<C>(changes: &[C], f: impl Fn(&C) -> (&str, &str)) -> Vec<FileRef> {
    changes
        .iter()
        .map(|c| {
            let (p, h) = f(c);
            FileRef {
                path: p.into(),
                hash: h.into(),
            }
        })
        .collect()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t)
        } else {
            fs::copy(e.path(), t).unwrap();
        }
    }
}

#[test]
fn idempotent_both() {
    let (d, _) = vault("idem");
    let v = d.join("copy");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../mda/tests/fixtures/vault_migrate"),
        &v,
    );
    let ctx = Ctx::new(Some(Root::new(&v).unwrap()));
    let p = frontmatter_plan(&ctx, PlanRequest {}).unwrap();
    assert!(!p.changes.is_empty());
    let r = apply_fm(&ctx, all_refs(&p.changes, |c| (&c.path, &c.hash))).unwrap();
    assert_eq!(r.skipped, 0, "{r:?}");
    assert!(
        frontmatter_plan(&ctx, PlanRequest {})
            .unwrap()
            .changes
            .is_empty()
    );
    let p = vks_plan(&ctx, PlanRequest {}).unwrap();
    assert!(!p.changes.is_empty());
    let stale = FileRef {
        path: "Variables/old.md".into(),
        hash: "0".repeat(64),
    };
    let mut files = all_refs(&p.changes, |c| (&c.path, &c.hash));
    let r = vks_apply(&ctx, ApplyRequest { files: vec![stale] }).unwrap();
    assert_eq!(skipped_code(&r.results[0]).0, "file.conflict");
    files.retain(|f| f.path != "Variables/old.md");
    let r = vks_apply(&ctx, ApplyRequest { files }).unwrap();
    assert_eq!(r.skipped, 0, "{r:?}");
    let fresh = vks_plan(&ctx, PlanRequest {}).unwrap();
    assert_eq!(fresh.changes.len(), 1, "only old.md left");
    let r = vks_apply(
        &ctx,
        ApplyRequest {
            files: all_refs(&fresh.changes, |c| (&c.path, &c.hash)),
        },
    )
    .unwrap();
    assert_eq!(r.written, 1);
    assert!(vks_plan(&ctx, PlanRequest {}).unwrap().changes.is_empty());
}

#[test]
fn plans_carry_no_values() {
    let (d, ctx) = vault("novalues");
    fs::write(
        d.join("vault/Variables/s.md"),
        "---\nartifactType: Variables\ntitle: S\n---\n\n```yaml VKs\nVK-a=SECRET-mig-41\n```\n",
    )
    .unwrap();
    let p = vks_plan(&ctx, PlanRequest {}).unwrap();
    assert_eq!(p.changes.len(), 1);
    assert!(!serde_json::to_string(&p).unwrap().contains("SECRET-mig-41"));
}

#[test]
fn plan_is_linear() {
    let (d, ctx) = vault("linear");
    for i in 0..2000 {
        fs::write(
            d.join(format!("vault/Variables/f{i}.md")),
            "---\nartifactType: Variables\ntitle: S\n---\n\n```yaml VKs\nVK-a=1\n```\n",
        )
        .unwrap();
    }
    let t = std::time::Instant::now();
    let p = vks_plan(&ctx, PlanRequest {}).unwrap();
    assert_eq!(p.changes.len(), 2000);
    assert!(t.elapsed().as_secs() < 3, "{:?}", t.elapsed());
}

/// H6.2 cross-op: validate → both migrations → validate agree, and the migrated values read back the same.
#[test]
fn validate_migrate_validate_agree() {
    use mda_ops::artifact::{ReadRequest, read};
    use mda_ops::validate::{ValidateRequest, validate};
    let (d, _) = vault("crossop");
    let v = d.join("copy");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../mda/tests/fixtures/vault_migrate"),
        &v,
    );
    let ctx = Ctx::new(Some(Root::new(&v).unwrap()));
    let vars_of = |path: &str| {
        let a = read(&ctx, ReadRequest { path: path.into() })
            .unwrap()
            .artifact;
        let blocks: Vec<_> = a.blocks.iter().map(|b| b.vars.clone()).collect();
        (a.vars, blocks)
    };
    let before = [
        "Variables/legacy.md",
        "Variables/sets.md",
        "Variables/crlf.md",
    ]
    .map(vars_of);

    let r = validate(&ctx, ValidateRequest {}).unwrap();
    assert_eq!((r.files, r.errors, r.warnings), (20, 1, 13));

    let p = frontmatter_plan(&ctx, PlanRequest {}).unwrap();
    let a = apply_fm(&ctx, all_refs(&p.changes, |c| (&c.path, &c.hash))).unwrap();
    assert_eq!((a.written, a.skipped), (5, 0), "{a:?}");
    // Fresh hashes after the frontmatter pass: `old.md` is listed with its new bytes.
    let p = vks_plan(&ctx, PlanRequest {}).unwrap();
    assert!(p.changes.iter().any(|c| c.path == "Variables/old.md"));
    let a = vks_apply(
        &ctx,
        ApplyRequest {
            files: all_refs(&p.changes, |c| (&c.path, &c.hash)),
        },
    )
    .unwrap();
    assert_eq!((a.written, a.skipped), (5, 0), "{a:?}");

    let r = validate(&ctx, ValidateRequest {}).unwrap();
    assert_eq!((r.errors, r.warnings), (1, 3), "{:?}", r.issues);
    assert!(r.issues.iter().all(|i| i.code != "migrate.pending"));
    let mut warn: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i.code != "vks.control_char")
        .map(|i| i.code)
        .collect();
    warn.sort_unstable();
    assert_eq!(
        warn,
        [
            "migrate.unfenced_vars",
            "vars.fenceless_section",
            "vars.mixed_file"
        ]
    );
    assert!(
        frontmatter_plan(&ctx, PlanRequest {})
            .unwrap()
            .changes
            .is_empty()
    );
    assert!(vks_plan(&ctx, PlanRequest {}).unwrap().changes.is_empty());
    let after = [
        "Variables/legacy.md",
        "Variables/sets.md",
        "Variables/crlf.md",
    ]
    .map(vars_of);
    assert_eq!(before, after);
}
