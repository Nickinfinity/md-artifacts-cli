//! The type-location resolver (spec §15): the resolution table, the guards and the conflict rule.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::location::{physical, resolve};
use mda_ops::{ArtifactType, Ctx, Root};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-loc-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

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

fn fixture(name: &str, into: &Path) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mda/tests/fixtures")
        .join(name);
    let dst = into.join(name);
    copy_dir(&src, &dst);
    dst.canonicalize().unwrap()
}

/// Session vault A, vault B, config dir; `cfg` is the machine file (`{B}` = B's path).
fn setup(name: &str, cfg: &str) -> (PathBuf, PathBuf, Ctx) {
    let t = tmp(name);
    let a = fixture("vault_loc_a", &t);
    let b = fixture("vault_loc_b", &t);
    let cd = t.join("cfg");
    fs::create_dir_all(&cd).unwrap();
    let text = cfg
        .replace("{B}", b.to_str().unwrap())
        .replace("{A}", a.to_str().unwrap());
    fs::write(cd.join("config.toml"), text).unwrap();
    let ctx = Ctx::new(Some(Root::new(&a).unwrap())).with_config_dir(cd);
    (a, b, ctx)
}

#[test]
fn snippet_in_dir_lib() {
    let (_a, b, ctx) = setup("lib", "[types.Snippet]\nvault = '{B}'\ndir = \"Lib\"\n");
    let r = physical(&ctx, "Snippets/s.md").unwrap();
    assert_eq!(r.rel, "Lib/s.md");
    assert_eq!(r.place.root.path(), b.as_path());
    assert_eq!(r.virt, "Snippets/s.md");
    assert_eq!(r.place.base.as_deref(), Some("Lib"));
}

#[test]
fn resolution_table() {
    // default: session vault, registry dir
    let (a, _b, ctx) = setup("tbl", "");
    let r = physical(&ctx, "Snippets/a.md").unwrap();
    assert_eq!(
        (r.rel.as_str(), r.place.root.path()),
        ("Snippets/a.md", a.as_path())
    );
    // dir (own vault)
    let (_a, b, ctx) = setup("tbl2", "[types.Template]\nvault = '{B}'\ndir = \"deep\"\n");
    let r = physical(&ctx, "Templates/t.md").unwrap();
    assert_eq!(
        (r.rel.as_str(), r.place.root.path()),
        ("deep/t.md", b.as_path())
    );
    // root (own vault): base None, rel without the type dir
    let (_a, b, ctx) = setup("tbl3", "[types.Template]\nvault = '{B}'\nroot = true\n");
    let r = physical(&ctx, "Templates/deep/t.md").unwrap();
    assert_eq!(
        (r.rel.as_str(), r.place.root.path()),
        ("deep/t.md", b.as_path())
    );
    assert!(r.place.base.is_none());
    // session vault only: dir on the session vault
    let (a, _b, ctx) = setup("tbl4", "[types.Template]\ndir = \"deep\"\n");
    let p = resolve(&ctx, ArtifactType::Template).unwrap();
    assert_eq!(
        (p.root.path(), p.base.as_deref()),
        (a.as_path(), Some("deep"))
    );
}

#[test]
fn no_config_dir_is_identity_and_echo_as_sent() {
    let t = tmp("ident");
    let a = fixture("vault_loc_a", &t);
    let ctx = Ctx::new(Some(Root::new(&a).unwrap()));
    let r = physical(&ctx, "snippets/a.md").unwrap();
    assert_eq!(
        (r.rel.as_str(), r.virt.as_str()),
        ("snippets/a.md", "snippets/a.md")
    );
}

#[test]
fn root_type_in_session_vault_conflicts_with_others() {
    let (_a, _b, ctx) = setup("rootconf", "[types.Template]\nroot = true\n");
    let e = physical(&ctx, "Snippets/a.md").unwrap_err();
    assert_eq!(e.code, "config.location_conflict");
    assert_eq!(e.params["type"], "Snippet");
    assert_eq!(e.params["other"], "Template");
    let e = physical(&ctx, "Templates/t.md").unwrap_err();
    assert_eq!(e.code, "config.location_conflict");
    // a type in the other vault is untouched by it
    assert!(
        physical(
            &setup(
                "rootconf2",
                "[types.Template]\nroot = true\nvault = '{B}'\n"
            )
            .2,
            "Snippets/a.md"
        )
        .is_ok()
    );
}

#[test]
fn shared_dir_conflicts_case_insensitively() {
    let (_a, _b, ctx) = setup("dirconf", "[types.Snippet]\ndir = \"commands\"\n");
    let e = physical(&ctx, "Snippets/a.md").unwrap_err();
    assert_eq!(
        (e.code, e.params["other"].as_str()),
        ("config.location_conflict", "Command")
    );
}

