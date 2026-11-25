//! Ports of `test/leetcode-config-blocks.test.ts` plus hostile cases (sinks: bounded output, no panic).

use super::*;
use std::time::Instant;

fn codes(r: &ConfigBlocksResult) -> Vec<&'static str> {
    r.warnings.iter().map(|w| w.code).collect()
}

fn fence(body: &str) -> String {
    format!("```yaml leetcode\n{body}\n```")
}

#[test]
fn extracts_a_single_fence_body() {
    assert_eq!(
        extract_config_blocks("```yaml leetcode\nreturns: int\n```").raw,
        "returns: int"
    );
}

#[test]
fn bare_yaml_and_files_entries_are_excluded() {
    for src in [
        "```yaml\nreturns: int\n```",
        "```yaml leetcode path=src/x.yml\nreturns: int\n```",
    ] {
        let r = extract_config_blocks(src);
        assert_eq!((r.raw.as_str(), r.spans.len()), ("", 0), "{src}");
    }
}

#[test]
fn unterminated_fence_yields_no_block_and_one_warning() {
    let r = extract_config_blocks("```yaml leetcode\nreturns: int");
    assert_eq!((r.raw.as_str(), r.spans.len()), ("", 0));
    assert_eq!(codes(&r), ["leetcode.config.unterminated_fence"]);
    assert!(r.warnings[0].params.is_empty());
}

#[test]
fn closing_fence_with_trailing_whitespace_closes() {
    let r = extract_config_blocks("```yaml leetcode\nreturns: int\n```  ");
    assert_eq!(r.raw, "returns: int");
    assert!(r.warnings.is_empty());
}

#[test]
fn unterminated_foreign_fence_hides_config_after_it() {
    let r = extract_config_blocks("```python\n```yaml leetcode\nreturns: int\n```");
    assert_eq!(r.raw, "");
    assert_eq!(codes(&r), ["leetcode.config.hidden_by_open_fence"]);
}

#[test]
fn adjacent_fences_do_not_merge() {
    let r = extract_config_blocks("```yaml leetcode\na: 1\n```\n```yaml leetcode\nb: 2\n```");
    assert_eq!((r.raw.as_str(), r.spans.len()), ("a: 1\nb: 2", 2));
}

#[test]
fn config_shaped_line_inside_foreign_fence_is_not_config() {
    assert_eq!(
        extract_config_blocks("```python\n```yaml leetcode\nfake: 1\n```\n```").raw,
        ""
    );
}

#[test]
fn crlf_gives_same_raw_and_lengthened_spans() {
    let lf = extract_config_blocks("```yaml leetcode\nreturns: int\n```");
    let crlf = extract_config_blocks("```yaml leetcode\r\nreturns: int\r\n```");
    assert_eq!(crlf.raw, lf.raw);
    assert_eq!((lf.spans[0].end, crlf.spans[0].end), (33, 35));
}

#[test]
fn indented_first_line_is_kept_and_warned() {
    let r = extract_config_blocks("```yaml leetcode\n  returns: int\n```");
    assert_eq!(r.raw, "  returns: int");
    assert_eq!(codes(&r), ["leetcode.config.indented"]);
    assert_eq!(r.warnings[0].params["line"], "  returns: int");
}

#[test]
fn duplicate_keys_name_key_count_and_winner() {
    let last = extract_config_blocks(&format!(
        "{}\n{}",
        fence("returns: int"),
        fence("returns: string")
    ));
    let w = &last.warnings[0];
    assert_eq!(w.code, "leetcode.config.duplicate_key");
    assert_eq!(
        (
            w.params["key"].as_str(),
            w.params["count"].as_str(),
            w.params["winner"].as_str()
        ),
        ("returns", "2", "last")
    );
    let first = extract_config_blocks(&format!(
        "{}\n{}",
        fence("libs:\n  python:\n    - a"),
        fence("libs:\n  python:\n    - b")
    ));
    assert_eq!(
        (
            first.warnings[0].params["key"].as_str(),
            first.warnings[0].params["winner"].as_str()
        ),
        ("libs", "first")
    );
}

#[test]
fn proto_key_duplicated_warns_nothing() {
    let r = extract_config_blocks(&format!(
        "{}\n{}",
        fence("__proto__: 1"),
        fence("__proto__: 2")
    ));
    assert!(r.warnings.is_empty());
}

