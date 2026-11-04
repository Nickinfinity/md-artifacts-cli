//! The one Variables collector (X-19): walk the Variables place, keep Variables files parsed, drop
//! other `.md`, keep read failures as entries. Shared by `vars.list`, `vars.search` and
//! `varsets.match`. Also the location-aware walk and path mapping every walking op shares (D-10).

use mda_core::model::ParsedArtifact;
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::ArtifactType;
use std::path::Path;

use mda_vault::{Root, contain, walk_md_multi};

use crate::artifact::MAX_ARTIFACT_BYTES;
use crate::error::{vault_error, vault_error_at};
use crate::location::{Place, resolve};
use crate::{Ctx, OpError};

/// One `.md` under `Variables/`: its virtual path and its parse, or the read error (virtual params).
pub(crate) struct Scanned {
    pub path: String,
    pub file: Result<ParsedArtifact, OpError>,
}

/// Physical vault-relative `rel` (as a walk reports it) -> the virtual path of type `ty`.
pub(crate) fn to_virtual(ty: ArtifactType, base: Option<&str>, rel: &str) -> String {
    let dir = ty.info().dir;
    match base {
        None => format!("{dir}/{rel}"),
        Some(b) if b == dir => rel.to_owned(),
        Some(b) => format!("{dir}{}", rel.strip_prefix(b).unwrap_or(rel)),
    }
}

/// Walk the given places with one shared budget (`walk.limit` refuses the call); `visit(virtual
/// path, bytes or the virtual-path error)`. A root place starts at exactly `""`.
pub(crate) fn walk_places(
    places: &[(ArtifactType, Place)],
    mut visit: impl FnMut(&str, Result<Vec<u8>, OpError>),
) -> Result<(), OpError> {
    let Some((_, first)) = places.first() else {
        return Ok(());
    };
    let starts: Vec<(&Root, &str)> = places
        .iter()
        .map(|(_, p)| (&p.root, p.base.as_deref().unwrap_or("")))
        .collect();
    // A start that escapes its vault fails here, named by its own type dir (never the first
    // place's root or a physical path).
    for ((ty, p), (_, start)) in places.iter().zip(&starts) {
        contain(&p.root, Path::new(start))
            .map_err(|e| vault_error_at(e, &p.root, ty.info().dir))?;
    }
    walk_md_multi(&starts, MAX_ARTIFACT_BYTES, |i, rel, res| {
        let Some((ty, p)) = places.get(i) else { return };
        let virt = to_virtual(*ty, p.base.as_deref(), rel);
        let res = res.map_err(|e| vault_error_at(e, &p.root, &virt));
        visit(&virt, res);
    })
    .map_err(|e| vault_error(e, &first.root))
}

/// Resolve each type once, then [`walk_places`].
pub(crate) fn walk_types(
    ctx: &Ctx,
    types: &[ArtifactType],
    visit: impl FnMut(&str, Result<Vec<u8>, OpError>),
) -> Result<(), OpError> {
    let places = types
        .iter()
        .map(|t| Ok((*t, resolve(ctx, *t)?)))
        .collect::<Result<Vec<_>, OpError>>()?;
    walk_places(&places, visit)
}

/// Walk the Variables place: Variables files parsed, other `.md` dropped, read failures kept as
/// entries. The virtual path is known **before** parsing (else a root-located file gets the wrong
/// type).
pub(crate) fn scan(ctx: &Ctx) -> Result<Vec<Scanned>, OpError> {
    let place = resolve(ctx, ArtifactType::Variables)?;
    scan_place(place)
}

fn scan_place(place: Place) -> Result<Vec<Scanned>, OpError> {
    let mut out = Vec::new();
    walk_places(&[(ArtifactType::Variables, place)], |virt, res| match res {
        Ok(bytes) => {
            let file = parse_from_content(&decode(&bytes), virt);
            // A frontmatter-less `.md` defaults to Variables by its dir (`artifact.read` parity).
            if file.frontmatter.artifact_type == ArtifactType::Variables {
                out.push(Scanned {
                    path: virt.to_owned(),
                    file: Ok(file),
                });
            }
        }
        Err(e) => out.push(Scanned {
            path: virt.to_owned(),
            file: Err(e),
        }),
    })?;
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
        let got = scan_place(Place {
            root: root.clone(),
            base: Some("Variables".into()),
        })
        .unwrap();
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
        let got = scan_place(Place {
            root: root.clone(),
            base: Some("Variables".into()),
        })
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, "Variables/leak.md");
        let e = got[0].file.as_ref().err().unwrap();
        assert_eq!(e.code, crate::error::PATH_OUTSIDE_ROOT);
        assert_eq!(e.params["path"], "Variables/leak.md");
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn second_start_lands_under_second_types_dir() {
        let (p, root) = vault("idx");
        fs::create_dir_all(p.join("vault/Lib")).unwrap();
        fs::write(p.join("vault/Lib/t.md"), "x").unwrap();
        fs::write(p.join("vault/Variables/v.md"), "x").unwrap();
        fs::write(p.join("vault/Variables/.hid"), "x").unwrap();
        let places = [
            (
                ArtifactType::Variables,
                Place {
                    root: root.clone(),
                    base: Some("Variables".into()),
                },
            ),
            (
                ArtifactType::Template,
                Place {
                    root: root.clone(),
                    base: Some("Lib".into()),
                },
            ),
        ];
        let mut seen = Vec::new();
        walk_places(&places, |v, _| seen.push(v.to_owned())).unwrap();
        assert_eq!(seen, ["Variables/v.md", "Templates/t.md"]);
        // a root place: start "" and a virtual dir prefix
        let rootp = Place { root, base: None };
        let mut seen = Vec::new();
        walk_places(&[(ArtifactType::Snippet, rootp)], |v, _| {
            seen.push(v.to_owned())
        })
        .unwrap();
        assert!(seen.contains(&"Snippets/Lib/t.md".to_owned()), "{seen:?}");
        let _ = fs::remove_dir_all(p);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_dir_into_another_types_dir_is_not_visited() {
        let (p, root) = vault("symdir");
        fs::create_dir_all(p.join("vault/Templates")).unwrap();
        fs::write(p.join("vault/Templates/secret.md"), "x").unwrap();
        std::os::unix::fs::symlink(p.join("vault/Templates"), p.join("vault/Variables/ln"))
            .unwrap();
        let places = [(
            ArtifactType::Variables,
            Place {
                root,
                base: Some("Variables".into()),
            },
        )];
        let mut seen = Vec::new();
        walk_places(&places, |v, _| seen.push(v.to_owned())).unwrap();
        assert!(seen.iter().all(|v| !v.contains("secret")), "{seen:?}");
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
        let e = scan_place(Place {
            root: root.clone(),
            base: Some("Variables".into()),
        })
        .err()
        .unwrap();
        assert_eq!(e.code, crate::error::WALK_LIMIT);
        assert_eq!(e.params["limit"], "depth");
        assert_eq!(e.params["max"], "32");
        let _ = fs::remove_dir_all(p);
    }
}
