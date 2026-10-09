//! Hostile-input tests for the containment rule and bounded reads. Each rejection asserts the sink
//! (the file was never opened), not only the checker's return value.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::{Path, PathBuf};

use mda_vault::{Root, VaultError, contain, read_bounded};

/// Fresh temp dir, removed on drop (std only, no `tempfile`).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-{}-{}", std::process::id(), name));
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

/// `<tmp>/vault` created and rooted.
fn vault(t: &Tmp) -> Root {
    fs::create_dir_all(t.join("vault")).unwrap();
    Root::new(&t.join("vault")).unwrap()
}

#[cfg(unix)]
fn chmod(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
}

/// True (with a stderr reason) when mode 000 does not block us, i.e. we run as root.
#[cfg(unix)]
fn running_as_root(p: &Path) -> bool {
    if fs::File::open(p).is_ok() {
        eprintln!("SKIPPED: mode-000 file is readable (running as root), sink cannot be proven");
        return true;
    }
    false
}

fn outside(e: Result<impl std::fmt::Debug, VaultError>) -> bool {
    matches!(e, Err(VaultError::OutsideRoot { .. }))
}

// --- ported from the extension's path-containment.test.ts ---

#[test]
fn child_is_contained() {
    let t = Tmp::new("child");
    let r = vault(&t);
    fs::create_dir_all(t.join("vault/Snippets")).unwrap();
    fs::write(t.join("vault/Snippets/a.md"), "x").unwrap();
    let got = contain(&r, Path::new("Snippets/a.md")).unwrap();
    assert_eq!(got, r.path().join("Snippets/a.md"));
}

#[test]
fn nested_child_is_contained() {
    let t = Tmp::new("nested");
    let r = vault(&t);
    fs::create_dir_all(t.join("vault/a/b/c")).unwrap();
    fs::write(t.join("vault/a/b/c/d.md"), "x").unwrap();
    assert!(contain(&r, Path::new("a/b/c/d.md")).is_ok());
}

#[test]
fn root_itself_is_contained() {
    let t = Tmp::new("rootself");
    let r = vault(&t);
    assert_eq!(contain(&r, r.path()).unwrap(), r.path());
}

#[test]
fn trailing_separator_does_not_matter() {
    let t = Tmp::new("trailing");
    fs::create_dir_all(t.join("vault/Snippets")).unwrap();
    let r = Root::new(Path::new(&format!("{}/", t.join("vault").display()))).unwrap();
    assert!(contain(&r, Path::new("Snippets")).is_ok());
    fs::create_dir_all(t.join("vault-backup")).unwrap();
    assert!(outside(contain(&r, &t.join("vault-backup/a.md"))));
}

#[test]
fn prefix_sibling_is_outside() {
    let t = Tmp::new("prefix");
    let r = vault(&t);
    fs::create_dir_all(t.join("vault-evil")).unwrap();
    fs::write(t.join("vault-evil/x"), "x").unwrap();
    assert!(outside(contain(&r, &t.join("vault-evil/x"))));
    assert!(outside(contain(&r, Path::new("../vault-evil/x"))));
}

#[test]
fn unrelated_absolute_is_outside() {
    let t = Tmp::new("unrelated");
    let r = vault(&t);
    assert!(outside(contain(&r, Path::new("/etc/passwd"))));
}

#[test]
fn traversal_is_refused() {
    let t = Tmp::new("traversal");
    let r = vault(&t);
    assert!(outside(contain(&r, Path::new("../escape"))));
    assert!(outside(contain(&r, Path::new("../../etc/passwd"))));
}

#[test]
fn traversal_back_inside_is_ok_when_parent_exists() {
    let t = Tmp::new("backinside");
    let r = vault(&t);
    fs::create_dir_all(t.join("vault/Snippets")).unwrap();
    fs::create_dir_all(t.join("vault/Commands")).unwrap();
    fs::write(t.join("vault/Commands/a.md"), "x").unwrap();
    assert!(contain(&r, Path::new("Snippets/../Commands/a.md")).is_ok());
}

