//! Port of `artifact-serializer.test.ts` (Group A direct emit, Group B round trips, YAML safety,
//! empty sub-set), `agent-serialize.test.ts`, plus the W-11 round-trip guard.
//! Cases with vars assert against `emit_body` (the one vks writer); byte goldens are conformance.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)] // reason: assertion helpers outside #[test] fns; a panic is the failure signal

mod common;

use mda_core::error::FieldReason;
use mda_core::model::{ArtifactModel, ParsedArtifact};
use mda_core::parse::parse_from_content;
use mda_core::serialize::{FRONTMATTER_KEY_ORDER, SerializeError, from_parsed, serialize};
use mda_core::vks::emit_body;
use serde_json::{Value, json};

const MAX: usize = 1 << 20;

fn model(v: Value) -> ArtifactModel {
    serde_json::from_value(v).unwrap()
}

fn ser(v: Value) -> String {
    serialize(&model(v), MAX).unwrap()
}

fn refused(v: Value) -> (String, FieldReason) {
    match serialize(&model(v), MAX) {
        Err(SerializeError::Field { field, reason }) => (field, reason),
        other => panic!("expected Field, got {other:?}"),
    }
}

fn body(m: &ArtifactModel, block: usize) -> String {
    emit_body(&m.blocks[block].vars).unwrap()
}

fn j(lines: &[&str]) -> String {
    lines.join("\n")
}

// ── Group A ──────────────────────────────────────────────────────────────────

#[test]
fn snippet_minimal_is_byte_exact() {
    let out = ser(json!({"artifactType":"Snippet","title":"Minimal Snippet",
        "description":"Smallest valid snippet.",
        "blocks":[{"language":"javascript","code":"console.log('hello');"}]}));
    assert_eq!(
        out,
        j(&[
            "---",
            "artifactType: Snippet",
            "title: Minimal Snippet",
            "description: Smallest valid snippet.",
            "language: javascript",
            "---",
            "",
            "```javascript",
            "console.log('hello');",
            "```",
            ""
        ])
    );
}

#[test]
fn snippet_plain_text_omits_language_and_emits_a_bare_fence() {
    let out = ser(
        json!({"artifactType":"Snippet","title":"Plain Text Snippet",
        "description":"Plain text with no language field and a bare code fence.",
        "blocks":[{"code":"Some plain text content.\nNo language highlight."}]}),
    );
    assert_eq!(
        out,
        j(&[
            "---",
            "artifactType: Snippet",
            "title: Plain Text Snippet",
            "description: Plain text with no language field and a bare code fence.",
            "---",
            "",
            "```",
            "Some plain text content.",
            "No language highlight.",
            "```",
            ""
        ])
    );
}

#[test]
fn quoted_default_emits_tags_and_a_vars_fence() {
    let m = model(
        json!({"artifactType":"Snippet","title":"Quoted Default Snippet",
        "description":"Tests verbatim default preservation with quoted strings.",
        "tags":["ui","state"],
        "blocks":[{"language":"javascript","code":"element.className = <VK-x>;",
            "vars":[{"name":"VK-x","value":"\"active\""}]}]}),
    );
    let want = format!(
        "{}\n```vks\n{}```\n",
        j(&[
            "---",
            "artifactType: Snippet",
            "title: Quoted Default Snippet",
            "description: Tests verbatim default preservation with quoted strings.",
            "language: javascript",
            "tags: [ui, state]",
            "---",
            "",
            "```javascript",
            "element.className = <VK-x>;",
            "```",
            "",
            "vars:"
        ]),
        body(&m, 0)
    );
    assert_eq!(serialize(&m, MAX).unwrap(), want);
}

#[test]
fn command_locked_language_is_bash_regardless_of_the_block() {
    let m = model(json!({"artifactType":"Command","title":"Deploy Command",
        "description":"Deploy to a named environment and region.","tags":["deploy"],
        "blocks":[{"language":"","code":"./deploy.sh <VK-env> <VK-region>",
            "vars":[{"name":"VK-env","value":"staging"},{"name":"VK-region","value":"us-east-1"}]}]}));
    let want = format!(
        "{}\n```vks\n{}```\n",
        j(&[
            "---",
            "artifactType: Command",
            "title: Deploy Command",
            "description: Deploy to a named environment and region.",
            "language: bash",
            "tags: [deploy]",
            "---",
            "",
            "```bash",
            "./deploy.sh <VK-env> <VK-region>",
            "```",
            "",
            "vars:"
        ]),
        body(&m, 0)
    );
    assert_eq!(serialize(&m, MAX).unwrap(), want);
}

