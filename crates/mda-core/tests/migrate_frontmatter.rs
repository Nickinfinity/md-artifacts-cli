//! Frontmatter migration (spec §11.1), ported from `frontmatter-migration.test.ts` :41–:217.
#![allow(clippy::unwrap_used, clippy::panic)] // reason: assertion helpers live outside #[test] fns

use mda_core::migrate::frontmatter::{FmRewrite, rewrite};
use mda_core::migrate::{Normalise, Skip, normalises};
use mda_core::registry::ArtifactType;

fn ok(c: &str) -> FmRewrite {
    rewrite(c)
        .unwrap()
        .unwrap_or_else(|| panic!("no change: {c:?}"))
}

fn none(c: &str) {
    assert_eq!(rewrite(c), Ok(None), "{c:?}");
}

fn quoted(c: &str, raw: &str) {
    assert_eq!(rewrite(c), Err(Skip::QuotedType(raw.to_owned())), "{c:?}");
}

#[test]
fn legacy_map_ports_ts() {
    use ArtifactType::*;
    for (v, t) in [
        ("agent", AIAgentsConfig),
        ("command", Command),
        ("snippet", Snippet),
        ("template", Template),
        ("variables", Variables),
    ] {
        assert_eq!(
            ok(&format!("---\ntype: {v}\n---\n")).artifact_type,
            t,
            "{v}"
        );
    }
    for v in ["aiprompt", "Snippet", "aiagentsconfig"] {
        none(&format!("---\ntype: {v}\n---\n"));
    }
}

#[test]
fn rewrites_port_ts() {
    let r = ok("---\ntype: snippet\n---\nbody");
    assert_eq!(r.old_line, "type: snippet");
    assert_eq!(r.new_line, "artifactType: Snippet");
    assert_eq!(r.content, "---\nartifactType: Snippet\n---\nbody");
    for (v, n) in [
        ("agent", "AIAgentsConfig"),
        ("command", "Command"),
        ("template", "Template"),
        ("variables", "Variables"),
    ] {
        assert_eq!(
            ok(&format!("---\ntype: {v}\n---\nbody")).content,
            format!("---\nartifactType: {n}\n---\nbody")
        );
    }
    none("# just a heading\n\nsome body text");
    none("Example:\n\n```yaml\ntype: snippet\n```\n");
    assert_eq!(
        ok("---\ntype: snippet\n---\n\nExample syntax:\n\n```yaml\ntype: snippet\n```\n").content,
        "---\nartifactType: Snippet\n---\n\nExample syntax:\n\n```yaml\ntype: snippet\n```\n"
    );
    none("---\nartifactType: Snippet\ntype: snippet\n---\nx");
    none("---\ntitle: x\ntype: stray\n---\nx");
    none("---\ntype: bogus\n---\nx");
    none("---\ntype: aiprompt\n---\nx");
    let before = "---\ntype: agent\nindex: true\npaths: [src, lib]\n---\n\n# Index\n";
    let r = ok(before);
    assert_eq!(
        r.content,
        before.replace("type: agent", "artifactType: AIAgentsConfig")
    );
    let r = ok("---\r\ntype: agent\r\nindex: true\r\npaths: [src]\r\n---\r\n\r\nBody.\r\n");
    assert_eq!(r.old_line, "type: agent");
    assert_eq!(
        r.content,
        "---\r\nartifactType: AIAgentsConfig\r\nindex: true\r\npaths: [src]\r\n---\r\n\r\nBody.\r\n"
    );
    assert_eq!(
        ok("---\ntype: snippet\n---").content,
        "---\nartifactType: Snippet\n---"
    );
    none("## heading\n```bash\necho hi\n```\n");
}

#[test]
fn skips_port_ts() {
    assert_eq!(
        rewrite("\n---\ntype: command\n---\nbody"),
        Err(Skip::LeadingBlank)
    );
    assert_eq!(
        rewrite("\r\n\r\n---\ntype: command\n---\nbody"),
        Err(Skip::LeadingBlank)
    );
    none("\n---\ntype: bogus\n---\nbody");
    quoted("---\ntype: \"template\"\n---\nbody", "\"template\"");
    quoted("---\ntype: 'snippet'\n---\nbody", "'snippet'");
    quoted("---\ntype: \"agent'\n---\nbody", "\"agent'");
    quoted("---\ntype: foo\ntype: \"snippet\"\n---\nx", "\"snippet\"");
    none("---\ntype: \"bogus\"\n---\nx");
}

#[test]
fn rust_edges() {
    let r = ok("---\n type : snippet\n---\nx");
    assert_eq!(r.old_line, " type : snippet");
    assert_eq!(r.content, "---\nartifactType: Snippet\n---\nx");
    assert_eq!(ok("---\ntype:snippet\n---\nx").old_line, "type:snippet");
    let r = ok("---\ntype: snippet  \n---\nx");
    assert_eq!(r.old_line, "type: snippet  ");
    assert_eq!(r.content, "---\nartifactType: Snippet\n---\nx");
    assert_eq!(
        ok("---\ntype: snippet\ntype: agent\n---\nx").content,
        "---\nartifactType: Snippet\ntype: agent\n---\nx"
    );
    assert_eq!(
        ok("---\ntype: \u{feff}snippet\u{feff}\n---\nx").artifact_type,
        ArtifactType::Snippet
    );
    assert_eq!(
        ok("---\ntype: snippet\ntags:\n  - a\n  - b\n---\nx\n").content,
        "---\nartifactType: Snippet\ntags:\n  - a\n  - b\n---\nx\n"
    );
    none("---\n---\n");
    none("---\ntitle: t\n---\ntype: snippet\n");
}

#[test]
fn idempotent() {
    for c in [
        "---\ntype: snippet\n---\nbody",
        "---\r\ntype: agent\r\nindex: true\r\n---\r\nB\r\n",
        "---\n type : command  \n---\n",
        "---\ntype: snippet\ntags:\n  - a\n---\n",
    ] {
        assert_eq!(rewrite(&ok(c).content), Ok(None), "{c:?}");
    }
}

#[test]
fn normalises_flags() {
    assert_eq!(normalises(b"\xEF\xBB\xBFx"), [Normalise::Bom]);
    assert_eq!(normalises(b"a\xFFb"), [Normalise::Encoding]);
    assert_eq!(
        normalises(b"\xEF\xBB\xBFa\xFF"),
        [Normalise::Bom, Normalise::Encoding]
    );
    assert!(normalises(b"ascii").is_empty());
}
