//! Ports `artifact-patcher.test.ts:23-188, 506-738` (15 tests, minus `patchVarDefaults`) plus
//! Rust-only cases for the patcher and `is_flagged`.
mod common;

use mda_core::error::Refusal::{Flagged, NoFence, NoFrontmatter, Variables};
use mda_core::parse::{is_flagged, parse_from_content};
use mda_core::patch::{CodeTarget, FmField, PatchError, patch_code, patch_field};

const P: &str = "Snippets/x.md";

fn j(lines: &[&str]) -> String {
    lines.join("\n")
}

fn field(content: &str, f: FmField, v: &str) -> Result<String, PatchError> {
    patch_field(content, P, f, v)
}

fn multi(index: usize, heading: &str) -> CodeTarget {
    CodeTarget::Block {
        index,
        heading: heading.into(),
    }
}

// ---- patchFrontmatterField (:23-188) ----

#[test]
fn title_replaced_others_unchanged() {
    let c = j(&[
        "---",
        "type: snippet",
        "title: Old Title",
        "description: Some description",
        "---",
        "",
        "```code",
        "const x = 1;",
        "```",
    ]);
    let want = c.replace("Old Title", "New Title");
    assert_eq!(field(&c, FmField::Title, "New Title"), Ok(want));
}

#[test]
fn description_replaced_others_unchanged() {
    let c = j(&[
        "---",
        "type: template",
        "title: My Template",
        "description: Original description",
        "language: typescript",
        "---",
    ]);
    let want = c.replace("Original description", "Updated description");
    assert_eq!(
        field(&c, FmField::Description, "Updated description"),
        Ok(want)
    );
}

// TS patches `language` here (not an FmField); Rust inserts `description` after `title`, which
// with no later key is the end of the block: same bytes as the TS expectation shape.
#[test]
fn absent_field_is_inserted_before_closing_dashes() {
    let c = j(&["---", "type: snippet", "title: My Snippet", "---"]);
    let want = j(&[
        "---",
        "type: snippet",
        "title: My Snippet",
        "description: d",
        "---",
    ]);
    assert_eq!(field(&c, FmField::Description, "d"), Ok(want));
}

// Rust-only: §1.4 order, not "before ---".
#[test]
fn absent_field_is_inserted_in_key_order() {
    let c = j(&["---", "title: T", "language: py", "tags: [a]", "---"]);
    let want = j(&[
        "---",
        "title: T",
        "description: d",
        "language: py",
        "tags: [a]",
        "---",
    ]);
    assert_eq!(field(&c, FmField::Description, "d"), Ok(want));
}

#[test]
fn multi_field_frontmatter_keeps_order() {
    let c = j(&[
        "---",
        "type: command",
        "title: Deploy",
        "description: Deploy to production",
        "language: bash",
        "tags: [deploy, prod]",
        "env: production",
        "target: terminal",
        "---",
    ]);
    let want = c.replace("title: Deploy\n", "title: Deploy to Prod\n");
    assert_eq!(field(&c, FmField::Title, "Deploy to Prod"), Ok(want));
}

// deviates_from_ts: TS wrapped the value in quotes, but the parser never unquotes, so the value
// read back with the quotes. The engine writes it as is.
#[test]
fn colon_value_is_written_unquoted() {
    let c = j(&["---", "type: snippet", "title: My Snippet", "---"]);
    let out = field(&c, FmField::Title, "https://example.com").unwrap();
    assert_eq!(
        out,
        j(&["---", "type: snippet", "title: https://example.com", "---"])
    );
    assert_eq!(
        parse_from_content(&out, P).frontmatter.title.as_deref(),
        Some("https://example.com")
    );
}

// deviates_from_ts: same reason as above.
#[test]
fn double_quote_value_is_written_unquoted() {
    let c = j(&["---", "type: snippet", "description: Original", "---"]);
    let out = field(&c, FmField::Description, "He said \"hello world\"").unwrap();
    assert_eq!(
        out,
        j(&[
            "---",
            "type: snippet",
            "description: He said \"hello world\"",
            "---"
        ])
    );
}

#[test]
fn no_frontmatter_is_refused() {
    let c = j(&["# Just a heading", "", "Some plain text."]);
    assert_eq!(
        field(&c, FmField::Title, "Anything"),
        Err(PatchError::Refused(NoFrontmatter))
    );
}

// TS patched `type` and no-opped; Rust refuses a title patch on a file not starting `---`.
#[test]
fn file_not_starting_with_dashes_is_refused() {
    let c = j(&["```code", "const x = 1;", "```"]);
    assert_eq!(
        field(&c, FmField::Title, "x"),
        Err(PatchError::Refused(NoFrontmatter))
    );
}

// ---- patchBlockCode (:506-738) ----

