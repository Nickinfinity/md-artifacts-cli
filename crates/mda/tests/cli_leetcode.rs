//! `mda leetcode read`: human output and the hostile sinks (🔒 a `.md` symlinked out of the vault,
//! an oversize file, a BOM + CRLF exercise).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::run_cfg;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("mda-cli-lc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A vault with a `LeetCode/` dir; returns (vault, config dir).
fn vault(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let v = tmp(&format!("{name}-v"));
    std::fs::create_dir_all(v.join(".obsidian")).unwrap();
    std::fs::create_dir_all(v.join("LeetCode")).unwrap();
    (v, tmp(&format!("{name}-c")))
}

fn read(
    v: &std::path::Path,
    cfg: &std::path::Path,
    path: &str,
    json: bool,
) -> std::process::Output {
    let mut a = vec!["--vault", v.to_str().unwrap(), "leetcode", "read", path];
    if json {
        a.push("--json");
    }
    run_cfg(&a, b"", cfg)
}

const FN: &str = include_str!("fixtures/vault_leetcode/LeetCode/fn.md");

#[test]
fn read_prints_a_summary() {
    let (v, cfg) = vault("human");
    std::fs::write(v.join("LeetCode/fn.md"), FN).unwrap();
    let out = read(&v, &cfg, "LeetCode/fn.md", false);
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let t = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        t.trim(),
        "Two Sum (function, easy, unsolved)\nwarnings: 0\nhash: e95b5c2dab84bad8a2c484916fbeae855b3df7c77de98bc1c775873dee8cba77"
    );
}

#[test]
fn bom_crlf_exercise_reads() {
    let (v, cfg) = vault("bom");
    let src = format!("\u{feff}{}", FN.replace('\n', "\r\n"));
    std::fs::write(v.join("LeetCode/b.md"), &src).unwrap();
    let out = read(&v, &cfg, "LeetCode/b.md", true);
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let j: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(j["exercise"]["title"], "Two Sum");
    // The hash is of the raw bytes (BOM and CRLF included): the generic read of the same bytes agrees.
    std::fs::create_dir_all(v.join("Snippets")).unwrap();
    std::fs::write(v.join("Snippets/b.md"), &src).unwrap();
    let g = run_cfg(
        &[
            "--vault",
            v.to_str().unwrap(),
            "artifact",
            "show",
            "Snippets/b.md",
            "--json",
        ],
        b"",
        &cfg,
    );
    let g: serde_json::Value = serde_json::from_slice(&g.stdout).unwrap();
    assert_eq!(j["hash"], g["hash"]);
}

#[test]
fn oversize_is_refused_unparsed() {
    let (v, cfg) = vault("big");
    let mut src = FN.to_owned();
    src.push_str(&"x".repeat(1024 * 1024));
    std::fs::write(v.join("LeetCode/big.md"), src).unwrap();
    let out = read(&v, &cfg, "LeetCode/big.md", true);
    assert_eq!(out.status.code(), Some(1));
    let t = String::from_utf8(out.stdout).unwrap();
    assert!(t.contains("file.too_large"), "{t}");
    assert!(!t.contains("Two Sum") && !t.contains("exercise"), "{t}");
}

#[cfg(unix)]
#[test]
fn symlink_out_of_the_vault_is_refused_without_leaking() {
    let (v, cfg) = vault("sym");
    let outside = tmp("sym-out");
    let secret = outside.join("secret.md");
    std::fs::write(&secret, FN.replace("Two Sum", "OUTSIDE-SECRET")).unwrap();
    std::os::unix::fs::symlink(&secret, v.join("LeetCode/link.md")).unwrap();
    let out = read(&v, &cfg, "LeetCode/link.md", true);
    assert_eq!(out.status.code(), Some(1));
    let t = String::from_utf8(out.stdout).unwrap();
    assert!(t.contains("\"path.outside_root\""), "{t}");
    assert!(
        !t.contains("OUTSIDE-SECRET") && !t.contains("exercise"),
        "{t}"
    );
    assert!(
        std::fs::read_to_string(&secret)
            .unwrap()
            .contains("OUTSIDE-SECRET")
    );
}

#[test]
fn human_summary_escapes_terminal_controls() {
    let (v, cfg) = vault("esc");
    std::fs::write(
        v.join("LeetCode/e.md"),
        FN.replace("Two Sum", "A\u{1b}[31mB\u{7f}"),
    )
    .unwrap();
    let out = read(&v, &cfg, "LeetCode/e.md", false);
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    let t = String::from_utf8(out.stdout).unwrap();
    assert!(t.starts_with("A\\u{1b}[31mB\\u{7f} ("), "{t:?}");
    assert!(!t.contains('\u{1b}') && !t.contains('\u{7f}'));
}
