//! The template-index links that point at a vault path (spec §14.3): `artifact.rename` reports them
//! as `rename.index_link` warnings and never rewrites them.

use std::collections::HashSet;

use mda_core::multi_index::{is_index, plan_links};
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::type_for_dir;
use mda_vault::{Root, VaultError, walk_md};

use crate::OpError;
use crate::artifact::MAX_ARTIFACT_BYTES;
use crate::error::{RENAME_INDEX_LINK, RENAME_LINKS_UNCHECKED, vault_error};

/// One `rename.index_link` per (index, link) under `old`'s type directory that resolves to `old`;
/// a walk that stops early appends one `rename.links_unchecked`.
pub(crate) fn index_links_to(root: &Root, old: &str) -> Vec<OpError> {
    let Some((dir, _)) = old.split_once('/') else {
        return vec![];
    };
    collect(root, old, dir, |visit| {
        walk_md(root, &[dir], MAX_ARTIFACT_BYTES, visit)
    })
}

/// The scan over any walker (the seam lets a test inject a walk error after some files).
fn collect(
    root: &Root,
    old: &str,
    dir: &str,
    walker: impl FnOnce(&mut dyn FnMut(&str, Result<Vec<u8>, VaultError>)) -> Result<(), VaultError>,
) -> Vec<OpError> {
    // Only whole-file types can be indexes.
    if !type_for_dir(dir).is_some_and(|t| t.info().writes_file) {
        return vec![];
    }
    let mut out = Vec::new();
    let mut visit = |rel: &str, res: Result<Vec<u8>, VaultError>| {
        // Unreadable or oversize files are skipped; the renamed file is never its own warning.
        let Ok(bytes) = res else { return };
        if rel.eq_ignore_ascii_case(old) {
            return;
        }
        let file = parse_from_content(&decode(&bytes), rel);
        if !is_index(&file.frontmatter) {
            return;
        }
        let idx_dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
        let mut seen = HashSet::new();
        for step in plan_links(&file.code).steps {
            // The joined vault path, not `rel_path` alone: links resolve from the index's dir.
            let target = format!("{idx_dir}/{}", step.rel_path);
            if target.eq_ignore_ascii_case(old) && seen.insert(step.raw.clone()) {
                out.push(
                    OpError::new(RENAME_INDEX_LINK)
                        .with("index", rel)
                        .with("link", step.raw),
                );
            }
        }
    };
    if let Err(e) = walker(&mut visit) {
        let mut w = OpError::new(RENAME_LINKS_UNCHECKED);
        if matches!(e, VaultError::Limit { .. }) {
            // Same params as `walk.limit`: one mapping only.
            w.params = vault_error(e, root).params;
        }
        out.push(w);
    }
    out
}

#[cfg(test)]
mod tests {
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
        let seen = collect(&root, "Templates/a.md", "Templates", |visit| {
            visit(
                "Templates/idx.md",
                Ok(fs::read(root.path().join("Templates/idx.md")).unwrap()),
            );
            Err(VaultError::Limit {
                limit: mda_vault::WalkLimit::Files,
                max: 3,
            })
        });
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].code, RENAME_INDEX_LINK);
        assert_eq!(seen[1].code, RENAME_LINKS_UNCHECKED);
        assert_eq!(seen[1].params["limit"], "files");
        assert_eq!(seen[1].params["max"], "3");
        let other = collect(&root, "Templates/a.md", "Templates", |_| {
            Err(VaultError::NotFound { path: "x".into() })
        });
        assert!(other[0].params.is_empty());
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn unreadable_file_skipped_later_files_still_warn() {
        let (p, root) = vault("skip", &[("Templates/z.md", &idx("[[a]]"))]);
        let z = fs::read(root.path().join("Templates/z.md")).unwrap();
        let got = collect(&root, "Templates/a.md", "Templates", |visit| {
            visit(
                "Templates/bad.md",
                Err(VaultError::NotFound { path: "x".into() }),
            );
            visit("Templates/z.md", Ok(z));
            Ok(())
        });
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].params["index"], "Templates/z.md");
        let _ = fs::remove_dir_all(p);
    }
}
