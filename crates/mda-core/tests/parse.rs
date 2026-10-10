//! Ports of `parser-vars` (`detect_vars`), `parser-type-from-dir`, `agent-parse`, the bare-body and
//! additive suites of `flags-parse`, the §6 quirk tests and the fuzz run.

mod common;

use mda_core::model::{ParsedArtifact, ParsedVar};
use mda_core::parse::tokens::detect_vars;
use mda_core::parse::{decode, parse_from_content};
use mda_core::registry::ArtifactType;

fn p(content: &str, path: &str) -> ParsedArtifact {
    parse_from_content(content, path)
}

fn vars(pairs: &[(&str, &str)]) -> Vec<ParsedVar> {
    pairs.iter().map(|(k, v)| ParsedVar::text(*k, *v)).collect()
}

fn names(code: &str) -> Vec<String> {
    detect_vars(code).into_iter().map(|v| v.name).collect()
}

// ---- the first assertion of the plan ----

#[test]
fn single_block_command_file() {
    let a = p(
        "---\nartifactType: Command\ntitle: T\n---\n\n```bash\nls <VK-dir>\n```\n",
        "Commands/sub/a.md",
    );
    assert_eq!(a.file_name, "a");
    assert_eq!(a.relative_path, "sub/a.md");
    assert_eq!(a.frontmatter.title.as_deref(), Some("T"));
    assert_eq!(a.code, "ls <VK-dir>");
    assert_eq!(a.frontmatter.language.as_deref(), Some("bash"));
    assert!(a.vars.is_empty());
}

#[test]
fn file_name_and_relative_path_edges() {
    assert_eq!(p("", "Snippets/.md").file_name, ".md");
    assert_eq!(p("", "Snippets/a.md").relative_path, "a.md");
    assert_eq!(p("", "a.md").relative_path, "");
}

// ---- parser-vars.test.ts:18-117 (16) ----

#[test]
fn single_var_is_one_parsed_var() {
    assert_eq!(detect_vars("<VK-items>"), vars(&[("VK-items", "")]));
}

#[test]
fn multiple_distinct_vars_in_first_appearance_order() {
    assert_eq!(
        names("<VK-host> <VK-port> <VK-path>"),
        ["VK-host", "VK-port", "VK-path"]
    );
}

#[test]
fn repeated_var_is_deduplicated() {
    assert_eq!(
        names("<VK-items> and <VK-items> and <VK-items>"),
        ["VK-items"]
    );
}

#[test]
fn camel_case_hint_is_valid() {
    assert_eq!(names("<VK-myVar>"), ["VK-myVar"]);
}

#[test]
fn upper_snake_case_hint_is_valid() {
    assert_eq!(names("<VK-MY_VAR>"), ["VK-MY_VAR"]);
}

#[test]
fn pascal_case_hint_is_valid() {
    assert_eq!(names("<VK-MyComponent>"), ["VK-MyComponent"]);
}

#[test]
fn mixed_casing_vars_all_extracted() {
    assert_eq!(
        names("<VK-myVar> <VK-MY_VAR> <VK-MyComponent>"),
        ["VK-myVar", "VK-MY_VAR", "VK-MyComponent"]
    );
}

#[test]
fn vars_embedded_in_real_js_code() {
    let code = "const x = <VK-items>.filter(i => i.<VK-prop> === <VK-value>)";
    assert_eq!(names(code), ["VK-items", "VK-prop", "VK-value"]);
}

#[test]
fn html_tags_produce_no_matches() {
    assert!(names("<div><span>Hello</span></div>").is_empty());
}

#[test]
fn vue_components_produce_no_matches() {
    assert!(names("<v-btn @click=\"go\"><v-card></v-card></v-btn>").is_empty());
}

#[test]
fn typescript_generics_produce_no_matches() {
    assert!(names("const a: Array<string> = []; const m: Map<K,V> = new Map();").is_empty());
}

#[test]
fn jsx_component_produces_no_match() {
    assert!(names("<MyComponent prop=\"val\" />").is_empty());
}

#[test]
fn handlebars_placeholders_produce_no_matches() {
    assert!(names("Hello {{name}}, your order {{orderId}} is ready.").is_empty());
}