#[test]
fn spans_cover_whole_fence_offset_into_body() {
    let body = "before\n```yaml leetcode\nreturns: int\n```\nafter";
    let r = extract_config_blocks(body);
    assert_eq!(r.spans.len(), 1);
    assert_eq!(
        &body[r.spans[0].start..r.spans[0].end],
        "```yaml leetcode\nreturns: int\n```"
    );
}

#[test]
fn retained_key_in_fence_warns_once_per_key() {
    let r = extract_config_blocks(&fence("status: unsolved"));
    assert_eq!(codes(&r), ["leetcode.config.key_in_frontmatter"]);
    assert_eq!(r.warnings[0].params["key"], "status");
    let twice = extract_config_blocks(&format!("{}\n\n{}", fence("title: A"), fence("title: B")));
    assert_eq!(codes(&twice), ["leetcode.config.key_in_frontmatter"]);
    assert!(
        extract_config_blocks(&fence("function: f\nreturns: int"))
            .warnings
            .is_empty()
    );
}

#[test]
fn warning_order_is_scan_then_retained_then_duplicate() {
    let src = format!(
        "{}\n{}\n```py",
        fence("  status: x\nreturns: 1"),
        fence("returns: 2\nstatus: y")
    );
    assert_eq!(
        codes(&extract_config_blocks(&src)),
        [
            "leetcode.config.indented",
            "leetcode.config.hidden_by_open_fence",
            "leetcode.config.key_in_frontmatter",
            "leetcode.config.duplicate_key"
        ]
    );
}

// ── legacy_frontmatter_keys ────────────────────────────────────────────────
#[test]
fn legacy_keys() {
    assert!(legacy_frontmatter_keys("type: leetcode\ntitle: Two Sum\n").is_empty());
    assert_eq!(
        legacy_frontmatter_keys("title: X\nfunction: twoSum\n"),
        ["function"]
    );
    let fm = "libs:\n  python:\n    - a\ntest:\n  type: function\nlibs:\n  rust:\n    - b\n";
    assert_eq!(legacy_frontmatter_keys(fm), ["libs", "test"]);
    assert!(legacy_frontmatter_keys("title: X\n  function: nested\n").is_empty());
    assert_eq!(
        legacy_frontmatter_keys("a: 1\r\nreturns: int\r\n"),
        ["returns"]
    );
}

// ── split_frontmatter ──────────────────────────────────────────────────────
#[test]
fn split_frontmatter_cases() {
    assert_eq!(
        split_frontmatter("---\ntitle: X\n---\nBody text"),
        ("title: X", "Body text")
    );
    assert_eq!(
        split_frontmatter("No frontmatter here"),
        ("", "No frontmatter here")
    );
    assert_eq!(
        split_frontmatter("---\r\ntitle: X\r\n---\r\nBody"),
        ("title: X", "Body")
    );
}

// ── without_body_set_keys ──────────────────────────────────────────────────
#[test]
fn without_body_set_keys_cases() {
    let clean = "type: leetcode\ntitle: X\ndifficulty: easy\ntags: [a]";
    assert_eq!(without_body_set_keys(clean), clean);
    assert_eq!(
        without_body_set_keys("title: X\nparams:\n  - name: a\n    type: int\nstatus: unsolved"),
        "title: X\nstatus: unsolved"
    );
    assert_eq!(
        without_body_set_keys("params:\n  - name: a\n    type: int\ntitle: Kept"),
        "title: Kept"
    );
    assert_eq!(without_body_set_keys("title: X\nreturns: int"), "title: X");
    let out = without_body_set_keys("params:\n  - name: a\n\n  - name: b\ntitle: X");
    assert!(!out.contains("name: a") && out.contains("title: X"));
    assert_eq!(
        without_body_set_keys("title: X\r\nparams:\r\n  - name: a\r\nstatus: unsolved"),
        "title: X\r\nstatus: unsolved"
    );
    assert_eq!(
        without_body_set_keys(
            "function: f\ntitle: X\nreturns: int\ntest:\n  type: function\ntags: [a]"
        ),
        "title: X\ntags: [a]"
    );
}

// ── shared block-line grammar ──────────────────────────────────────────────
#[test]
fn block_lines_cases() {
    let l = ["params:", "  - name: a", "  - name: b", "returns: int"];
    assert_eq!(block_lines(&l, 0), ["  - name: a", "  - name: b"]);
    let l = ["params:", "  - name: a", "", "  - name: b", "returns: int"];
    assert_eq!(block_lines(&l, 0), ["  - name: a", "  - name: b"]);
    assert!(block_lines(&["params:", "returns: int"], 0).is_empty());
    assert!(block_lines(&["a"], 5).is_empty());
}

