//! Atomic writes: create-without-clobber, hash-guarded replace/delete, hostile symlinks. Rejections
//! assert the sink (outside bytes unchanged, no temp left behind), not only the returned error.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::{Path, PathBuf};

use mda_vault::{Root, VaultError, content_hash, create_new, delete, replace};

const MAX: u64 = 1 << 20;

/// Fresh temp dir with `vault/` and `outside/`, removed on drop (std only).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-wr-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("vault/Templates")).unwrap();
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

#[test]
fn create_writes_bytes() {
    let t = Tmp::new("create");
    create_new(&t.root(), Path::new("Templates/a.md"), b"x").unwrap();
    assert_eq!(fs::read(t.vault().join("Templates/a.md")).unwrap(), b"x");
    no_temp(&t.vault().join("Templates"));
}

#[test]
fn create_twice_is_exists_and_keeps_bytes() {
    let t = Tmp::new("twice");
    let r = t.root();
    create_new(&r, Path::new("Templates/a.md"), b"x").unwrap();
    let e = create_new(&r, Path::new("Templates/a.md"), b"y").unwrap_err();
    assert!(matches!(e, VaultError::Exists { .. }), "{e:?}");
    assert_eq!(fs::read(t.vault().join("Templates/a.md")).unwrap(), b"x");
    no_temp(&t.vault().join("Templates"));
}

#[test]
fn create_makes_missing_dirs() {
    let t = Tmp::new("dirs");
    create_new(&t.root(), Path::new("Templates/new/deep/a.md"), b"x").unwrap();
    assert!(t.vault().join("Templates/new/deep/a.md").is_file());
}

#[test]
fn replace_right_hash_writes_wrong_hash_conflicts() {
    let t = Tmp::new("replace");
    let r = t.root();
    let p = t.vault().join("Templates/a.md");
    fs::write(&p, b"old").unwrap();
    let e = replace(&r, Path::new("Templates/a.md"), b"new", "bad", MAX).unwrap_err();
    match e {
        VaultError::Conflict {
            expected, actual, ..
        } => {
            assert_eq!(expected, "bad");
            assert_eq!(actual, content_hash(b"old"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read(&p).unwrap(), b"old");
    no_temp(&t.vault().join("Templates"));
    replace(
        &r,
        Path::new("Templates/a.md"),
        b"new",
        &content_hash(b"old"),
        MAX,
    )
    .unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"new");
    no_temp(&t.vault().join("Templates"));
}

#[test]
fn replace_missing_is_not_found() {
    let t = Tmp::new("replace-missing");
    let e = replace(&t.root(), Path::new("Templates/none.md"), b"n", "h", MAX).unwrap_err();
    assert!(matches!(e, VaultError::NotFound { .. }), "{e:?}");
}

#[test]
fn delete_right_hash_removes_wrong_hash_conflicts() {
    let t = Tmp::new("delete");
    let r = t.root();
    let p = t.vault().join("Templates/a.md");
    fs::write(&p, b"old").unwrap();
    let e = delete(&r, Path::new("Templates/a.md"), "bad", MAX).unwrap_err();
    assert!(matches!(e, VaultError::Conflict { .. }), "{e:?}");
    assert!(p.exists());
    delete(&r, Path::new("Templates/a.md"), &content_hash(b"old"), MAX).unwrap();
    assert!(!p.exists());
}

#[test]
fn delete_missing_is_not_found() {
    let t = Tmp::new("delete-missing");
    let e = delete(&t.root(), Path::new("Templates/none.md"), "h", MAX).unwrap_err();
    assert!(matches!(e, VaultError::NotFound { .. }), "{e:?}");
}

#[cfg(unix)]
#[test]
fn symlinked_file_never_touches_target() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("symfile");
    let secret = t.outside().join("secret.md");
    fs::write(&secret, b"secret").unwrap();
    symlink(&secret, t.vault().join("Templates/out.md")).unwrap();
    let r = t.root();
    let p = Path::new("Templates/out.md");
    let h = content_hash(b"secret");
    let e = replace(&r, p, b"pwn", &h, MAX).unwrap_err();
    assert!(matches!(e, VaultError::OutsideRoot { .. }), "{e:?}");
    let e = create_new(&r, p, b"pwn").unwrap_err();
    assert!(matches!(e, VaultError::OutsideRoot { .. }), "{e:?}");
    let e = delete(&r, p, &h, MAX).unwrap_err();
    assert!(matches!(e, VaultError::NotRegular { .. }), "{e:?}");
    assert_eq!(fs::read(&secret).unwrap(), b"secret");
    no_temp(&t.vault().join("Templates"));
    no_temp(&t.outside());
}

#[cfg(unix)]
#[test]
fn symlinked_dir_never_writes_outside() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("symdir");
    symlink(t.outside(), t.vault().join("Templates/linkdir")).unwrap();
    let e = create_new(&t.root(), Path::new("Templates/linkdir/x.md"), b"pwn").unwrap_err();
    assert!(matches!(e, VaultError::OutsideRoot { .. }), "{e:?}");
    assert_eq!(fs::read_dir(t.outside()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn fifo_replace_is_not_regular_and_returns() {
    let t = Tmp::new("fifo");
    let f = t.vault().join("Templates/f.md");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&f)
            .status()
            .unwrap()
            .success()
    );
    let root = t.root();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(replace(&root, Path::new("Templates/f.md"), b"x", "h", MAX));
    });
    let e = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("replace blocked on FIFO")
        .unwrap_err();
    assert!(matches!(e, VaultError::NotRegular { .. }), "{e:?}");
}

#[test]
fn delete_dotdot_and_absolute_are_outside_root() {
    let t = Tmp::new("delete-escape");
    let victim = t.outside().join("x.md");
    fs::write(&victim, b"keep").unwrap();
    let h = content_hash(b"keep");
    let r = t.root();
    for rel in [Path::new("../outside/x.md"), victim.as_path()] {
        let e = delete(&r, rel, &h, MAX).unwrap_err();
        assert!(
            matches!(e, VaultError::OutsideRoot { .. }),
            "{rel:?}: {e:?}"
        );
    }
    assert_eq!(fs::read(&victim).unwrap(), b"keep");
}