#[test]
fn multi_block_has_no_top_level_language_or_fence() {
    let m = model(json!({"artifactType":"Snippet","title":"API URLs",
        "description":"Development and production API base URLs.","tags":["api","urls"],
        "blocks":[
          {"heading":"Development","description":"Local development server URL.","language":"javascript",
           "code":"const baseUrl = 'http://localhost:<VK-PORT>';","vars":[{"name":"VK-PORT","value":"3000"}]},
          {"heading":"Production","description":"Production API base URL.","language":"javascript",
           "code":"const baseUrl = 'https://api.<VK-DOMAIN>';","vars":[{"name":"VK-DOMAIN","value":"example.com"}]}]}));
    let head = j(&[
        "---",
        "artifactType: Snippet",
        "title: API URLs",
        "description: Development and production API base URLs.",
        "tags: [api, urls]",
        "---",
        "",
        "## Development",
        "Local development server URL.",
        "",
        "```javascript",
        "const baseUrl = 'http://localhost:<VK-PORT>';",
        "```",
        "",
        "vars:",
        "```vks",
        "",
    ]);
    let mid = format!(
        "{}```\n\n## Production\nProduction API base URL.\n\n```javascript\nconst baseUrl = 'https://api.<VK-DOMAIN>';\n```\n\nvars:\n```vks\n",
        body(&m, 0)
    );
    let want = format!("{head}{mid}{}```\n", body(&m, 1));
    assert_eq!(serialize(&m, MAX).unwrap(), want);
}

#[test]
fn orphan_default_is_emitted() {
    let m = model(
        json!({"artifactType":"Snippet","title":"Orphan Default Snippet",
        "blocks":[{"language":"javascript","code":"console.log('no tokens here');",
            "vars":[{"name":"VK-extra","value":"hello"}]}]}),
    );
    let out = serialize(&m, MAX).unwrap();
    assert!(out.ends_with(&format!("```\n\nvars:\n```vks\n{}```\n", body(&m, 0))));
}

#[test]
fn key_order_is_pinned() {
    assert_eq!(
        FRONTMATTER_KEY_ORDER,
        [
            "artifactType",
            "title",
            "description",
            "language",
            "extension",
            "provider",
            "model",
            "version",
            "tags",
            "env",
            "target"
        ]
    );
}

// ── Group B ──────────────────────────────────────────────────────────────────

const MINIMAL: &str = "---\nartifactType: Snippet\ntitle: Minimal Snippet\ndescription: Smallest valid snippet.\nlanguage: javascript\n---\n<!-- c -->\n\n```javascript\nconsole.log('hello');\n```\n";
const QUOTED: &str = "---\nartifactType: Snippet\ntitle: Q\nlanguage: javascript\ntags: [ui, state]\n---\n\n```javascript\nelement.className = <VK-x>;\n```\n\nvars:\n```vks\nVK-x=\"active\"\n```\n";
const PLAIN: &str = "---\nartifactType: Snippet\ntitle: P\n---\n\n```\nSome plain text content.\nNo language highlight.\n```\n";
const COMMAND: &str = "---\nartifactType: Command\ntitle: Deploy Command\ntags: [deploy]\n---\n\n```\n./deploy.sh <VK-env> <VK-region>\n```\n\nvars:\n```vks\nVK-env=staging\nVK-region=us-east-1\n```\n";
const MULTI: &str = "---\nartifactType: Snippet\ntitle: API URLs\ntags: [api, urls]\n---\n\n## Development\nLocal development server URL.\n\n```javascript\nconst baseUrl = 'http://localhost:<VK-PORT>';\n```\n\n### VKs:\n\n```vks\nVK-PORT=3000\n```\n\n## Production\nProduction API base URL.\n\n```javascript\nconst baseUrl = 'https://api.<VK-DOMAIN>';\n```\n\n### VKs:\n\n```vks\nVK-DOMAIN=example.com\n```\n";
const ORPHAN: &str = "---\nartifactType: Snippet\ntitle: O\nlanguage: javascript\n---\n\n```javascript\nconsole.log('no tokens here');\n```\n\nvars:\n```vks\nVK-extra=hello\n```\n";

fn round_trip(src: &str, path: &str) -> (ParsedArtifact, ParsedArtifact, String) {
    let p1 = parse_from_content(src, path);
    let out = serialize(&from_parsed(&p1), MAX).unwrap();
    (p1, parse_from_content(&out, path), out)
}

