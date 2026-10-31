use super::*;
use std::fs;

fn vault(name: &str, files: &[(&str, &str)]) -> (std::path::PathBuf, Root) {
    let p = std::env::temp_dir().join(format!("mda-rlinks-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    for (rel, body) in files {
        let f = p.join("vault").join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, body).unwrap();
    }
    fs::create_dir_all(p.join("vault/Snippets")).unwrap();
    let root = Root::new(&p.join("vault")).unwrap();
    (p, root)
}

fn idx(body: &str) -> String {
    format!("---\nartifactType: Template\ntitle: I\nindex: true\n---\n\n{body}\n")
}

const LEAF: &str = "---\nartifactType: Template\ntitle: A\n---\n\nx\n";

#[test]
fn index_link_found_with_raw_stripped() {
    let (p, root) = vault(
        "found",
        &[
            (
                "Templates/idx.md",
                &idx("[[sub/a]]\n[[SUB/A#h|al]]\n[x](sub/a.md)\n[[sub/a]]"),
            ),
            ("Templates/sub/a.md", LEAF),
        ],
    );
    let w = index_links_to(&root, "Templates/sub/a.md");
    let links: Vec<_> = w.iter().map(|e| e.params["link"].as_str()).collect();
    assert_eq!(links, ["sub/a", "SUB/A", "sub/a.md"]);
    assert!(w.iter().all(|e| e.code == RENAME_INDEX_LINK));
    assert!(w.iter().all(|e| e.params["index"] == "Templates/idx.md"));
    let _ = fs::remove_dir_all(p);
}

#[test]
fn nested_index_resolves_from_its_dir_and_ignores_rejects_and_embeds() {
    let (p, root) = vault(
        "nested",
        &[
            ("Templates/sub/idx.md", &idx("[[a]]\n![[a]]\n[[../sub/a]]")),
            ("Templates/other.md", &idx("[[a]]")),
            ("Templates/sub/a.md", LEAF),
        ],
    );
    let w = index_links_to(&root, "Templates/sub/a.md");
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].params["index"], "Templates/sub/idx.md");
    assert!(index_links_to(&root, "Templates/a.md").len() == 1);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn non_whole_file_type_skips_walk() {
    let (p, root) = vault("snip", &[("Snippets/a.md", &idx("[[b]]"))]);
    assert!(index_links_to(&root, "Snippets/b.md").is_empty());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn walk_error_keeps_earlier_warnings_and_appends_unchecked() {
    let (p, root) = vault("cap", &[("Templates/idx.md", &idx("[[a]]"))]);
    let seen = warnings(collect(&root, "Templates/a.md", "Templates", |visit| {
        visit(
            "Templates/idx.md",
            Ok(fs::read(root.path().join("Templates/idx.md")).unwrap()),
        );
        Err(VaultError::Limit {
            limit: mda_vault::WalkLimit::Files,
            max: 3,
        })
    }));
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].code, RENAME_INDEX_LINK);
    assert_eq!(seen[1].code, RENAME_LINKS_UNCHECKED);
    assert_eq!(seen[1].params["limit"], "files");
    assert_eq!(seen[1].params["max"], "3");
    let other = warnings(collect(&root, "Templates/a.md", "Templates", |_| {
        Err(VaultError::NotFound { path: "x".into() })
    }));
    assert!(other[0].params.is_empty());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn unreadable_file_skipped_later_files_still_warn() {
    let (p, root) = vault("skip", &[("Templates/z.md", &idx("[[a]]"))]);
    let z = fs::read(root.path().join("Templates/z.md")).unwrap();
    let got = warnings(collect(&root, "Templates/a.md", "Templates", |visit| {
        visit(
            "Templates/bad.md",
            Err(VaultError::NotFound { path: "x".into() }),
        );
        visit("Templates/z.md", Ok(z));
        Ok(())
    }));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].params["index"], "Templates/z.md");
    let _ = fs::remove_dir_all(p);
}

const OLD: &str = "Templates/a.md";
const NEW: &str = "Templates/b.md";

fn bytes(root: &Root, rel: &str) -> Vec<u8> {
    fs::read(root.path().join(rel)).unwrap()
}

fn failed(w: &[OpError]) -> Vec<(&str, &str)> {
    w.iter()
        .filter(|e| e.code == RENAME_LINK_UPDATE_FAILED)
        .map(|e| (e.params["index"].as_str(), e.params["reason"].as_str()))
        .collect()
}

