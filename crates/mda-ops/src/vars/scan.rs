//! The one Variables collector (X-19): walk `Variables/`, keep Variables files parsed, drop other
//! `.md`, keep read failures as entries. Shared by `vars.list`, `vars.search` and `varsets.match`.

use mda_core::model::ParsedArtifact;
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::ArtifactType;
use mda_vault::{Root, walk_md};

use crate::OpError;
use crate::artifact::MAX_ARTIFACT_BYTES;
use crate::error::vault_error;

/// One `.md` under `Variables/`: its vault-relative path and its parse, or the read error
/// (vault-relative params).
pub(crate) struct Scanned {
    pub path: String,
    pub file: Result<ParsedArtifact, OpError>,
}

/// Walk `Variables/`: Variables files parsed, other `.md` dropped, read failures kept as entries;
/// a walk limit refuses the whole call (`walk.limit`).
pub(crate) fn scan(root: &Root) -> Result<Vec<Scanned>, OpError> {
    let mut out = Vec::new();
    walk_md(
        root,
        &[ArtifactType::Variables.info().dir],
        MAX_ARTIFACT_BYTES,
        |rel, res| match res {
            Ok(bytes) => {
                let file = parse_from_content(&decode(&bytes), rel);
                // A frontmatter-less `.md` defaults to Variables by its dir (`artifact.read` parity).
                if file.frontmatter.artifact_type == ArtifactType::Variables {
                    out.push(Scanned {
                        path: rel.to_owned(),
                        file: Ok(file),
                    });
                }
            }
            Err(e) => out.push(Scanned {
                path: rel.to_owned(),
                file: Err(vault_error(e, root)),
            }),
        },
    )
    .map_err(|e| vault_error(e, root))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn vault(name: &str) -> (std::path::PathBuf, Root) {
        let p = std::env::temp_dir().join(format!("mda-scan-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("vault/Variables")).unwrap();
        let root = Root::new(&p.join("vault")).unwrap();
        (p, root)
    }

    #[test]
    fn keeps_variables_drops_others_defaults_by_dir() {
        let (p, root) = vault("kinds");
        let v = p.join("vault/Variables");
        fs::write(
            v.join("a.md"),
            "---\nartifactType: Variables\ntitle: A\n---\n",
        )
        .unwrap();
        fs::write(
            v.join("note.md"),
            "---\nartifactType: Snippet\ntitle: N\n---\n\n```sh\necho\n```\n",
        )
        .unwrap();
        fs::write(v.join("bare.md"), "```yaml VKs\nVK-x: 1\n```\n").unwrap();
        let got = scan(&root).unwrap();
        let paths: Vec<_> = got.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(paths, ["Variables/a.md", "Variables/bare.md"]);
        assert!(got.iter().all(|s| s.file.is_ok()));
        let _ = fs::remove_dir_all(p);
    }

    #[cfg(unix)]
    #[test]
    fn leak_symlink_is_a_relative_error_entry() {
        let (p, root) = vault("leak");
        fs::create_dir_all(p.join("outside")).unwrap();
        fs::write(p.join("outside/s.md"), "MARKER").unwrap();
        std::os::unix::fs::symlink(p.join("outside/s.md"), p.join("vault/Variables/leak.md"))
            .unwrap();
        let got = scan(&root).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, "Variables/leak.md");
        let e = got[0].file.as_ref().err().unwrap();
        assert_eq!(e.code, crate::error::PATH_OUTSIDE_ROOT);
        assert_eq!(e.params["path"], "Variables/leak.md");
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn deep_chain_is_vars_limit() {
        let (p, root) = vault("deep");
        let mut d = p.join("vault/Variables");
        for i in 0..33 {
            d.push(format!("d{i}"));
        }
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("x.md"), "").unwrap();
        let e = scan(&root).err().unwrap();
        assert_eq!(e.code, crate::error::WALK_LIMIT);
        assert_eq!(e.params["limit"], "depth");
        assert_eq!(e.params["max"], "32");
        let _ = fs::remove_dir_all(p);
    }
}
