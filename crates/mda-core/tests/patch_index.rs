//! `patch_index_body` (spec 14.3, D-38): the body of a template index, whatever its layout.

use mda_core::error::Refusal;
use mda_core::patch::{PatchError, patch_index_body};

const P: &str = "Templates/i.md";
const FM: &str = "---\nartifactType: Template\ntitle: I\nindex: true\n---\n";

fn bad() -> PatchError {
    PatchError::Unrepresentable("code")
}

#[test]
fn bare_body() {
    let c = format!("{FM}\n[[a]]\n[[b]]\n");
    assert_eq!(
        patch_index_body(&c, P, "[[x]]\n[[b]]"),
        Ok(format!("{FM}\n[[x]]\n[[b]]\n"))
    );
}

#[test]
fn fenced_body_keeps_closing_fence() {
    let c = format!("{FM}\n```md\n[[a]]\n```\nafter\n");
    assert_eq!(
        patch_index_body(&c, P, "[[x]]"),
        Ok(format!("{FM}\n```md\n[[x]]\n```\nafter\n"))
    );
}

#[test]
fn flagged_body_keeps_notes() {
    let c = format!("{FM}\nnote\n%%oa:start%%\n[[a]]\n%%oa:end%%\ntail\n");
    assert_eq!(
        patch_index_body(&c, P, "[[x]]"),
        Ok(format!(
            "{FM}\nnote\n%%oa:start%%\n[[x]]\n%%oa:end%%\ntail\n"
        ))
    );
}

#[test]
fn crlf_single_line_and_multi_line_fenced() {
    let c = "---\r\nartifactType: Template\r\nindex: true\r\n---\r\n\r\n[[a]]\r\n";
    assert_eq!(
        patch_index_body(c, P, "[[x]]"),
        Ok("---\r\nartifactType: Template\r\nindex: true\r\n---\r\n\r\n[[x]]\r\n".to_owned())
    );
    let c =
        "---\r\nartifactType: Template\r\nindex: true\r\n---\r\n```md\r\n[[a]]\r\n[[b]]\r\n```\r\n";
    assert_eq!(
        patch_index_body(c, P, "[[x]]\r\n[[b]]"),
        Ok("---\r\nartifactType: Template\r\nindex: true\r\n---\r\n```md\r\n[[x]]\r\n[[b]]\r\n```\r\n".to_owned())
    );
}

#[test]
fn bare_crlf_multi_line_is_verbatim() {
    let c = "---\r\nartifactType: Template\r\nindex: true\r\n---\r\n[[a]]\r\n[[b]]\r\n";
    assert_eq!(
        patch_index_body(c, P, "[[x]]\r\n[[b]]"),
        Ok("---\r\nartifactType: Template\r\nindex: true\r\n---\r\n[[x]]\r\n[[b]]\r\n".to_owned())
    );
}

#[test]
fn text_after_frontmatter_is_used() {
    // The code text also sits in the frontmatter; only the body occurrence changes.
    let c = "---\nartifactType: Template\ntitle: [[a]]\nindex: true\n---\n[[a]]\n";
    assert_eq!(
        patch_index_body(c, P, "[[x]]"),
        Ok("---\nartifactType: Template\ntitle: [[a]]\nindex: true\n---\n[[x]]\n".to_owned())
    );
}

#[test]
fn ambiguous_or_unverbatim_refused() {
    // The code text also sits in a note before the flag: two hits after the frontmatter.
    let c = format!("{FM}\n[[a]]\n%%oa:start%%\n[[a]]\n%%oa:end%%\n");
    assert_eq!(patch_index_body(&c, P, "[[x]]"), Err(bad()));
    // Multi-line flagged CRLF: the parser normalises, so the code text is not verbatim.
    let c = "---\r\nartifactType: Template\r\nindex: true\r\n---\r\n%%oa:start%%\r\n[[a]]\r\n[[b]]\r\n%%oa:end%%\r\n";
    assert_eq!(patch_index_body(c, P, "[[x]]"), Err(bad()));
}

#[test]
fn defaults_fence_refused() {
    let c = format!("{FM}\n```yaml VKs\nVK-a: 1\n```\n");
    assert_eq!(
        patch_index_body(&c, P, "[[x]]"),
        Err(PatchError::Refused(Refusal::NoFence))
    );
}

#[test]
fn new_text_with_a_token_is_fine() {
    let c = format!("{FM}\n[[a]]\n");
    assert_eq!(
        patch_index_body(&c, P, "[[x]] <VK-v>"),
        Ok(format!("{FM}\n[[x]] <VK-v>\n"))
    );
}

#[test]
fn mid_body_vars_line_refused() {
    // `vars:` is read as variables, so the code text is not the whole body.
    let c = format!("{FM}\n[[a]]\nvars:\n[[b]]\n");
    assert_eq!(patch_index_body(&c, P, "[[x]]"), Err(bad()));
}

#[test]
fn bare_body_with_defaults_fence_branches_on_layout() {
    let c = format!("{FM}\n[[a]]\n\n```yaml VKs\nVK-a: 1\n```\n");
    assert_eq!(
        patch_index_body(&c, P, "[[x]]"),
        Ok(format!("{FM}\n[[x]]\n\n```yaml VKs\nVK-a: 1\n```\n"))
    );
    // The defaults hold the code text too: two hits, refused.
    let c = format!("{FM}\n[[a]]\n\n```yaml VKs\nVK-a: [[a]]\n```\n");
    assert_eq!(patch_index_body(&c, P, "[[x]]"), Err(bad()));
}