#[test]
fn percent_encoded_traversal_is_inert() {
    let t = Tmp::new("percent");
    let r = vault(&t);
    let got = contain(&r, Path::new("%2e%2e/etc/passwd")).unwrap();
    assert_eq!(got, r.path().join("%2e%2e/etc/passwd"));
}

#[test]
fn parent_of_root_is_outside() {
    let t = Tmp::new("parent");
    let r = vault(&t);
    assert!(outside(contain(&r, &t.join(""))));
}

// --- hostile cases ---

#[cfg(unix)]
#[test]
fn sink_symlink_escape_never_opens_target() {
    let t = Tmp::new("symescape");
    let r = vault(&t);
    let secret = t.join("secret.md");
    fs::write(&secret, "top secret").unwrap();
    chmod(&secret, 0o000);
    if running_as_root(&secret) {
        return;
    }
    std::os::unix::fs::symlink(&secret, t.join("vault/link.md")).unwrap();
    assert!(outside(contain(&r, Path::new("link.md"))));
    // Io(PermissionDenied) here would prove the file was opened.
    assert!(outside(read_bounded(&r, Path::new("link.md"), 1024)));
    chmod(&secret, 0o600);
}

#[cfg(unix)]
#[test]
fn sink_size_check_precedes_open() {
    let t = Tmp::new("sizefirst");
    let r = vault(&t);
    let f = t.join("vault/big.md");
    fs::write(&f, vec![b'a'; 11]).unwrap();
    chmod(&f, 0o000);
    if running_as_root(&f) {
        return;
    }
    let e = read_bounded(&r, Path::new("big.md"), 10);
    chmod(&f, 0o600);
    assert!(matches!(
        e,
        Err(VaultError::TooLarge {
            size: 11,
            max: 10,
            ..
        })
    ));
}

#[cfg(unix)]
#[test]
fn dangling_symlink_is_outside() {
    let t = Tmp::new("dangling");
    let r = vault(&t);
    let target = t.join("not-yet");
    std::os::unix::fs::symlink(&target, t.join("vault/dangling")).unwrap();
    assert!(outside(contain(&r, Path::new("dangling"))));
    assert!(outside(contain(&r, Path::new("dangling/sub/f.md"))));
    assert!(!target.exists());
}

#[test]
fn nonexistent_inside_is_ok_and_dotdot_escape_is_not() {
    let t = Tmp::new("newpath");
    let r = vault(&t);
    let got = contain(&r, Path::new("new/dir/file.md")).unwrap();
    assert_eq!(got, r.path().join("new/dir/file.md"));
    assert!(outside(contain(&r, Path::new("new/../../x"))));
}

#[test]
fn exactly_max_bytes_is_ok() {
    let t = Tmp::new("exact");
    let r = vault(&t);
    fs::write(t.join("vault/a.md"), b"12345").unwrap();
    assert_eq!(read_bounded(&r, Path::new("a.md"), 5).unwrap(), b"12345");
    assert!(matches!(
        read_bounded(&r, Path::new("a.md"), 4),
        Err(VaultError::TooLarge { .. })
    ));
}

#[test]
fn missing_root_is_not_found() {
    let t = Tmp::new("noroot");
    assert!(matches!(
        Root::new(&t.join("nope")),
        Err(VaultError::NotFound { .. })
    ));
}

#[test]
fn directory_is_not_regular() {
    let t = Tmp::new("dirread");
    let r = vault(&t);
    fs::create_dir_all(t.join("vault/d")).unwrap();
    assert!(matches!(
        read_bounded(&r, Path::new("d"), 10),
        Err(VaultError::NotRegular { .. })
    ));
}