#[test]
fn update_rewrites_only_the_link_target() {
    let (p, root) = vault("upd", &[("Templates/idx.md", &idx("[[a#h|t]]\n[[z]]"))]);
    let (w, done) = update_index_links(&root, OLD, NEW);
    let want = idx("[[b#h|t]]\n[[z]]").into_bytes();
    assert!(w.is_empty());
    assert_eq!(bytes(&root, "Templates/idx.md"), want);
    assert_eq!(done[0].hash, content_hash(&want));
    let _ = fs::remove_dir_all(p);
}

#[test]
fn conflict_when_file_changes_between_scan_and_write() {
    let (p, root) = vault("conf", &[("Templates/idx.md", &idx("[[a]]"))]);
    let scan = collect(&root, OLD, "Templates", |v| {
        walk_md(&root, &["Templates"], MAX_ARTIFACT_BYTES, v)
    });
    let changed = idx("[[a]]\nchanged");
    fs::write(root.path().join("Templates/idx.md"), &changed).unwrap();
    let (w, done) = apply(&root, OLD, NEW, scan);
    assert_eq!(failed(&w), [("Templates/idx.md", "conflict")]);
    assert_eq!(w[1].code, RENAME_INDEX_LINK);
    assert!(done.is_empty());
    assert_eq!(bytes(&root, "Templates/idx.md"), changed.into_bytes());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn non_utf8_index_is_not_patchable_and_untouched() {
    let (p, root) = vault("bin", &[]);
    let mut raw = idx("[[a]]").into_bytes();
    raw.push(0xff);
    fs::create_dir_all(root.path().join("Templates")).unwrap();
    fs::write(root.path().join("Templates/idx.md"), &raw).unwrap();
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert_eq!(failed(&w), [("Templates/idx.md", "not_patchable")]);
    assert!(done.is_empty());
    assert_eq!(bytes(&root, "Templates/idx.md"), raw);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn bom_and_crlf_survive_byte_for_byte() {
    let src = format!(
        "\u{feff}{}",
        idx("[[a]]\n[[a#h]]\n[[z]]").replace('\n', "\r\n")
    );
    let (p, root) = vault("bom", &[("Templates/idx.md", &src)]);
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert!(w.is_empty());
    assert_eq!(done.len(), 1);
    assert_eq!(
        bytes(&root, "Templates/idx.md"),
        src.replace("[[a", "[[b").into_bytes()
    );
    assert_eq!(
        done[0].hash,
        content_hash(&bytes(&root, "Templates/idx.md"))
    );
    let _ = fs::remove_dir_all(p);
}

#[test]
fn size_bound_refuses_before_rewriting() {
    let (p, root) = vault("cap2", &[]);
    let head = idx("[[a]]\n");
    let pad = usize::try_from(MAX_ARTIFACT_BYTES).unwrap() - 2 - head.len();
    let big = format!("{head}{}", "x".repeat(pad));
    fs::create_dir_all(root.path().join("Templates")).unwrap();
    fs::write(root.path().join("Templates/idx.md"), &big).unwrap();
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert_eq!(failed(&w), [("Templates/idx.md", "not_patchable")]);
    assert!(done.is_empty());
    assert_eq!(bytes(&root, "Templates/idx.md"), big.into_bytes());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn moved_index_at_new_path_is_not_rewritten_but_warns_in_default_mode() {
    let (p, root) = vault("self", &[("Templates/b.md", &idx("[[a]]"))]);
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert!(w.is_empty() && done.is_empty());
    assert_eq!(bytes(&root, "Templates/b.md"), idx("[[a]]").into_bytes());
    assert_eq!(index_links_to(&root, OLD).len(), 1);
    let _ = fs::remove_dir_all(p);
}

// Case-sensitive filesystems only: `B.md` is a different file from the moved `b.md`.
#[cfg(target_os = "linux")]
#[test]
fn case_variant_neighbour_of_the_moved_file_is_updated_on_linux() {
    let (p, root) = vault(
        "neigh",
        &[("Templates/B.md", &idx("[[a]]")), ("Templates/b.md", LEAF)],
    );
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert!(w.is_empty());
    assert_eq!(done.len(), 1);
    assert_eq!(bytes(&root, "Templates/B.md"), idx("[[b]]").into_bytes());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn walk_cap_keeps_links_unchecked_last_after_updates_and_failures() {
    let (p, root) = vault(
        "cap3",
        &[
            ("Templates/sub/i.md", &idx("[[a]]")),
            ("Templates/j.md", &idx("[[sub/a]]")),
        ],
    );
    let (old, new) = ("Templates/sub/a.md", "Templates/b.md");
    let scan = collect(&root, old, "Templates", |visit| {
        for rel in ["Templates/sub/i.md", "Templates/j.md"] {
            visit(rel, Ok(bytes(&root, rel)));
        }
        Err(VaultError::Limit {
            limit: mda_vault::WalkLimit::Files,
            max: 3,
        })
    });
    let (w, done) = apply(&root, old, new, scan);
    let got: Vec<_> = w
        .iter()
        .map(|e| (e.code, e.params.get("index").map(String::as_str)))
        .collect();
    assert_eq!(
        got,
        [
            (RENAME_LINK_UPDATE_FAILED, Some("Templates/sub/i.md")),
            (RENAME_INDEX_LINK, Some("Templates/sub/i.md")),
            (RENAME_LINKS_UNCHECKED, None),
        ]
    );
    assert_eq!(w[0].params["reason"], "unreachable");
    assert_eq!(done[0].path, "Templates/j.md");
    assert_eq!(bytes(&root, "Templates/j.md"), idx("[[b]]").into_bytes());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn new_dir_prefix_is_compared_exactly() {
    let (p, root) = vault("exact", &[("Templates/sub/idx.md", &idx("[[a]]"))]);
    let before = bytes(&root, "Templates/sub/idx.md");
    let (w, done) = update_index_links(&root, "Templates/sub/a.md", "Templates/SUB/b.md");
    assert_eq!(failed(&w), [("Templates/sub/idx.md", "unreachable")]);
    assert!(done.is_empty());
    assert_eq!(bytes(&root, "Templates/sub/idx.md"), before);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn file_deleted_between_scan_and_write_is_conflict() {
    let (p, root) = vault("gone", &[("Templates/idx.md", &idx("[[a]]"))]);
    let scan = collect(&root, OLD, "Templates", |v| {
        walk_md(&root, &["Templates"], MAX_ARTIFACT_BYTES, v)
    });
    fs::remove_file(root.path().join("Templates/idx.md")).unwrap();
    let (w, done) = apply(&root, OLD, NEW, scan);
    assert_eq!(failed(&w), [("Templates/idx.md", "conflict")]);
    assert!(done.is_empty());
    assert!(!root.path().join("Templates/idx.md").exists());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn code_text_repeated_in_the_body_is_not_patchable_and_untouched() {
    let src = idx("[[a]]\n\nvars:\n```yaml VKs\nn: [[a]]\n```");
    let (p, root) = vault("dup", &[("Templates/idx.md", &src)]);
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert_eq!(failed(&w), [("Templates/idx.md", "not_patchable")]);
    assert!(done.is_empty());
    assert_eq!(bytes(&root, "Templates/idx.md"), src.into_bytes());
    let _ = fs::remove_dir_all(p);
}

#[cfg(unix)]
#[test]
fn symlinked_index_pointing_outside_writes_nothing_outside() {
    let (p, root) = vault("symout", &[]);
    let outside = p.join("outside.md");
    fs::write(&outside, idx("[[a]]")).unwrap();
    fs::create_dir_all(root.path().join("Templates")).unwrap();
    std::os::unix::fs::symlink(&outside, root.path().join("Templates/l.md")).unwrap();
    let (_, done) = update_index_links(&root, OLD, NEW);
    assert!(done.is_empty());
    assert_eq!(fs::read(&outside).unwrap(), idx("[[a]]").into_bytes());
    let _ = fs::remove_dir_all(p);
}

#[cfg(unix)]
#[test]
fn in_vault_symlink_to_another_type_dir_writes_nothing_there() {
    let (p, root) = vault("pin", &[("Snippets/x.md", &idx("[[a]]"))]);
    fs::create_dir_all(root.path().join("Templates")).unwrap();
    std::os::unix::fs::symlink("../Snippets/x.md", root.path().join("Templates/l.md")).unwrap();
    let (w, done) = update_index_links(&root, OLD, NEW);
    assert_eq!(failed(&w), [("Templates/l.md", "not_patchable")]);
    assert!(done.is_empty());
    assert_eq!(bytes(&root, "Snippets/x.md"), idx("[[a]]").into_bytes());
    let _ = fs::remove_dir_all(p);
}