#[test]
fn corrupt_machine_file_is_config_invalid() {
    let (_a, _b, ctx) = setup("corrupt", "[[[ not toml");
    assert_eq!(
        physical(&ctx, "Snippets/a.md").unwrap_err().code,
        "config.invalid"
    );
}

#[test]
fn bad_type_vault_is_location_invalid() {
    let t = tmp("badv");
    for v in ["/nonexistent-mda-loc", &format!("{}", t.display())] {
        let (_a, _b, ctx) = setup(
            "badv2",
            &format!("[types.Snippet]\nvault = '{v}'\ndir = \"Lib\"\n"),
        );
        let e = physical(&ctx, "Snippets/s.md").unwrap_err();
        assert_eq!(
            (e.code, e.params["reason"].as_str()),
            ("config.location_invalid", "vault")
        );
    }
}

#[test]
fn root_location_refuses_dot_segments() {
    let (_a, b, ctx) = setup("dot", "[types.Snippet]\nvault = '{B}'\nroot = true\n");
    for p in [
        "Snippets/.obsidian/app.json",
        "Snippets/x/.git/y.md",
        "Snippets/.obsidian/x.md",
    ] {
        assert_eq!(
            physical(&ctx, p).unwrap_err().code,
            "artifact.bad_path",
            "{p}"
        );
    }
    assert!(b.join(".obsidian/app.json").exists());
    assert!(physical(&ctx, "Snippets/Lib/s.md").is_ok());
}

#[cfg(unix)]
#[test]
fn pinned_dir_symlinks_are_refused() {
    use std::os::unix::fs::symlink;
    let (_a, b, ctx) = setup("pin", "[types.Snippet]\nvault = '{B}'\ndir = \"Lib\"\n");
    let outside = tmp("pin-out");
    fs::write(outside.join("s.md"), "secret").unwrap();
    fs::remove_dir_all(b.join("Lib")).unwrap();
    symlink(&outside, b.join("Lib")).unwrap();
    assert_eq!(
        physical(&ctx, "Snippets/s.md").unwrap_err().code,
        "artifact.bad_path"
    );
    let e = resolve(&ctx, ArtifactType::Snippet).unwrap_err();
    assert_eq!(
        (e.code, e.params["path"].as_str()),
        ("artifact.bad_path", "Snippets")
    );
    assert_eq!(fs::read_to_string(outside.join("s.md")).unwrap(), "secret");
    // Lib -> Commands (another type's dir in the same vault)
    fs::remove_file(b.join("Lib")).unwrap();
    fs::create_dir_all(b.join("Commands")).unwrap();
    fs::write(b.join("Commands/c.md"), "cmd").unwrap();
    symlink(b.join("Commands"), b.join("Lib")).unwrap();
    assert_eq!(
        physical(&ctx, "Snippets/c.md").unwrap_err().code,
        "artifact.bad_path"
    );
    assert_eq!(fs::read_to_string(b.join("Commands/c.md")).unwrap(), "cmd");
}

#[cfg(unix)]
#[test]
fn default_dir_symlink_stays_allowed() {
    use std::os::unix::fs::symlink;
    let t = tmp("defsym");
    let a = fixture("vault_loc_a", &t);
    let elsewhere = t.join("elsewhere");
    fs::rename(a.join("Snippets"), &elsewhere).unwrap();
    symlink(&elsewhere, a.join("Snippets")).unwrap();
    let ctx = Ctx::new(Some(Root::new(&a).unwrap())).with_config_dir(t.join("cfg"));
    assert!(physical(&ctx, "Snippets/a.md").is_ok());
}

#[cfg(windows)]
#[test]
fn verbatim_vault_equal_to_session_conflicts() {
    let (a, _b, ctx) = setup("verb", "");
    // `canonicalize` is already verbatim on Windows; build the `\\?\` form from the plain path so
    // the stored value is exactly one `\\?\` prefix over the session vault.
    let verbatim = format!(r"\\?\{}", mda_vault::plain_path(&a).display());
    assert!(verbatim.starts_with(r"\\?\") && !verbatim.starts_with(r"\\?\\\?\"));
    let cfg = format!("[types.Template]\nvault = '{verbatim}'\nroot = true\n");
    fs::write(ctx.config_dir.as_ref().unwrap().join("config.toml"), cfg).unwrap();
    assert_eq!(
        physical(&ctx, "Snippets/a.md").unwrap_err().code,
        "config.location_conflict"
    );
}
