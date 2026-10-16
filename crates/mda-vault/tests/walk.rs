//! `walk_md`: the bounded Variables walk (spec §13.2); hostile trees never escape, loop or block.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::PathBuf;

use mda_vault::{Root, VaultError, WalkLimit, walk_md};

/// Fresh temp dir with `vault/` and `outside/`, removed on drop (std only).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-walk-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("vault/Variables")).unwrap();
        fs::create_dir_all(p.join("outside")).unwrap();
        Self(p)
    }
    fn v(&self, rel: &str) -> PathBuf {
        self.0.join("vault").join(rel)
    }
    fn out(&self, rel: &str) -> PathBuf {
        self.0.join("outside").join(rel)
    }
    fn root(&self) -> Root {
        Root::new(&self.0.join("vault")).unwrap()
    }
    fn write(&self, rel: &str, bytes: &[u8]) {
        let p = self.v(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

type Seen = Vec<(String, Result<Vec<u8>, VaultError>)>;

fn walk(t: &Tmp, max_file: u64) -> (Result<(), VaultError>, Seen) {
    let mut seen: Seen = Vec::new();
    let r = walk_md(&t.root(), "Variables", max_file, |rel, res| {
        seen.push((rel.to_owned(), res));
    });
    (r, seen)
}

fn names(seen: &Seen) -> Vec<&str> {
    seen.iter().map(|(n, _)| n.as_str()).collect()
}

fn limit(r: &Result<(), VaultError>) -> (WalkLimit, u64) {
    let Err(VaultError::Limit { limit, max }) = r else {
        unreachable!("expected Limit, got {r:?}")
    };
    (*limit, *max)
}

#[test]
fn walks_md_depth_first_sorted() {
    let t = Tmp::new("order");
    for f in ["b.md", "A.md", "x.txt", "sub/c.md", "sub.md/d.md"] {
        t.write(&format!("Variables/{f}"), b"x");
    }
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(
        names(&seen),
        [
            "Variables/A.md",
            "Variables/b.md",
            "Variables/sub/c.md",
            "Variables/sub.md/d.md"
        ]
    );
    assert!(seen.iter().all(|(_, r)| r.is_ok()));
}

#[test]
fn missing_or_file_start_is_empty() {
    let t = Tmp::new("missing");
    fs::remove_dir(t.v("Variables")).unwrap();
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert!(seen.is_empty());
    t.write("Variables", b"i am a file");
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert!(seen.is_empty());
}

#[cfg(unix)]
#[test]
fn symlink_loop_not_followed() {
    let t = Tmp::new("loop");
    t.write("Variables/a.md", b"x");
    std::os::unix::fs::symlink("..", t.v("Variables/loop")).unwrap();
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(names(&seen), ["Variables/a.md"]);
}

#[cfg(unix)]
#[test]
fn symlinked_dir_out_not_descended() {
    let t = Tmp::new("dirout");
    t.write("Variables/a.md", b"x");
    fs::write(t.out("secret.md"), b"MARKER").unwrap();
    fs::write(t.out("x.txt"), b"x").unwrap();
    std::os::unix::fs::symlink(t.0.join("outside"), t.v("Variables/out")).unwrap();
    std::os::unix::fs::symlink(t.out("x.txt"), t.v("Variables/notes")).unwrap();
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(names(&seen), ["Variables/a.md"]);
}

#[cfg(unix)]
#[test]
fn symlinked_file_out_is_an_error_entry() {
    let t = Tmp::new("fileout");
    fs::write(t.out("secret.md"), b"MARKER").unwrap();
    fs::write(t.out("big"), vec![0u8; 2 << 20]).unwrap();
    std::os::unix::fs::symlink(t.out("secret.md"), t.v("Variables/leak.md")).unwrap();
    std::os::unix::fs::symlink(t.out("big"), t.v("Variables/big.md")).unwrap();
    let (r, seen) = walk(&t, 1 << 20);
    r.unwrap();
    assert_eq!(names(&seen), ["Variables/big.md", "Variables/leak.md"]);
    for (_, res) in &seen {
        assert!(
            matches!(res, Err(VaultError::OutsideRoot { .. })),
            "{res:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlinked_md_dir_inside_is_skipped() {
    let t = Tmp::new("mddirlink");
    t.write("Variables/real/a.md", b"x");
    std::os::unix::fs::symlink(t.v("Variables/real"), t.v("Variables/alias.md")).unwrap();
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(names(&seen), ["Variables/real/a.md"]);
}

#[cfg(unix)]
#[test]
fn fifo_is_not_regular() {
    let t = Tmp::new("fifo");
    let st = std::process::Command::new("mkfifo")
        .arg(t.v("Variables/p.md"))
        .status()
        .unwrap();
    assert!(st.success());
    let root = t.root();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut seen = Vec::new();
        let r = walk_md(&root, "Variables", 1024, |rel, res| {
            seen.push((rel.to_owned(), res.map(|b| b.len())));
        });
        let _ = tx.send((r.is_ok(), seen));
    });
    let (ok, seen) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("walk blocked on a FIFO");
    assert!(ok);
    assert_eq!(seen.len(), 1);
    assert!(matches!(seen[0].1, Err(VaultError::NotRegular { .. })));
}

#[test]
fn files_cap() {
    let t = Tmp::new("files10000");
    for i in 0..10_000 {
        t.write(&format!("Variables/f{i:05}.md"), b"");
    }
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(seen.len(), 10_000);
    t.write("Variables/z.md", b"");
    let (r, seen) = walk(&t, 1024);
    assert_eq!(limit(&r), (WalkLimit::Files, 10_000));
    assert_eq!(
        seen.len(),
        0,
        "cap fires during collection, before any read"
    );
}

#[test]
fn files_cap_counts_dirs_and_spreads_across_dirs() {
    let t = Tmp::new("filesdirs");
    // 9 999 files across the start dir + one subdir holding 1 file: 9 999 + dir + file = 10 001.
    for i in 0..9_999 {
        t.write(&format!("Variables/f{i:05}.md"), b"");
    }
    t.write("Variables/sub/one.md", b"");
    let (r, seen) = walk(&t, 1024);
    assert_eq!(limit(&r).0, WalkLimit::Files);
    assert!(seen.len() <= 10_000);
}

#[test]
fn non_md_names_are_never_counted() {
    let t = Tmp::new("txt");
    for i in 0..9_999 {
        t.write(&format!("Variables/f{i:05}.md"), b"");
    }
    for i in 0..10_001 {
        t.write(&format!("Variables/t{i:05}.txt"), b"");
    }
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(seen.len(), 9_999);
}

#[test]
fn bytes_cap() {
    let t = Tmp::new("bytes");
    for i in 0..64 {
        let f = fs::File::create(t.v(&format!("Variables/a{i:02}.md"))).unwrap();
        f.set_len(1 << 20).unwrap();
    }
    let (r, seen) = walk(&t, 1 << 20);
    r.unwrap();
    assert_eq!(seen.len(), 64);
    t.write("Variables/b.md", b"x");
    let (r, seen) = walk(&t, 1 << 20);
    assert_eq!(limit(&r), (WalkLimit::Bytes, 67_108_864));
    assert!(!names(&seen).contains(&"Variables/b.md"));
}

fn chain(t: &Tmp, depth: usize) {
    let mut rel = String::from("Variables");
    for i in 0..depth {
        rel.push_str(&format!("/d{i}"));
    }
    t.write(&format!("{rel}/x.md"), b"x");
}

#[test]
fn depth_cap() {
    let t = Tmp::new("depth32");
    chain(&t, 32);
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(seen.len(), 1);
    let t = Tmp::new("depth33");
    chain(&t, 33);
    let (r, _) = walk(&t, 1024);
    assert_eq!(limit(&r), (WalkLimit::Depth, 32));
}

#[test]
fn too_large_file_is_an_entry_not_counted() {
    let t = Tmp::new("toolarge");
    t.write("Variables/a.md", &[b'x'; 11]);
    t.write("Variables/b.md", b"ok");
    let (r, seen) = walk(&t, 10);
    r.unwrap();
    assert!(matches!(seen[0].1, Err(VaultError::TooLarge { .. })));
    assert_eq!(seen[1].1.as_ref().unwrap(), b"ok");
}

#[test]
fn start_outside_root_is_refused() {
    let t = Tmp::new("startout");
    let r = walk_md(&t.root(), "../outside", 1024, |_, _| {});
    assert!(matches!(r, Err(VaultError::OutsideRoot { .. })));
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_name_is_skipped() {
    use std::os::unix::ffi::OsStrExt;
    let t = Tmp::new("nonutf8");
    fs::write(
        t.v("Variables")
            .join(std::ffi::OsStr::from_bytes(b"bad\xff.md")),
        b"x",
    )
    .unwrap();
    t.write("Variables/ok.md", b"x");
    let (r, seen) = walk(&t, 1024);
    r.unwrap();
    assert_eq!(names(&seen), ["Variables/ok.md"]);
}