#[test]
fn vk_without_hyphen_or_hint_produces_no_match() {
    assert!(names("<VK>").is_empty());
}

#[test]
fn vk_with_empty_hint_produces_no_match() {
    assert!(names("<VK->").is_empty());
}

#[test]
fn empty_string_returns_empty() {
    assert!(detect_vars("").is_empty());
}

// ---- Rust-only: the §4.1 + §10 grammar ----

#[test]
fn closing_form_is_the_same_variable() {
    assert_eq!(names("<VK-a></VK-a> </VK-b>"), ["VK-a", "VK-b"]);
}

#[test]
fn path_loop_and_join_tokens_detect_their_root() {
    assert_eq!(names("<VK-users.name>"), ["VK-users"]);
    assert_eq!(names("<VK-each:users:\", \">x<VK-end:users>"), ["VK-users"]);
    assert_eq!(names("<VK-join:tags:\" | \">"), ["VK-tags"]);
    assert_eq!(
        names("<VK-each:users.tags><VK-end:users.tags><VK-join:a.b.c>"),
        ["VK-users", "VK-a"]
    );
}

#[test]
fn each_without_a_colon_is_a_plain_variable() {
    assert_eq!(names("<VK-each>"), ["VK-each"]);
}

#[test]
fn separator_with_gt_detects_nothing() {
    assert!(names("<VK-each:a:\"a>b\">").is_empty());
}

