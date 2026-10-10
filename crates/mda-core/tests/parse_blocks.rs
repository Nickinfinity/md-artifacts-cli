//! Port of `parser-blocks.test.ts:15-429` (25 tests), through `parse_from_content(..).blocks`.

use mda_core::model::{ParsedBlock, ParsedVar};
use mda_core::parse::parse_from_content;

fn j(lines: &[&str]) -> String {
    lines.join("\n")
}

fn blocks(content: &str) -> Vec<ParsedBlock> {
    parse_from_content(content, "Snippets/x.md").blocks
}

fn vars(pairs: &[(&str, &str)]) -> Vec<ParsedVar> {
    pairs.iter().map(|(k, v)| ParsedVar::text(*k, *v)).collect()
}

#[test]
fn single_heading_and_fence_returns_a_one_element_array() {
    let b = blocks(&j(&[
        "---",
        "type: snippet",
        "---",
        "",
        "## Greeting",
        "```bash",
        "echo hello",
        "```",
    ]));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].heading, "Greeting");
    assert_eq!(b[0].code, "echo hello");
}

#[test]
fn multiple_headings_are_returned_in_document_order() {
    let b = blocks(&j(&[
        "## Alpha", "```js", "a();", "```", "", "## Beta", "```js", "b();", "```", "", "## Gamma",
        "```js", "c();", "```",
    ]));
    let h: Vec<_> = b.iter().map(|b| b.heading.as_str()).collect();
    assert_eq!(h, ["Alpha", "Beta", "Gamma"]);
}

#[test]
fn description_is_the_text_between_the_heading_line_and_the_opening_fence() {
    let b = blocks(&j(&[
        "## Deploy",
        "Deploys the current build to the target environment.",
        "```bash",
        "npm run deploy",
        "```",
    ]));
    assert_eq!(
        b[0].description,
        "Deploys the current build to the target environment."
    );
}

#[test]
fn heading_immediately_followed_by_a_fence_has_an_empty_description() {
    let b = blocks(&j(&["## Silent", "```bash", "true", "```"]));
    assert_eq!(b[0].description, "");
}

#[test]
fn fence_language_tag_is_captured_in_fence_lang() {
    let b = blocks(&j(&["## Module", "```javascript", "const x = 1;", "```"]));
    assert_eq!(b[0].fence_lang.as_deref(), Some("javascript"));
}

#[test]
fn fence_with_no_language_tag_leaves_fence_lang_none() {
    let b = blocks(&j(&["## Raw", "```", "raw content", "```"]));
    assert_eq!(b[0].fence_lang, None);
}

#[test]
fn multi_line_code_block_is_captured_in_full_with_internal_newlines_preserved() {
    let b = blocks(&j(&[
        "## Script",
        "```bash",
        "line one",
        "line two",
        "line three",
        "```",
    ]));
    assert_eq!(b[0].code, "line one\nline two\nline three");
}

#[test]
fn block_with_no_vk_tokens_has_no_vars() {
    let b = blocks(&j(&["## Static", "```bash", "echo done", "```"]));
    assert!(b[0].vars.is_empty());
}

