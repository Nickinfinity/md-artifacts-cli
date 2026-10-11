//! Vault validation, type-dir state and creation, and `Root::new` refusing non-directories.
//! Rejections assert the sink (bytes unchanged, nothing created), not only the return value.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::PathBuf;

use mda_vault::{DirState, Root, VaultError, create_type_dir, dir_state, validate_vault};

/// Fresh temp dir, removed on drop (std only).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-vault-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn join(&self, s: &str) -> PathBuf {
        self.0.join(s)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `<tmp>/v` with the given children created as directories.
fn vault_with(t: &Tmp, dirs: &[&str]) -> Root {
    for d in dirs {
        fs::create_dir_all(t.join(&format!("v/{d}"))).unwrap();
    }
    fs::create_dir_all(t.join("v")).unwrap();
    Root::new(&t.join("v")).unwrap()
}

#[test]
fn validate_accepts_obsidian_dir() {
    let t = Tmp::new("val-ok");
    vault_with(&t, &[".obsidian"]);
    assert!(validate_vault(&t.join("v")).is_ok());
}

#[test]
fn validate_without_obsidian_is_not_obsidian() {
    let t = Tmp::new("val-none");
    vault_with(&t, &[]);
    assert!(matches!(
        validate_vault(&t.join("v")),
        Err(VaultError::NotObsidian { .. })
    ));
}

#[test]
fn validate_obsidian_file_is_not_obsidian() {
    let t = Tmp::new("val-file");
    vault_with(&t, &[]);
    fs::write(t.join("v/.obsidian"), b"x").unwrap();
    assert!(matches!(
        validate_vault(&t.join("v")),
        Err(VaultError::NotObsidian { .. })
    ));
}

#[test]
fn validate_missing_and_file_paths() {
    let t = Tmp::new("val-paths");
    assert!(matches!(
        validate_vault(&t.join("nope")),
        Err(VaultError::NotFound { .. })
    ));
    fs::write(t.join("f"), b"x").unwrap();
    assert!(matches!(
        validate_vault(&t.join("f")),
        Err(VaultError::NotDir { .. })
    ));
}

#[cfg(unix)]
#[test]
fn validate_symlinked_obsidian() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("val-link");
    vault_with(&t, &[]);
    fs::create_dir_all(t.join("elsewhere")).unwrap();
    symlink(t.join("elsewhere"), t.join("v/.obsidian")).unwrap();
    assert!(validate_vault(&t.join("v")).is_ok());
    fs::remove_file(t.join("v/.obsidian")).unwrap();
    symlink(t.join("gone"), t.join("v/.obsidian")).unwrap();
    assert!(matches!(
        validate_vault(&t.join("v")),
        Err(VaultError::NotObsidian { .. })
    ));
}

#[test]
fn root_new_refuses_regular_file() {
    let t = Tmp::new("root-file");
    fs::write(t.join("f"), b"x").unwrap();
    assert!(matches!(
        Root::new(&t.join("f")),
        Err(VaultError::NotDir { .. })
    ));
}

#[cfg(unix)]
#[test]
fn root_new_follows_symlink_to_dir() {
    let t = Tmp::new("root-link");
    fs::create_dir_all(t.join("d")).unwrap();
    std::os::unix::fs::symlink(t.join("d"), t.join("l")).unwrap();
    assert!(Root::new(&t.join("l")).is_ok());
}

#[cfg(unix)]
#[test]
fn dir_state_classifies() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("state");
    let r = vault_with(&t, &["Snippets"]);
    fs::write(t.join("v/Commands"), b"x").unwrap();
    fs::create_dir_all(t.join("out")).unwrap();
    symlink(t.join("out"), t.join("v/Templates")).unwrap();
    let fifo = t.join("v/AIPrompts");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(dir_state(&r, "Snippets"), DirState::Present);
    assert_eq!(dir_state(&r, "Variables"), DirState::Missing);
    assert_eq!(dir_state(&r, "Commands"), DirState::Blocked);
    assert_eq!(dir_state(&r, "Templates"), DirState::Blocked);
    assert_eq!(dir_state(&r, "AIPrompts"), DirState::Blocked);
}

#[test]
fn create_type_dir_creates_once() {
    let t = Tmp::new("create");
    let r = vault_with(&t, &[]);
    assert!(create_type_dir(&r, "Commands").unwrap());
    assert!(t.join("v/Commands").is_dir());
    assert!(!create_type_dir(&r, "Commands").unwrap());
}

#[test]
fn create_type_dir_file_in_the_way_is_untouched() {
    let t = Tmp::new("create-file");
    let r = vault_with(&t, &[]);
    fs::write(t.join("v/Snippets"), b"keep").unwrap();
    assert!(matches!(
        create_type_dir(&r, "Snippets"),
        Err(VaultError::NotDir { .. })
    ));
    assert_eq!(fs::read(t.join("v/Snippets")).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn create_type_dir_refuses_escaping_symlinks() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("create-link");
    let r = vault_with(&t, &[]);
    fs::create_dir_all(t.join("out")).unwrap();
    symlink(t.join("out"), t.join("v/Snippets")).unwrap();
    assert!(matches!(
        create_type_dir(&r, "Snippets"),
        Err(VaultError::OutsideRoot { .. })
    ));
    assert_eq!(fs::read_dir(t.join("out")).unwrap().count(), 0);
    symlink(t.join("gone"), t.join("v/Commands")).unwrap();
    assert!(matches!(
        create_type_dir(&r, "Commands"),
        Err(VaultError::OutsideRoot { .. })
    ));
    assert!(!t.join("gone").exists());
}