#[test]
fn fixtures_round_trip_to_an_equal_parse() {
    for (src, path) in [
        (MINIMAL, "Snippets/a.md"),
        (QUOTED, "Snippets/a.md"),
        (PLAIN, "Snippets/a.md"),
        (MULTI, "Snippets/a.md"),
        (ORPHAN, "Snippets/a.md"),
    ] {
        let (p1, p2, _) = round_trip(src, path);
        assert_eq!(p2, p1, "{src}");
    }
}

#[test]
fn command_normalises_to_bash_and_is_idempotent() {
    let (p1, p2, out) = round_trip(COMMAND, "Commands/c.md");
    assert_eq!(p2.frontmatter.language.as_deref(), Some("bash"));
    assert_eq!(p2.code, p1.code);
    assert_eq!(p2.vars, p1.vars);
    assert_eq!(serialize(&from_parsed(&p2), MAX).unwrap(), out);
}

// ── YAML safety, sub-sets, agents ────────────────────────────────────────────

fn fm_of(v: Value, path: &str) -> ParsedArtifact {
    parse_from_content(&ser(v), path)
}

#[test]
fn newlines_in_title_and_description_become_spaces() {
    let p = fm_of(
        json!({"artifactType":"Snippet","title":"Foo\nBar","description":"Line1\nLine2","blocks":[{"code":"x"}]}),
        "Snippets/p.md",
    );
    assert_eq!(p.frontmatter.title.as_deref(), Some("Foo Bar"));
    assert_eq!(p.frontmatter.description.as_deref(), Some("Line1 Line2"));
}

#[test]
fn crlf_and_cr_in_title_become_spaces() {
    let p = fm_of(
        json!({"artifactType":"Snippet","title":"A\r\nB\rC","description":"X","blocks":[{"code":"x"}]}),
        "Snippets/p.md",
    );
    assert_eq!(p.frontmatter.title.as_deref(), Some("A B C"));
}

#[test]
fn empty_default_is_not_emitted() {
    let out = ser(json!({"artifactType":"Snippet","title":"T",
        "blocks":[{"code":"<VK-host>","vars":[{"name":"VK-host","value":""}]}]}));
    assert!(!out.contains("vks"));
}

#[test]
fn empty_sub_set_survives_the_round_trip() {
    let out = ser(json!({"artifactType":"Variables","title":"Dev","blocks":[
        {"heading":"Alpha","vars":[{"name":"VK-a","value":"1"}]},
        {"heading":"Fresh"}]}));
    let p = parse_from_content(&out, "Variables/dev.md");
    let heads: Vec<_> = p.blocks.iter().map(|b| b.heading.as_str()).collect();
    assert_eq!(heads, ["Alpha", "Fresh"]);
    assert!(p.blocks[1].vars.is_empty());
}

#[test]
fn one_headed_variables_block_keeps_its_heading_and_flat_stays_flat() {
    let out = ser(
        json!({"artifactType":"Variables","blocks":[{"heading":"Only","vars":[{"name":"VK-a","value":"1"}]}]}),
    );
    assert_eq!(parse_from_content(&out, "Variables/v.md").blocks.len(), 1);
    let flat =
        ser(json!({"artifactType":"Variables","blocks":[{"vars":[{"name":"VK-a","value":"1"}]}]}));
    let p = parse_from_content(&flat, "Variables/v.md");
    assert!(p.blocks.is_empty());
    assert_eq!(p.vars.len(), 1);
    assert_eq!(
        ser(json!({"artifactType":"Variables"})),
        "---\nartifactType: Variables\n---\n"
    );
}

