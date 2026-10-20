//! `list_dir`: one level, dirs and `.md` files, sorted; hostile entries never escape or block.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::{Path, PathBuf};

use mda_vault::{Root, VaultError, list_dir};

/// Fresh temp dir, removed on drop (std only).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-ls-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("vault")).unwrap();
        Self(p)
    }
    fn vault(&self) -> PathBuf {
        self.0.join("vault")
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

#[test]
fn lists_md_files_and_dirs_sorted() {
    let t = Tmp::new("basic");
    let v = t.vault();
    for f in ["b.md", "A.md", "c.md", "c.txt"] {
        fs::write(v.join(f), "x").unwrap();
    }
    for d in ["Z", "y"] {
        fs::create_dir(v.join(d)).unwrap();
    }
    let l = list_dir(&t.root(), Path::new(".")).unwrap();
    assert_eq!(l.files, ["A.md", "b.md", "c.md"]);
    assert_eq!(l.dirs, ["y", "Z"]);
}

#[test]
fn missing_is_not_found() {
    let t = Tmp::new("missing");
    assert!(matches!(
        list_dir(&t.root(), Path::new("nope")),
        Err(VaultError::NotFound { .. })
    ));
}

#[test]
fn file_is_not_found() {
    let t = Tmp::new("file");
    fs::write(t.vault().join("a.md"), "x").unwrap();
    assert!(matches!(
        list_dir(&t.root(), Path::new("a.md")),
        Err(VaultError::NotFound { .. })
    ));
}

#[cfg(unix)]
#[test]
fn symlinked_dir_outside_is_refused() {
    let t = Tmp::new("symout");
    fs::create_dir(t.0.join("outside")).unwrap();
    fs::write(t.0.join("outside/secret.md"), "s").unwrap();
    std::os::unix::fs::symlink(t.0.join("outside"), t.vault().join("link")).unwrap();
    let got = list_dir(&t.root(), Path::new("link"));
    assert!(matches!(got, Err(VaultError::OutsideRoot { .. })));
    // The link itself still shows as a dir (metadata follows), so the client can try and be refused.
    let top = list_dir(&t.root(), Path::new(".")).unwrap();
    assert_eq!(top.dirs, ["link"]);
    assert!(top.files.is_empty());
}

#[cfg(unix)]
#[test]
fn fifo_is_listed_and_never_opened() {
    let t = Tmp::new("fifo");
    let st = std::process::Command::new("mkfifo")
        .arg(t.vault().join("x.md"))
        .status()
        .unwrap();
    assert!(st.success());
    let root = t.root();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(list_dir(&root, Path::new(".")));
    });
    let got = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("list_dir blocked on a FIFO")
        .unwrap();
    assert_eq!(got.files, ["x.md"]);
}

#[cfg(unix)]
#[test]
fn dangling_md_symlink_is_listed_as_file() {
    let t = Tmp::new("dangling");
    std::os::unix::fs::symlink(t.0.join("gone"), t.vault().join("d.md")).unwrap();
    let l = list_dir(&t.root(), Path::new(".")).unwrap();
    assert_eq!(l.files, ["d.md"]);
    assert!(l.dirs.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_name_is_skipped() {
    use std::os::unix::ffi::OsStrExt;
    let t = Tmp::new("nonutf8");
    let name = std::ffi::OsStr::from_bytes(b"bad\xff.md");
    fs::write(t.vault().join(name), "x").unwrap();
    fs::write(t.vault().join("ok.md"), "x").unwrap();
    let l = list_dir(&t.root(), Path::new(".")).unwrap();
    assert_eq!(l.files, ["ok.md"]);
}