#[test]
fn separator_escapes_are_accepted() {
    assert_eq!(names(r#"<VK-join:a:"\n\t\"\\">"#), ["VK-a"]);
}

#[test]
fn non_ascii_hint_chars_do_not_extend_a_token() {
    assert!(names("<VK-é>").is_empty());
    assert_eq!(names("<VK-aé>"), Vec::<String>::new());
}

// ---- parser-type-from-dir.test.ts:27-53 (5) ----

const NO_FM: &str = "## Change url (remote)\n\n```bash\ngit remote set-url origin \"new_url\"\n```";

#[test]
fn commands_file_without_frontmatter_is_a_command() {
    assert_eq!(
        p(NO_FM, "Commands/GIT/Remote.md").frontmatter.artifact_type,
        ArtifactType::Command
    );
}

#[test]
fn blocks_inherit_the_directory_type() {
    let a = p(NO_FM, "Commands/GIT/Remote.md");
    assert!(!a.blocks.is_empty());
    assert_eq!(a.frontmatter.artifact_type, ArtifactType::Command);
}

#[test]
fn snippets_file_without_frontmatter_is_a_snippet() {
    assert_eq!(
        p(NO_FM, "Snippets/x.md").frontmatter.artifact_type,
        ArtifactType::Snippet
    );
}

#[test]
fn explicit_frontmatter_wins_over_the_directory() {
    let c = format!("---\nartifactType: Snippet\n---\n\n{NO_FM}");
    assert_eq!(
        p(&c, "Commands/x.md").frontmatter.artifact_type,
        ArtifactType::Snippet
    );
}

#[test]
fn unrecognised_directory_falls_back_to_snippet() {
    assert_eq!(
        p(NO_FM, "Other/x.md").frontmatter.artifact_type,
        ArtifactType::Snippet
    );
}

// ---- agent-parse.test.ts:26-41 (2) ----

#[test]
fn reads_provider_model_version_off_an_agent_file() {
    let md = "---\nartifactType: AIAgentsConfig\ntitle: Code reviewer\nprovider: Claude\nmodel: Opus\nversion: 4.8\n---\n\n```md\nYou are a reviewer.\n```\n";
    let f = p(md, "AIAgentsConf/reviewer.md").frontmatter;
    assert_eq!(f.artifact_type, ArtifactType::AIAgentsConfig);
    assert_eq!(f.provider.as_deref(), Some("Claude"));
    assert_eq!(f.model.as_deref(), Some("Opus"));
    assert_eq!(f.version.as_deref(), Some("4.8"));
}

#[test]
fn leaves_the_keys_none_when_the_file_omits_them() {
    let md = "---\nartifactType: AIAgentsConfig\ntitle: Bare\n---\n\n```md\nhi\n```\n";
    let f = p(md, "AIAgentsConf/bare.md").frontmatter;
    assert_eq!((f.provider, f.model, f.version), (None, None, None));
}

#[test]
fn frontmatter_value_kinds() {
    let md = "---\ntags: [a, b,, c ]\npaths: x\nindex: true\nenv: Dev\ntarget:\nbogus: 1\n---\n";
    let f = p(md, "Templates/t.md").frontmatter;
    assert_eq!(f.tags, Some(vec!["a".into(), "b".into(), "c".into()]));
    assert_eq!(f.paths, Some(vec!["x".into()]));
    assert_eq!(f.index, Some(true));
    assert_eq!(f.env.as_deref(), Some("Dev"));
    assert_eq!(f.target.as_deref(), Some(""));
    assert_eq!(
        p("---\nindex: True\n---\n", "Templates/t.md")
            .frontmatter
            .index,
        Some(false)
    );
}

// ---- flags-parse.test.ts:164-199 + 213-244 (9), resolveOutputFileName asserts dropped ----

const AGENT: &str = "AIAgentsConf/reviewer.md";

#[test]
fn bare_markdown_agent_note_is_the_payload() {
    let c = "---\nartifactType: AIAgentsConfig\ntitle: Reviewer\ntarget: CLAUDE.md\n---\n\n# Reviewer\n\nBe terse with <VK-repo_name>.";
    let a = p(c, AGENT);
    assert_eq!(a.code, "# Reviewer\n\nBe terse with <VK-repo_name>.");
    assert_eq!(a.frontmatter.language.as_deref(), Some("markdown"));
    assert!(a.blocks.is_empty());
    assert_eq!(a.frontmatter.target.as_deref(), Some("CLAUDE.md"));
}

#[test]
fn vars_section_is_not_written_into_the_payload() {
    let c = "---\nartifactType: AIAgentsConfig\n---\nBe terse with <VK-repo_name>.\n\nvars:\n```vks\nVK-repo_name=my-app\n```\n";
    let a = p(c, AGENT);
    assert_eq!(a.code, "Be terse with <VK-repo_name>.");
    assert_eq!(a.vars, vars(&[("VK-repo_name", "my-app")]));
}

#[test]
fn flag_less_note_keeps_its_triple_star() {
    let c = "---\nartifactType: AIAgentsConfig\n---\nintro\n\n***\n\noutro";
    assert_eq!(p(c, AGENT).code, "intro\n\n***\n\noutro");
}

#[test]
fn template_with_no_fence_takes_the_whole_body() {
    let c = "---\nartifactType: Template\ntitle: Readme\n---\n# <VK-project_name>\n\nDocs go here.";
    assert_eq!(
        p(c, "Templates/readme.md").code,
        "# <VK-project_name>\n\nDocs go here."
    );
}

#[test]
fn existing_code_fence_wins_over_the_bare_body() {
    let c = "---\nartifactType: AIAgentsConfig\n---\nPreamble prose.\n\n```md\nfenced payload\n```\n\nTrailing prose.";
    assert_eq!(p(c, AGENT).code, "fenced payload");
}

#[test]
fn snippet_with_no_fence_does_not_take_the_bare_body() {
    let c = "---\nartifactType: Snippet\n---\nJust some prose in a note.";
    assert_eq!(p(c, "Snippets/x.md").code, "");
}

#[test]
fn classic_fenced_single_block_file_parses_as_before() {
    let c = "---\nartifactType: Snippet\nlanguage: javascript\n---\n\n```javascript\nconst x = <VK-name>;\n```\n\nvars:\nVK-name=hi\n";
    let a = p(c, "Snippets/x.md");
    assert_eq!(a.code, "const x = <VK-name>;");
    assert_eq!(a.frontmatter.language.as_deref(), Some("javascript"));
    assert_eq!(a.vars, vars(&[("VK-name", "hi")]));
    assert!(a.blocks.is_empty());
}

#[test]
fn legacy_unfenced_vars_section_parses_in_every_spacing() {
    let vars_of = |s: &str| {
        let c = format!("---\nartifactType: Snippet\n---\n\n```js\nx = <VK-a>;\n```\n\n{s}");
        p(&c, "Snippets/x.md").vars
    };
    let expected = vars(&[("VK-a", "1")]);
    assert_eq!(vars_of("vars:\nVK-a=1\n"), expected, "tight");
    assert_eq!(
        vars_of("vars:\n\nVK-a=1\n"),
        expected,
        "blank line after the label"
    );
    assert_eq!(vars_of("vars:\nVK-a=1"), expected, "no trailing newline");
}

#[test]
fn classic_hash_heading_file_still_splits_on_headings() {
    let c =
        "---\nartifactType: Snippet\n---\n## Dev\n```bash\ndev\n```\n## Prod\n```bash\nprod\n```";
    let h: Vec<_> = p(c, "Snippets/x.md")
        .blocks
        .into_iter()
        .map(|b| b.heading)
        .collect();
    assert_eq!(h, ["Dev", "Prod"]);
}

// ---- one named test per §6 TS quirk ----

#[test]
fn quirk_single_block_top_vars_are_the_fence_only() {
    let c = "```js\nf(<VK-a>);\n```\n\nvars:\nVK-b=2\n";
    assert_eq!(p(c, "Snippets/x.md").vars, vars(&[("VK-b", "2")]));
}

#[test]
fn quirk_first_vks_fence_anywhere_even_inside_a_block_is_file_level() {
    let c = "## A\n```js\nf(<VK-x>);\n```\n```vks\nVK-x=1\n```\n";
    let a = p(c, "Snippets/x.md");
    assert_eq!(a.vars, vars(&[("VK-x", "1")]));
    assert_eq!(a.blocks[0].vars, vars(&[("VK-x", "1")]));
}

#[test]
fn quirk_failing_first_vks_fence_inside_a_block_errors_file_and_block() {
    let c = "## A\n```js\nf(<VK-x>);\n```\n```vks\nVK-x: 'unclosed\n```\n";
    let a = p(c, "Snippets/x.md");
    for (what, e) in [("file", &a.vars_error), ("block", &a.blocks[0].vars_error)] {
        let e = e.as_ref().unwrap_or_else(|| panic!("{what} error missing"));
        assert_eq!(e.code, "vks.syntax", "{what}");
        assert_eq!(e.params["rule"], "unclosed_quote", "{what}");
        assert_eq!(e.params["line"], "1", "{what}");
    }
    assert_eq!(a.blocks[0].vars, vars(&[("VK-x", "")]));
}

/// `regions` flagged regions, each using `<VK-a>`, then a legacy fence of `defaults` entries.
fn flagged_file(regions: usize, defaults: usize) -> String {
    let mut c = String::from("---\nartifactType: AIAgentsConfig\n---\n");
    for i in 0..regions {
        c.push_str(&format!("%%oa:start r{i}%%\nx <VK-a>\n%%oa:end%%\n"));
    }
    c.push_str("```vks\n");
    for i in 0..defaults {
        c.push_str(&format!("VK-d{i}=v\n"));
    }
    c.push_str("```\n");
    c
}

#[test]
fn sec_flagged_regions_times_defaults_over_the_cap_is_refused() {
    let c = flagged_file(400, 300); // 120_000 > MAX_BLOCK_DEFAULTS
    let a = p(&c, AGENT);
    let e = a.vars_error.as_ref().expect("limit error on the artifact");
    assert_eq!(e.code, "vks.limit");
    assert_eq!(e.params["limit"], "block_defaults");
    assert_eq!(e.params["max"], "100000");
    assert_eq!(e.params["line"], "0");
    assert_eq!(a.vars, vars(&[("VK-a", "")]));
    assert_eq!(a.blocks.len(), 400);
    for b in &a.blocks {
        assert_eq!(b.vars, vars(&[("VK-a", "")]));
        assert!(b.vars_error.is_none());
    }
}

#[test]
fn sec_flagged_product_exactly_at_the_cap_keeps_the_overlay() {
    let a = p(&flagged_file(400, 250), AGENT); // 100_000 == MAX_BLOCK_DEFAULTS
    assert!(a.vars_error.is_none());
    assert_eq!(a.blocks.len(), 400);
    assert_eq!(a.blocks[0].vars.len(), 251);
}

#[test]
fn quirk_variables_file_code_is_the_vks_body_and_language_vks() {
    let a = p(
        "---\nartifactType: Variables\n---\n\n```vks\nVK-a=1\n```\n",
        "Variables/v.md",
    );
    assert_eq!(a.code, "VK-a=1");
    assert_eq!(a.frontmatter.language.as_deref(), Some("vks"));
    assert_eq!(a.vars, vars(&[("VK-a", "1")]));
}

#[test]
fn quirk_fence_closes_on_the_first_triple_backtick_mid_line() {
    assert_eq!(p("```js\nabc```def\n```\n", "Snippets/x.md").code, "abc");
}

#[test]
fn quirk_c_plus_plus_fence_is_skipped() {
    let a = p("```c++\nx\n```\n", "Snippets/x.md");
    assert_eq!(a.code, "");
    assert_eq!(a.frontmatter.language, None);
}

#[test]
fn quirk_closing_dashes_followed_by_text_still_close_frontmatter() {
    let a = p("---\ntitle: T\n---x\n```js\nq\n```", "Snippets/x.md");
    assert_eq!(a.frontmatter.title.as_deref(), Some("T"));
    assert_eq!(a.code, "q");
}

#[test]
fn quirk_dashes_only_is_not_frontmatter() {
    let a = p("---\n---\n```js\nq\n```", "Commands/x.md");
    assert_eq!(a.frontmatter.title, None);
    assert_eq!(a.frontmatter.artifact_type, ArtifactType::Command);
    assert_eq!(a.code, "q");
}

#[test]
fn quirk_crlf_code_keeps_inner_carriage_returns() {
    assert_eq!(p("```js\r\na\r\nb\r\n```", "Snippets/x.md").code, "a\r\nb");
}

#[test]
fn quirk_legacy_duplicates_are_appended_twice() {
    let c = "## A\n```js\nf(<VK-x>);\n```\n```vks\nVK-z=1\nVK-z=2\nVK-x=9\n```\n";
    let b = &p(c, "Snippets/x.md").blocks[0];
    assert_eq!(b.vars, vars(&[("VK-x", "9"), ("VK-z", "1"), ("VK-z", "2")]));
}

#[test]
fn quirk_hash_hash_with_empty_text_is_dropped() {
    let b = p("## \n```js\nx\n```\n## B\n```js\ny\n```", "Snippets/x.md").blocks;
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].heading, "B");
}

