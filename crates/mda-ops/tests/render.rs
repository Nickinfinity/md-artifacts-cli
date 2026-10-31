//! `artifact.render` op: size gates run before any render work; errors never leak machine paths.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;

use mda_core::render::Values;
use mda_core::vks::VksValue;
use mda_ops::artifact::MAX_ARTIFACT_BYTES;
use mda_ops::render::{RenderRequest, render};
use mda_ops::{Ctx, OpError, Root};

fn ctx(name: &str) -> (Ctx, String) {
    let dir = std::env::temp_dir().join(format!("mda-rend-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("Snippets")).unwrap();
    fs::write(
        dir.join("Snippets/a.md"),
        "---\nartifactType: Snippet\ntitle: A\n---\n\n```text\nx\n```\n",
    )
    .unwrap();
    let root = Root::new(&dir).unwrap();
    let canon = root.path().display().to_string();
    (Ctx::new(Some(root)), canon)
}

fn req(code: Option<String>, values: Values) -> RenderRequest {
    RenderRequest {
        path: "Snippets/a.md".into(),
        block: None,
        code,
        values,
        join: None,
    }
}

fn no_abs(e: &OpError, canon: &str) {
    for (k, v) in &e.params {
        assert!(!v.contains(canon), "param {k}={v} leaks {canon}");
    }
}

#[test]
fn oversize_code_is_refused_before_render() {
    let (c, canon) = ctx("big");
    let size = usize::try_from(MAX_ARTIFACT_BYTES).unwrap() + 1;
    let e = render(&c, req(Some("a".repeat(size)), Values::new())).unwrap_err();
    assert_eq!(e.code, "file.too_large");
    assert_eq!(e.params["path"], "Snippets/a.md");
    assert_eq!(e.params["size"], size.to_string());
    assert_eq!(e.params["max"], MAX_ARTIFACT_BYTES.to_string());
    no_abs(&e, &canon);
}

#[test]
fn output_limit_is_a_code() {
    let (c, canon) = ctx("lim");
    let mut v = Values::new();
    v.insert("VK-a".into(), VksValue::Str("a".repeat(600 * 1024)));
    let e = render(&c, req(Some("<VK-a><VK-a>".into()), v)).unwrap_err();
    assert_eq!(e.code, "render.limit");
    assert_eq!(e.params["limit"], "output_bytes");
    assert_eq!(e.params["max"], "1048576");
    no_abs(&e, &canon);
}

#[test]
fn bad_values_fail_before_the_file_is_read() {
    let (c, canon) = ctx("order");
    let mut v = Values::new();
    v.insert("VK-a".into(), VksValue::Str("\u{1b}".into()));
    let mut r = req(None, v);
    r.path = "Snippets/missing.md".into();
    let e = render(&c, r).unwrap_err();
    assert_eq!(e.code, "vks.control_char");
    no_abs(&e, &canon);
}

#[test]
fn join_limit_counts_the_joined_bytes() {
    use mda_core::vks::VksList;
    use mda_ops::render::{TextRequest, text};
    let mut v = Values::new();
    v.insert(
        "VK-u".into(),
        VksValue::List(VksList::Strings(vec!["i".into(); 1000])),
    );
    v.insert("VK-g".into(), VksValue::Str("g".repeat(1046)));
    let mk = |join| TextRequest {
        text: "<VK-each:u>\n<VK-g>\n<VK-end:u>".into(),
        values: v.clone(),
        join,
    };
    // Rendered fits; the ` && ` separators push the joined text over the cap.
    let plain = text(&Ctx::new(None), mk(None)).unwrap();
    assert_eq!(plain.output.len(), 1_047_000);
    // The refused joined length: 1000 lines of 1046 bytes, 999 separators of 4 bytes (` && `).
    let joined_len = 1000 * 1046 + 999 * 4;
    assert_eq!(joined_len, 1_049_996);
    assert!(plain.output.len() <= mda_core::render::MAX_OUTPUT_BYTES);
    assert!(joined_len > mda_core::render::MAX_OUTPUT_BYTES);
    let e = text(
        &Ctx::new(None),
        mk(Some(mda_core::render::JoinTechnique::And)),
    )
    .unwrap_err();
    assert_eq!(e.code, "render.limit");
    assert_eq!(e.params["limit"], "output_bytes");
}