#[test]
fn vk_tokens_in_block_code_are_extracted_as_vars_in_order_of_appearance() {
    let b = blocks(&j(&[
        "## Endpoint",
        "```bash",
        "curl <VK-host>/<VK-path>",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-host", ""), ("VK-path", "")]));
}

#[test]
fn repeated_vk_token_within_one_block_is_deduplicated() {
    let b = blocks(&j(&[
        "## Repeated",
        "```bash",
        "echo <VK-env> && echo <VK-env>",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-env", "")]));
}

#[test]
fn same_var_name_in_two_blocks_is_listed_independently_in_each_block() {
    let b = blocks(&j(&[
        "## Dev",
        "```bash",
        "http://<VK-host>/dev",
        "```",
        "",
        "## Prod",
        "```bash",
        "https://<VK-host>/prod",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-host", "")]));
    assert_eq!(b[1].vars, vars(&[("VK-host", "")]));
}

#[test]
fn vars_are_scoped_to_their_block_adjacent_blocks_do_not_bleed() {
    let b = blocks(&j(&[
        "## WithVar",
        "```bash",
        "echo <VK-name>",
        "```",
        "",
        "## NoVar",
        "```bash",
        "echo static",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-name", "")]));
    assert!(b[1].vars.is_empty());
}

#[test]
fn content_with_no_headings_returns_empty() {
    assert!(
        blocks(&j(&[
            "---",
            "type: snippet",
            "---",
            "",
            "```bash",
            "echo hello",
            "```"
        ]))
        .is_empty()
    );
}

#[test]
fn frontmatter_is_stripped_and_not_parsed_as_a_block() {
    let b = blocks(&j(&[
        "---",
        "type: snippet",
        "title: Only One Block",
        "---",
        "",
        "## First Block",
        "```bash",
        "echo hi",
        "```",
    ]));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].heading, "First Block");
}

#[test]
fn empty_string_returns_empty() {
    assert!(blocks("").is_empty());
}

#[test]
fn vks_fence_parses_key_value_pairs_into_block_vars() {
    let b = blocks(&j(&[
        "## Local Dev",
        "```vks",
        "VK-host=localhost",
        "VK-port=3000",
        "```",
    ]));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].fence_lang.as_deref(), Some("vks"));
    assert_eq!(
        b[0].vars,
        vars(&[("VK-host", "localhost"), ("VK-port", "3000")])
    );
}

#[test]
fn vks_fence_with_empty_value_yields_empty_default() {
    let b = blocks(&j(&["## Empty Defaults", "```vks", "VK-token=", "```"]));
    assert_eq!(b[0].vars, vars(&[("VK-token", "")]));
}

#[test]
fn multi_block_variable_file_each_vks_fence_yields_its_own_vars() {
    let b = blocks(&j(&[
        "## Dev",
        "```vks",
        "VK-host=localhost",
        "```",
        "",
        "## Prod",
        "```vks",
        "VK-host=prod.example.com",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-host", "localhost")]));
    assert_eq!(b[1].vars, vars(&[("VK-host", "prod.example.com")]));
}

#[test]
fn vks_fence_after_a_code_fence_applies_defaults_to_detected_vars() {
    let code = "const <VK-result> = <VK-array>.filter(i => i.<VK-prop> === <VK-value>);";
    let b = blocks(&j(&[
        "## Filter",
        "Filter items.",
        "```javascript",
        code,
        "```",
        "```vks",
        "VK-result=filtered",
        "VK-array=items",
        "VK-prop=status",
        "VK-value=\"active\"",
        "```",
    ]));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].fence_lang.as_deref(), Some("javascript"));
    assert_eq!(b[0].code, code);
    assert_eq!(b[0].description, "Filter items.");
    assert_eq!(
        b[0].vars,
        vars(&[
            ("VK-result", "filtered"),
            ("VK-array", "items"),
            ("VK-prop", "status"),
            ("VK-value", "\"active\"")
        ])
    );
}

#[test]
fn detected_var_with_no_matching_vks_line_keeps_empty_default() {
    let b = blocks(&j(&[
        "## Partial",
        "```javascript",
        "const <VK-a> = <VK-b>;",
        "```",
        "```vks",
        "VK-a=1",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-a", "1"), ("VK-b", "")]));
}

#[test]
fn vks_line_with_no_matching_code_token_is_appended_after_detected_vars() {
    let b = blocks(&j(&[
        "## Extra",
        "```javascript",
        "const <VK-a> = 1;",
        "```",
        "```vks",
        "VK-a=10",
        "VK-unused=99",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-a", "10"), ("VK-unused", "99")]));
}

#[test]
fn code_plus_vars_fence_pairs_are_scoped_per_block() {
    let b = blocks(&j(&[
        "## One",
        "```javascript",
        "f(<VK-x>);",
        "```",
        "```vks",
        "VK-x=alpha",
        "```",
        "",
        "## Two",
        "```javascript",
        "g(<VK-x>);",
        "```",
        "```vks",
        "VK-x=beta",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-x", "alpha")]));
    assert_eq!(b[1].vars, vars(&[("VK-x", "beta")]));
    assert_eq!(b[0].code, "f(<VK-x>);");
    assert_eq!(b[1].code, "g(<VK-x>);");
}

#[test]
fn arbitrary_marker_text_between_code_fence_and_vks_fence_is_ignored() {
    let b = blocks(&j(&[
        "## Group",
        "Bucket an array.",
        "```javascript",
        "const <VK-result> = <VK-array>.reduce(acc => acc, {});",
        "```",
        "",
        "### VKs:",
        "",
        "```vks",
        "VK-result=grouped",
        "VK-array=items",
        "```",
    ]));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].heading, "Group");
    assert_eq!(b[0].description, "Bucket an array.");
    assert_eq!(
        b[0].code,
        "const <VK-result> = <VK-array>.reduce(acc => acc, {});"
    );
    assert_eq!(
        b[0].vars,
        vars(&[("VK-result", "grouped"), ("VK-array", "items")])
    );
}

#[test]
fn marker_text_plus_vks_scoped_per_block_across_two_sections() {
    let b = blocks(&j(&[
        "## One",
        "```javascript",
        "f(<VK-x>);",
        "```",
        "whatever marker",
        "```vks",
        "VK-x=alpha",
        "```",
        "",
        "## Two",
        "```javascript",
        "g(<VK-x>);",
        "```",
        "--- notes ---",
        "```vks",
        "VK-x=beta",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-x", "alpha")]));
    assert_eq!(b[1].vars, vars(&[("VK-x", "beta")]));
}

#[test]
fn code_fence_with_no_trailing_vks_fence_still_yields_empty_default_vars() {
    let b = blocks(&j(&[
        "## NoDefaults",
        "```javascript",
        "const <VK-z> = 1;",
        "```",
    ]));
    assert_eq!(b[0].vars, vars(&[("VK-z", "")]));
}