#[cfg(unix)]
#[test]
fn fifo_is_not_regular_and_does_not_block() {
    let t = Tmp::new("fifo");
    let r = vault(&t);
    let st = std::process::Command::new("mkfifo")
        .arg(t.join("vault/pipe"))
        .status()
        .unwrap();
    assert!(st.success());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_bounded(&r, Path::new("pipe"), 10));
    });
    let got = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("read_bounded blocked on a FIFO");
    assert!(matches!(got, Err(VaultError::NotRegular { .. })));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_case_insensitive_candidate_is_contained() {
    let t = Tmp::new("case");
    fs::create_dir_all(t.join("CaseVault")).unwrap();
    fs::write(t.join("CaseVault/a.md"), "x").unwrap();
    let r = Root::new(&t.join("CaseVault")).unwrap();
    assert!(contain(&r, &t.join("casevault/a.md")).is_ok());
}

/// Unexercised by the macOS gate.
#[cfg(windows)]
#[test]
fn windows_verbatim_root_and_plain_candidate() {
    let t = Tmp::new("winprefix");
    fs::create_dir_all(t.join("v")).unwrap();
    fs::write(t.join("v/a.md"), "x").unwrap();
    let r = Root::new(&t.join("v")).unwrap();
    assert!(contain(&r, &t.join("v/a.md")).is_ok());
}

// --- source guard ---

/// Comment-stripped substring scan for lossy path-to-text conversions. Ceiling: not an AST.
fn offenders(src: &str) -> Vec<&'static str> {
    let mut code = String::new();
    let mut in_block = false;
    for line in src.lines() {
        let mut rest = line;
        let mut kept = String::new();
        loop {
            if in_block {
                match rest.find("*/") {
                    Some(i) => {
                        rest = &rest[i + 2..];
                        in_block = false;
                    }
                    None => break,
                }
            } else {
                let line_c = rest.find("//");
                let block_c = rest.find("/*");
                match (line_c, block_c) {
                    (Some(l), Some(b)) if b < l => {
                        kept.push_str(&rest[..b]);
                        rest = &rest[b + 2..];
                        in_block = true;
                    }
                    (Some(l), _) => {
                        kept.push_str(&rest[..l]);
                        break;
                    }
                    (None, Some(b)) => {
                        kept.push_str(&rest[..b]);
                        rest = &rest[b + 2..];
                        in_block = true;
                    }
                    (None, None) => {
                        kept.push_str(rest);
                        break;
                    }
                }
            }
        }
        code.push_str(&kept);
        code.push('\n');
    }
    ["to_str(", "to_string_lossy(", "as_bytes("]
        .into_iter()
        .filter(|p| code.contains(p))
        .collect()
}

#[test]
fn source_guard_detects_and_passes() {
    assert_eq!(
        offenders("let s = x.to_str(); // as_bytes("),
        vec!["to_str("]
    );
    assert!(offenders("// x.to_str(\n/* as_bytes( */ let a = 1;").is_empty());
    for f in ["src/contain.rs", "src/read.rs"] {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(f);
        let src = fs::read_to_string(&p).unwrap();
        assert!(offenders(&src).is_empty(), "{f}");
    }
}

#[test]
fn huge_candidate_is_rejected_fast() {
    let t = Tmp::new("huge");
    let r = vault(&t);
    let cand: PathBuf = std::iter::repeat_n("a", 100_000).collect();
    let start = std::time::Instant::now();
    let got = contain(&r, &cand);
    assert!(outside(got));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

/// Pin for W2's write path: a new file under a symlinked dir that points outside.
#[cfg(unix)]
#[test]
fn new_file_under_outside_symlink_dir_is_outside() {
    let t = Tmp::new("ldir");
    let r = vault(&t);
    fs::create_dir_all(t.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(t.join("elsewhere"), t.join("vault/ldir")).unwrap();
    assert!(outside(contain(&r, Path::new("ldir/new.md"))));
    assert!(!t.join("elsewhere/new.md").exists());
}
