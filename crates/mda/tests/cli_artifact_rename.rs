//! `mda artifact rename`: human output and the symlink sinks (real binary, temp vault copies).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

mod common;
use common::{copy_tree, run};

const VAULT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");
#[cfg(unix)]
const HELLO: &str = "ec08b1ff85244ca852728e7bf761ccb8cfcaf796abd9ddfef6e5f8aef5082c1e";
const WIDGET: &str = "dfb8eaca9f611de341f5176b4e6bc16256a221a5f0b89bbf812aaa96e1abc6a0";

fn temp_vault(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-rn-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(std::path::Path::new(VAULT), &d);
    d
}

#[test]
fn human_prints_wrote_and_index_link_warning() {
    let d = temp_vault("human");
    let out = run(
        &[
            "--vault",
            d.to_str().unwrap(),
            "artifact",
            "rename",
            "Templates/sub/widget.md",
            "Templates/sub/w2.md",
            "--hash",
            WIDGET,
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&format!("wrote Templates/sub/w2.md  {WIDGET}")),
        "{text}"
    );
    // Warnings go to stderr (`print::warnings`).
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("rename.index_link"), "{err}");
    assert!(err.contains("an index links the old name"), "{err}");
    assert!(d.join("Templates/sub/w2.md").exists());
    assert!(!d.join("Templates/sub/widget.md").exists());
    let _ = std::fs::remove_dir_all(d);
}

// Symlink sinks: Unix only (its callers are gated too).
#[cfg(unix)]
fn rename_through(
    link: &str,
    target: &std::path::Path,
    d: &std::path::Path,
) -> std::process::Output {
    std::os::unix::fs::symlink(target, d.join(link)).unwrap();
    run(
        &[
            "--vault",
            d.to_str().unwrap(),
            "artifact",
            "rename",
            "Snippets/hello.md",
            &format!("{link}/x.md"),
            "--hash",
            HELLO,
        ],
        b"",
    )
}

#[cfg(unix)]
#[test]
fn symlink_out_of_vault_is_refused_and_nothing_lands() {
    let d = temp_vault("out");
    let outside = std::env::temp_dir().join(format!("mda-rn-{}-outside", std::process::id()));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).unwrap();
    let out = rename_through("Snippets/out", &outside, &d);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("path.outside_root"));
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    assert!(d.join("Snippets/hello.md").exists());
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn symlink_to_other_type_dir_is_bad_path_and_nothing_lands() {
    let d = temp_vault("pin");
    let before = std::fs::read_dir(d.join("Templates")).unwrap().count();
    let out = rename_through("Snippets/link", std::path::Path::new("../Templates"), &d);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("artifact.bad_path"));
    assert!(!d.join("Templates/x.md").exists());
    assert_eq!(
        std::fs::read_dir(d.join("Templates")).unwrap().count(),
        before
    );
    assert!(d.join("Snippets/hello.md").exists());
    let _ = std::fs::remove_dir_all(d);
}
