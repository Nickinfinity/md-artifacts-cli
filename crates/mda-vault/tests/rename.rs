//! `rename_new`: atomic no-clobber move. Rejections assert the sink (both files byte-identical, no
//! stray directory, outside dir empty, no temp left), not only the returned error.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::{Path, PathBuf};

use mda_vault::{Root, VaultError, content_hash, rename_new};

const MAX: u64 = 1 << 20;

struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-rn-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("vault/Snippets")).unwrap();
        fs::create_dir_all(p.join("outside")).unwrap();
        Self(p)
    }
    fn vault(&self) -> PathBuf {
        self.0.join("vault")
    }
    fn outside(&self) -> PathBuf {
        self.0.join("outside")
    }
    fn root(&self) -> Root {
        Root::new(&self.vault()).unwrap()
    }
    /// Write `Snippets/a.md`, return its hash.
    fn seed(&self, rel: &str, bytes: &[u8]) -> String {
        fs::write(self.vault().join(rel), bytes).unwrap();
        content_hash(bytes)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn no_temp(dir: &Path) {
    for e in fs::read_dir(dir).unwrap() {
        let n = e.unwrap().file_name();
        assert!(!n.to_string_lossy().contains(".mda-tmp-"), "leftover {n:?}");
    }
}

fn mv(t: &Tmp, from: &str, to: &str, h: &str, bytes: Option<&[u8]>) -> Result<(), VaultError> {
    rename_new(&t.root(), from.as_ref(), to.as_ref(), h, bytes, MAX)
}

#[test]
fn verbatim_moves_bytes() {
    let t = Tmp::new("verbatim");
    let h = t.seed("Snippets/a.md", b"orig");
    mv(&t, "Snippets/a.md", "Snippets/b.md", &h, None).unwrap();
    assert_eq!(fs::read(t.vault().join("Snippets/b.md")).unwrap(), b"orig");
    assert!(!t.vault().join("Snippets/a.md").exists());
    no_temp(&t.vault().join("Snippets"));
}

#[test]
fn bytes_written_at_target() {
    let t = Tmp::new("bytes");
    let h = t.seed("Snippets/a.md", b"orig");
    mv(&t, "Snippets/a.md", "Snippets/b.md", &h, Some(b"new")).unwrap();
    assert_eq!(fs::read(t.vault().join("Snippets/b.md")).unwrap(), b"new");
    assert!(!t.vault().join("Snippets/a.md").exists());
    no_temp(&t.vault().join("Snippets"));
}

#[test]
fn stale_hash_conflicts_and_changes_nothing() {
    let t = Tmp::new("stale");
    t.seed("Snippets/a.md", b"orig");
    let e = mv(&t, "Snippets/a.md", "Snippets/sub/b.md", "bad", None).unwrap_err();
    assert!(matches!(e, VaultError::Conflict { .. }), "{e:?}");
    assert_eq!(fs::read(t.vault().join("Snippets/a.md")).unwrap(), b"orig");
    assert!(!t.vault().join("Snippets/sub").exists(), "stray dir");
}

#[test]
fn existing_target_is_exists_both_untouched_no_stray_dir() {
    let t = Tmp::new("exists");
    let h = t.seed("Snippets/a.md", b"orig");
    t.seed("Snippets/b.md", b"other");
    let e = mv(&t, "Snippets/a.md", "Snippets/b.md", &h, Some(b"new")).unwrap_err();
    assert!(matches!(e, VaultError::Exists { .. }), "{e:?}");
    assert_eq!(fs::read(t.vault().join("Snippets/a.md")).unwrap(), b"orig");
    assert_eq!(fs::read(t.vault().join("Snippets/b.md")).unwrap(), b"other");
    no_temp(&t.vault().join("Snippets"));
}

#[test]
fn missing_parents_created() {
    let t = Tmp::new("parents");
    let h = t.seed("Snippets/a.md", b"orig");
    mv(&t, "Snippets/a.md", "Snippets/x/y/b.md", &h, None).unwrap();
    assert_eq!(
        fs::read(t.vault().join("Snippets/x/y/b.md")).unwrap(),
        b"orig"
    );
}

#[cfg(unix)]
mod hostile {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn from_symlink_refused_target_untouched() {
        let t = Tmp::new("fromlink");
        fs::write(t.outside().join("secret.md"), b"secret").unwrap();
        symlink(
            t.outside().join("secret.md"),
            t.vault().join("Snippets/a.md"),
        )
        .unwrap();
        let h = content_hash(b"secret");
        let e = mv(&t, "Snippets/a.md", "Snippets/b.md", &h, None).unwrap_err();
        assert!(matches!(e, VaultError::NotRegular { .. }), "{e:?}");
        assert_eq!(fs::read(t.outside().join("secret.md")).unwrap(), b"secret");
        assert!(!t.vault().join("Snippets/b.md").exists());
    }

    #[test]
    fn unlink_failure_undoes_the_publish() {
        use std::os::unix::fs::PermissionsExt;
        let t = Tmp::new("unlinkfail");
        fs::create_dir(t.vault().join("Snippets/ro")).unwrap();
        let h = t.seed("Snippets/ro/a.md", b"orig");
        let ro = t.vault().join("Snippets/ro");
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
        let r = mv(&t, "Snippets/ro/a.md", "Snippets/b.md", &h, None);
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
        if r.is_ok() {
            return; // privileged user ignores the mode: nothing to test
        }
        assert!(matches!(r, Err(VaultError::Io { .. })), "{r:?}");
        assert_eq!(fs::read(ro.join("a.md")).unwrap(), b"orig");
        assert!(!t.vault().join("Snippets/b.md").exists(), "left both");
    }

    #[test]
    fn to_symlink_refused_target_untouched() {
        let t = Tmp::new("tolink");
        let h = t.seed("Snippets/a.md", b"orig");
        fs::write(t.outside().join("victim.md"), b"victim").unwrap();
        symlink(
            t.outside().join("victim.md"),
            t.vault().join("Snippets/b.md"),
        )
        .unwrap();
        let e = mv(&t, "Snippets/a.md", "Snippets/b.md", &h, Some(b"new")).unwrap_err();
        assert!(matches!(e, VaultError::NotRegular { .. }), "{e:?}");
        assert_eq!(fs::read(t.outside().join("victim.md")).unwrap(), b"victim");
        assert_eq!(fs::read(t.vault().join("Snippets/a.md")).unwrap(), b"orig");
    }

    #[test]
    fn to_under_symlinked_dir_outside_refused() {
        let t = Tmp::new("dirlink");
        let h = t.seed("Snippets/a.md", b"orig");
        symlink(t.outside(), t.vault().join("Snippets/esc")).unwrap();
        let e = mv(&t, "Snippets/a.md", "Snippets/esc/new/b.md", &h, None).unwrap_err();
        assert!(matches!(e, VaultError::OutsideRoot { .. }), "{e:?}");
        assert_eq!(fs::read_dir(t.outside()).unwrap().count(), 0, "outside dir");
        assert_eq!(fs::read(t.vault().join("Snippets/a.md")).unwrap(), b"orig");
    }
}

#[test]
fn dotdot_and_absolute_refused() {
    let t = Tmp::new("dotdot");
    let h = t.seed("Snippets/a.md", b"orig");
    for to in ["Snippets/../../outside/b.md", "/tmp/mda-abs.md"] {
        let e = mv(&t, "Snippets/a.md", to, &h, None).unwrap_err();
        assert!(matches!(e, VaultError::OutsideRoot { .. }), "{to}: {e:?}");
    }
    let e = mv(&t, "../outside/a.md", "Snippets/b.md", &h, None).unwrap_err();
    assert!(matches!(e, VaultError::OutsideRoot { .. }), "{e:?}");
    assert_eq!(fs::read_dir(t.outside()).unwrap().count(), 0);
    assert_eq!(fs::read(t.vault().join("Snippets/a.md")).unwrap(), b"orig");
}

/// True when the FS folds case (probe: create `x`, look up `X`).
fn case_insensitive(t: &Tmp) -> bool {
    let p = t.vault().join("probe-x");
    fs::write(&p, b"").unwrap();
    let r = t.vault().join("PROBE-X").exists();
    fs::remove_file(&p).unwrap();
    r
}

#[test]
fn case_only_rename_verbatim_and_with_bytes() {
    let t = Tmp::new("case");
    if !case_insensitive(&t) {
        eprintln!("skip: case-sensitive FS");
        return;
    }
    let h = t.seed("Snippets/a.md", b"orig");
    mv(&t, "Snippets/a.md", "Snippets/A.md", &h, None).unwrap();
    let names = |d: &str| -> Vec<String> {
        fs::read_dir(t.vault().join(d))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(names("Snippets"), ["A.md"]);
    assert_eq!(fs::read(t.vault().join("Snippets/A.md")).unwrap(), b"orig");

    mv(&t, "Snippets/A.md", "Snippets/a.md", &h, Some(b"new")).unwrap();
    assert_eq!(names("Snippets"), ["a.md"]);
    assert_eq!(fs::read(t.vault().join("Snippets/a.md")).unwrap(), b"new");
    no_temp(&t.vault().join("Snippets"));
}