#[test]
fn indent_of_and_unquote_pair() {
    assert_eq!(
        (
            indent_of("    - name: a"),
            indent_of("function: f"),
            indent_of("\u{feff} x")
        ),
        (4, 0, 2)
    );
    assert_eq!(unquote_pair("\"react\""), "react");
    assert_eq!(unquote_pair("'react'"), "react");
    assert_eq!(unquote_pair("  react  "), "react");
    assert_eq!(unquote_pair("\"react'"), "\"react'");
    assert_eq!(unquote_pair("\""), "\"");
    assert_eq!(unquote_pair("\"a\u{2028}b\""), "\"a\u{2028}b\"");
}

#[test]
fn scanners_use_js_classes() {
    assert_eq!(top_level_key("status: x"), Some("status"));
    assert_eq!(top_level_key("  nested: v"), None);
    assert_eq!(top_level_key("é: v"), None); // JS \w is ASCII
    assert_eq!(top_level_key(": v"), None);
    assert_eq!(kv_line("  type: x"), None);
    assert_eq!(kv_line("type: call "), Some(("type", "call ")));
    assert_eq!(kv_line("type:\u{2028}call"), Some(("type", "call")));
    assert_eq!(kv_line("type: a\u{2028}b"), None);
}

// ── hostile: each asserts the bounded sink (result shape / time), never a panic ──
#[test]
fn hostile_one_mb_key_line_is_linear() {
    let body = format!("```yaml leetcode\n{}\n```", "a".repeat(1_000_000));
    let t = Instant::now();
    let r = extract_config_blocks(&body);
    assert!(t.elapsed().as_secs() < 2);
    assert_eq!(
        (r.raw.len(), r.spans.len(), r.warnings.len()),
        (1_000_000, 1, 0)
    );
}

#[test]
fn hostile_one_mb_unterminated_frontmatter_is_no_frontmatter() {
    let src = format!("---\n{}", "a".repeat(1_000_000));
    let t = Instant::now();
    let (fm, body) = split_frontmatter(&src);
    assert!(t.elapsed().as_secs() < 2);
    assert_eq!((fm, body.len()), ("", src.len()));
}

#[test]
fn hostile_frontmatter_cap_is_inclusive_and_skips_the_block() {
    let build = |n: usize| format!("---\n{}\n---\nBody", "a".repeat(n));
    // The cap is the whole block: `---\n` + content + `\n---\n` is content + 9 bytes.
    let ok = build(MAX_FRONTMATTER_BYTES - 9);
    assert_eq!(split_frontmatter(&ok).0.len(), MAX_FRONTMATTER_BYTES - 9);
    let big = build(MAX_FRONTMATTER_BYTES - 8);
    assert_eq!(split_frontmatter(&big), ("", "Body"));
}

#[test]
fn hostile_many_fences_and_keys_bound_the_warnings() {
    let one = "```yaml leetcode\nreturns: int\nk1: 1\nstatus: x\n```\n";
    let r = extract_config_blocks(&one.repeat(20_000));
    assert_eq!((r.spans.len(), r.warnings.len()), (20_000, 2));
    assert_eq!(r.warnings[1].params["count"], "20000");
}

#[test]
fn hostile_long_indented_line_truncates_the_param() {
    let r = extract_config_blocks(&fence(&format!("  {}", "é".repeat(5000))));
    assert_eq!(r.warnings[0].params["line"].chars().count(), 200);
    assert_eq!(r.raw.chars().count(), 5002);
}

#[test]
fn hostile_odd_text_never_panics() {
    for s in [
        "",
        "```",
        "```yaml leetcode",
        "```yaml leetcode\n",
        "\u{2028}\r\r\n```yaml  leetcode \r\n```\r",
        "```yaml\u{a0}leetcode\u{feff}\n```",
    ] {
        let _ = extract_config_blocks(s);
        let _ = split_frontmatter(s);
        let _ = without_body_set_keys(s);
        let _ = legacy_frontmatter_keys(s);
    }
    // NBSP / BOM count as JS whitespace in the marker.
    assert_eq!(
        extract_config_blocks("```yaml\u{a0}leetcode\u{feff}\nx: 1\n```").raw,
        "x: 1"
    );
}