#[test]
fn single_replace_keeps_info_string() {
    let c = j(&[
        "---",
        "type: snippet",
        "title: Counter",
        "language: javascript",
        "---",
        "",
        "```javascript",
        "const x = 1;",
        "```",
    ]);
    let want = j(&[
        "---",
        "type: snippet",
        "title: Counter",
        "language: javascript",
        "---",
        "",
        "```javascript",
        "const y = 2;",
        "const z = 3;",
        "```",
    ]);
    let out = patch_code(&c, P, &CodeTarget::Single, "const y = 2;\nconst z = 3;");
    assert_eq!(out, Ok(want));
}

#[test]
fn single_keeps_trailing_vars_section() {
    let c = j(&[
        "---",
        "type: snippet",
        "---",
        "",
        "```code",
        "echo <VK-message>",
        "```",
        "",
        "vars:",
        "VK-message=hello",
    ]);
    let want = c.replace("echo <VK-message>", "printf <VK-message>");
    assert_eq!(
        patch_code(&c, P, &CodeTarget::Single, "printf <VK-message>"),
        Ok(want)
    );
}

#[test]
fn multi_patches_only_target() {
    let c = j(&[
        "---",
        "type: snippet",
        "title: API URLs",
        "---",
        "",
        "## Development",
        "Local dev server.",
        "```bash",
        "http://localhost:<VK-PORT>",
        "```",
        "",
        "## Production",
        "```bash",
        "https://api.example.com",
        "```",
    ]);
    let want = c.replace("https://api.example.com", "https://api.v2.example.com");
    let out = patch_code(&c, P, &multi(1, "Production"), "https://api.v2.example.com");
    assert_eq!(out, Ok(want));
}

#[test]
fn multi_keeps_per_block_vks() {
    let c = j(&[
        "---",
        "type: snippet",
        "---",
        "",
        "## Development",
        "```bash",
        "http://localhost:<VK-PORT>",
        "```",
        "",
        "### VKs:",
        "",
        "```vks",
        "VK-PORT=3000",
        "```",
        "",
        "## Production",
        "```bash",
        "https://api.example.com",
        "```",
    ]);
    let want = c.replace("http://localhost:<VK-PORT>", "http://127.0.0.1:<VK-PORT>");
    let out = patch_code(
        &c,
        P,
        &multi(0, "Development"),
        "http://127.0.0.1:<VK-PORT>",
    );
    assert_eq!(out, Ok(want));
}

#[test]
fn unknown_heading_is_block_not_found() {
    let c = j(&[
        "---",
        "type: snippet",
        "---",
        "",
        "## Development",
        "```bash",
        "echo dev",
        "```",
    ]);
    assert_eq!(
        patch_code(&c, P, &multi(0, "Nope"), "echo changed"),
        Err(PatchError::BlockNotFound(Some(0)))
    );
}

#[test]
fn round_trip_single() {
    let c = j(&[
        "---",
        "language: python",
        "---",
        "",
        "```python",
        "print(\"old\")",
        "```",
    ]);
    let new = "print(\"new\")\nprint(\"line2\")";
    let out = patch_code(&c, P, &CodeTarget::Single, new).unwrap();
    assert_eq!(parse_from_content(&out, P).code, new);
}

#[test]
fn round_trip_multi() {
    let c = j(&[
        "---",
        "---",
        "",
        "## Development",
        "```bash",
        "echo dev",
        "```",
        "",
        "## Production",
        "```bash",
        "echo prod",
        "```",
    ]);
    let out = patch_code(&c, P, &multi(1, "Production"), "echo production-v2").unwrap();
    let b = parse_from_content(&out, P).blocks;
    assert_eq!(
        (b[0].code.as_str(), b[1].code.as_str()),
        ("echo dev", "echo production-v2")
    );
}

// ---- Rust-only ----

#[test]
fn crlf_title_patch_keeps_crlf() {
    let c = "---\r\ntitle: a\r\ndescription: d\r\n---\r\n\r\n```js\r\nx\r\n```\r\n";
    let out = field(c, FmField::Title, "b").unwrap();
    assert_eq!(out, c.replace("title: a", "title: b"));
    // Insert uses the file's terminator too.
    let ins = field("---\r\ntitle: a\r\n---\r\n", FmField::Description, "d").unwrap();
    assert_eq!(ins, "---\r\ntitle: a\r\ndescription: d\r\n---\r\n");
}

#[test]
fn duplicate_title_last_wins_and_parse_sees_it() {
    let c = "---\ntitle: one\ntitle: two\n---\n";
    let out = field(c, FmField::Title, "new").unwrap();
    assert_eq!(out, "---\ntitle: one\ntitle: new\n---\n");
    assert_eq!(
        parse_from_content(&out, P).frontmatter.title.as_deref(),
        Some("new")
    );
}

#[test]
fn empty_value_removes_every_key_line() {
    let c = "---\ntitle: one\ntype: x\ntitle: two\n---\nbody";
    assert_eq!(
        field(c, FmField::Title, "  "),
        Ok("---\ntype: x\n---\nbody".into())
    );
    // Removing the last line leaves no blank line before the closing dashes.
    assert_eq!(
        field("---\ntype: x\ntitle: a\n---\n", FmField::Title, ""),
        Ok("---\ntype: x\n---\n".into())
    );
    assert_eq!(
        field("---\ntitle: a\n---\n", FmField::Title, ""),
        Ok("---\n\n---\n".into())
    );
}