#[test]
fn quirk_type_key_is_ignored_and_lowercase_artifact_type_falls_back_to_dir() {
    let a = p("---\ntype: snippet\n---\n", "Commands/x.md");
    assert_eq!(a.frontmatter.artifact_type, ArtifactType::Command);
    let a = p("---\nartifactType: snippet\n---\n", "Commands/x.md");
    assert_eq!(a.frontmatter.artifact_type, ArtifactType::Command);
}

#[test]
fn quirk_lone_cr_and_unicode_separators_start_a_heading() {
    let c = "intro\r## A\n```js\na\n```\u{2028}## B\n```js\nb\n```";
    let h: Vec<_> = p(c, "Snippets/x.md")
        .blocks
        .into_iter()
        .map(|b| b.heading)
        .collect();
    assert_eq!(h, ["A", "B"]);
}

#[test]
fn quirk_heading_stops_at_a_lone_cr() {
    let b = p("## A\r```js\nx\n```", "Snippets/x.md").blocks;
    assert_eq!(b[0].heading, "A");
    assert_eq!(b[0].code, "x");
    // The first `\n` lies past the fence start, so JS `slice(b < a)` is "".
    assert_eq!(b[0].description, "");
}

// ---- fuzz: no panic ----

#[test]
fn fuzz_strings_never_panic() {
    for s in common::fuzz_strings(0xC0FFEE, 10_000, 60) {
        let _ = parse_from_content(&s, "Snippets/f.md");
        let _ = parse_from_content(&s, "AIAgentsConf/f.md");
    }
}

#[test]
fn fuzz_bytes_never_panic() {
    for b in common::fuzz_bytes(0xC0FFEE, 10_000, 120) {
        let _ = parse_from_content(&decode(&b), "Templates/f.md");
    }
}