fn agent(over: Value) -> Value {
    let mut v = json!({"artifactType":"AIAgentsConfig","title":"Code reviewer","provider":"Claude",
        "model":"Opus","version":"4.8","blocks":[{"language":"md","code":"review it"}]});
    for (k, x) in over.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

#[test]
fn agent_keys_in_d3_order() {
    let md = ser(agent(json!({"tags":["review"]})));
    let at = |k: &str| md.find(k).unwrap();
    assert!(
        md.contains("provider: Claude\n")
            && md.contains("model: Opus\n")
            && md.contains("version: 4.8\n")
    );
    assert!(
        at("provider:") < at("model:")
            && at("model:") < at("version:")
            && at("version:") < at("tags:")
    );
}

#[test]
fn empty_agent_keys_are_omitted() {
    let md = ser(agent(json!({"provider":"","model":"","version":""})));
    assert!(!md.contains("provider:") && !md.contains("model:") && !md.contains("version:"));
}

#[test]
fn newline_injection_in_provider_cannot_override_the_type() {
    let md = ser(agent(json!({"provider":"x\nartifactType: Command"})));
    let p = parse_from_content(&md, "AIAgentsConf/x.md");
    assert_eq!(p.frontmatter.artifact_type.as_str(), "AIAgentsConfig");
    assert!(!md.contains("\nartifactType: Command"));
}

#[test]
fn all_ten_frontmatter_strings_round_trip() {
    let m = model(agent(
        json!({"description":"d","extension":"md","env":"e","target":"AGENTS.md","tags":["a"]}),
    ));
    let out = serialize(&m, MAX).unwrap();
    assert_eq!(
        from_parsed(&parse_from_content(&out, "AIAgentsConf/x.md")),
        m
    );
}

// ── The guard ────────────────────────────────────────────────────────────────

#[test]
fn code_with_a_fence_is_refused() {
    let r = refused(json!({"artifactType":"Snippet","blocks":[{"code":"a```b"}]}));
    assert_eq!(r, ("blocks/0/code".into(), FieldReason::RoundTrip));
}

#[test]
fn tag_with_a_comma_is_refused() {
    let r = refused(json!({"artifactType":"Snippet","tags":["a,b"],"blocks":[{"code":"x"}]}));
    assert_eq!(r, ("tags".into(), FieldReason::RoundTrip));
}

#[test]
fn heading_line_in_multi_block_code_is_refused() {
    let r = refused(json!({"artifactType":"Snippet","blocks":[
        {"heading":"A","code":"x\n## x\ny"},{"heading":"B","code":"z"}]}));
    assert_eq!(r.1, FieldReason::RoundTrip);
    assert!(r.0.starts_with("blocks"), "{}", r.0);
}

#[test]
fn empty_heading_in_a_two_block_snippet_is_refused() {
    let r = refused(json!({"artifactType":"Snippet","blocks":[
        {"heading":"A","code":"x"},{"heading":"","code":"z"}]}));
    assert_eq!(r, ("blocks".into(), FieldReason::RoundTrip));
}

#[test]
fn template_with_two_blocks_is_multi_block() {
    let r = refused(json!({"artifactType":"Template","blocks":[
        {"heading":"A","code":"x"},{"heading":"B","code":"z"}]}));
    assert_eq!(r, ("blocks".into(), FieldReason::MultiBlock));
}

#[test]
fn output_over_the_limit_is_too_large() {
    let m = model(json!({"artifactType":"Snippet","blocks":[{"code":"x"}]}));
    assert!(matches!(serialize(&m, 10), Err(SerializeError::TooLarge { size }) if size > 10));
}

#[test]
fn model_var_order_does_not_matter_and_locked_language_is_normalised() {
    let ok = serialize(
        &model(json!({"artifactType":"Snippet","blocks":[
            {"heading":"A","code":"<VK-a> <VK-b>","vars":[{"name":"VK-b","value":"2"},{"name":"VK-a","value":"1"}]},
            {"heading":"B","code":"z"}]})),
        MAX,
    );
    assert!(ok.is_ok(), "{ok:?}");
    let out = ser(json!({"artifactType":"Command","blocks":[{"language":"sh","code":"ls"}]}));
    assert!(out.contains("language: bash\n") && out.contains("```bash\n"));
}

#[test]
fn fuzzed_strings_never_panic() {
    for s in common::fuzz_strings(0x5E71, 2_000, 30) {
        for v in [
            json!({"artifactType":"Snippet","title":s,"blocks":[{"code":s}]}),
            json!({"artifactType":"Snippet","blocks":[{"heading":s,"code":"x"},{"heading":"b","description":s,"code":"y"}]}),
            json!({"artifactType":"Variables","blocks":[{"heading":s}]}),
        ] {
            let _ = serialize(&model(v), MAX);
        }
    }
}

#[test]
fn variables_block_language_is_normalised_away() {
    let out = serialize(
        &model(
            json!({"artifactType":"Variables","blocks":[{"language":"x","vars":[{"name":"VK-a","value":"1"}]}]}),
        ),
        MAX,
    );
    assert!(out.is_ok(), "{out:?}");
}

#[test]
fn empty_code_template_is_refused_recorded_ceiling() {
    // ponytail ceiling: TS parser parity; an empty-fence whole-file type re-parses as body text.
    let r = serialize(
        &model(json!({"artifactType":"Template","title":"New"})),
        MAX,
    );
    assert!(
        matches!(
            r,
            Err(SerializeError::Field {
                reason: FieldReason::RoundTrip,
                ..
            })
        ),
        "{r:?}"
    );
}