#[test]
fn value_newlines_are_flattened() {
    let out = field(
        "---\ntitle: a\n---\n",
        FmField::Title,
        "x\ndescription: evil",
    )
    .unwrap();
    assert_eq!(out, "---\ntitle: x description: evil\n---\n");
}

#[test]
fn flagged_file_is_refused_by_both() {
    let c = "---\ntitle: a\n---\n%%oa:start%%\nhi\n%%oa:end%%\n";
    assert_eq!(
        field(c, FmField::Title, "b"),
        Err(PatchError::Refused(Flagged))
    );
    assert_eq!(
        patch_code(c, P, &CodeTarget::Single, "x"),
        Err(PatchError::Refused(Flagged))
    );
}

#[test]
fn code_with_backticks_is_unrepresentable() {
    let c = "---\n---\n```js\nx\n```\n";
    assert_eq!(
        patch_code(c, P, &CodeTarget::Single, "a\n```\nb"),
        Err(PatchError::Unrepresentable("code"))
    );
    let m = "## A\n```js\nx\n```\n## B\n```js\ny\n```\n";
    assert_eq!(
        patch_code(m, P, &multi(0, "A"), "q ``` w"),
        Err(PatchError::Unrepresentable("code"))
    );
}

#[test]
fn block_code_cannot_smuggle_file_level_vars() {
    let m = "## A\n```js\nx\n```\n## B\n```js\ny\n```\n";
    assert_eq!(
        patch_code(m, P, &multi(1, "B"), "vars:\nA=1"),
        Err(PatchError::Unrepresentable("code"))
    );
}

#[test]
fn variables_file_is_refused() {
    let c = "---\n---\n```js\nx\n```\n";
    assert_eq!(
        patch_code(c, "Variables/v.md", &CodeTarget::Single, "y"),
        Err(PatchError::Refused(Variables))
    );
}

#[test]
fn right_index_wrong_heading_and_bad_index() {
    let m = "## A\n```js\nx\n```\n## B\n```js\ny\n```\n";
    assert_eq!(
        patch_code(m, P, &multi(1, "A"), "q"),
        Err(PatchError::BlockNotFound(Some(1)))
    );
    assert_eq!(
        patch_code(m, P, &multi(9, "A"), "q"),
        Err(PatchError::BlockNotFound(Some(9)))
    );
}

#[test]
fn single_body_edit_on_multi_block_file() {
    let m = "## A\n```js\nx\n```\n";
    assert_eq!(
        patch_code(m, P, &CodeTarget::Single, "q"),
        Err(PatchError::BlockNotFound(None))
    );
}

#[test]
fn bare_body_template_is_no_fence() {
    let c = "---\nartifactType: Template\n---\njust text <VK-a>\n";
    assert_eq!(
        patch_code(c, "Templates/t.md", &CodeTarget::Single, "q"),
        Err(PatchError::Refused(NoFence))
    );
}

#[test]
fn vks_only_fence_is_no_fence() {
    let c = "---\nartifactType: Template\n---\n```vks\na: 1\n```\n";
    assert_eq!(
        patch_code(c, "Templates/t.md", &CodeTarget::Single, "q"),
        Err(PatchError::Refused(NoFence))
    );
    let b = "## A\n```vks\na: 1\n```\n";
    assert_eq!(
        patch_code(b, P, &multi(0, "A"), "q"),
        Err(PatchError::Refused(NoFence))
    );
}

#[test]
fn mid_line_fence_close_patches_the_parsers_span() {
    // The parser closes on the first ``` even mid-line: the span is `x`, not the next line.
    let c = "```js\nx```\nrest\n";
    assert_eq!(
        patch_code(c, P, &CodeTarget::Single, "y"),
        Ok("```js\ny\n```\nrest\n".into())
    );
}

#[test]
fn is_flagged_cases() {
    assert!(is_flagged("---\n---\n%%oa:start%%\nhi\n%%oa:end%%\n"));
    assert!(!is_flagged("```\n%%oa:start%%\nhi\n%%oa:end%%\n```\n"));
    // Flags in the frontmatter do not count: it is stripped first.
    assert!(!is_flagged("---\nnote: %%oa:start%%\n---\nbody"));
    assert!(!is_flagged(""));
}

#[test]
fn fuzz_never_panics_and_ok_outputs_reparse() {
    let strings = common::fuzz_strings(0x9A7C, 5_000, 40);
    for (i, s) in strings.iter().enumerate() {
        let other = &strings[(i + 1) % strings.len()];
        let mut outs = vec![
            patch_field(s, P, FmField::Title, other),
            patch_field(s, P, FmField::Description, other),
            patch_code(s, P, &CodeTarget::Single, other),
            patch_code(s, P, &multi(0, "A"), other),
            patch_field(other, P, FmField::Title, s),
            patch_code(other, P, &CodeTarget::Single, s),
        ];
        for o in outs.drain(..).flatten() {
            let _ = parse_from_content(&o, P);
        }
    }
}
