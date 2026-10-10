//! Ports `flags.service.test.ts:16-204`.
mod common;

use mda_core::parse::flags::{FlaggedRegion, extract_flagged_regions as x};

fn r(name: &str, content: &str) -> FlaggedRegion {
    FlaggedRegion {
        name: name.into(),
        content: content.into(),
    }
}

#[test]
fn no_flags_no_regions() {
    assert_eq!(x("# Notes\n\n```js\nx\n```\n"), vec![]);
}

#[test]
fn unnamed_region() {
    let body = "Notes above.\n\n%%oa:start%%\nReview <VK-file>.\n%%oa:end%%\n\nNotes below.";
    assert_eq!(x(body), vec![r("", "Review <VK-file>.")]);
}

#[test]
fn outside_text_dropped() {
    let body = "Private vault notes.\n%%oa:start%%\npayload\n%%oa:end%%\ntrailing notes";
    assert_eq!(x(body)[0].content, "payload");
}

#[test]
fn named_start() {
    assert_eq!(
        x("%%oa:start Dev server%%\nrun dev\n%%oa:end%%"),
        vec![r("Dev server", "run dev")]
    );
}

#[test]
fn several_in_order() {
    let body = "%%oa:start Dev%%\ndev payload\n%%oa:end%%\nnotes between\n%%oa:start Prod%%\nprod payload\n%%oa:end%%";
    assert_eq!(
        x(body),
        vec![r("Dev", "dev payload"), r("Prod", "prod payload")]
    );
}

#[test]
fn marker_whitespace_tolerated() {
    let body = "  %%  oa:start   Dev  %%  \npayload\n  %% oa:end %%";
    assert_eq!(x(body), vec![r("Dev", "payload")]);
}

#[test]
fn inner_markdown_verbatim() {
    let body = "%%oa:start%%\n# Heading\n\n- bullet\n\n```bash\nnpm test\n```\n%%oa:end%%";
    assert_eq!(
        x(body)[0].content,
        "# Heading\n\n- bullet\n\n```bash\nnpm test\n```"
    );
}

#[test]
fn end_flag_in_fence_does_not_close() {
    let body = "%%oa:start%%\nWrap your prompt like this:\n```md\n%%oa:start%%\ntext\n%%oa:end%%\n```\nDone.\n%%oa:end%%";
    let g = x(body);
    assert_eq!(g.len(), 1);
    assert!(g[0].content.ends_with("Done."));
    assert!(g[0].content.contains("%%oa:end%%"));
}

#[test]
fn tilde_fence_not_closed_by_backticks() {
    let g = x("%%oa:start%%\n~~~\n```\n%%oa:end%%\n~~~\ntail\n%%oa:end%%");
    assert_eq!(g.len(), 1);
    assert!(g[0].content.ends_with("tail"));
}

#[test]
fn unterminated_runs_to_eof() {
    assert_eq!(
        x("%%oa:start Draft%%\nstill writing"),
        vec![r("Draft", "still writing")]
    );
}

#[test]
fn second_start_is_content() {
    let g = x("%%oa:start A%%\none\n%%oa:start B%%\ntwo\n%%oa:end%%");
    assert_eq!(g, vec![r("A", "one\n%%oa:start B%%\ntwo")]);
}

#[test]
fn blank_ends_trimmed_middle_kept() {
    assert_eq!(
        x("%%oa:start%%\n\n\nfirst\n\n\nlast\n\n%%oa:end%%")[0].content,
        "first\n\n\nlast"
    );
}

#[test]
fn rule_bracket_dropped() {
    assert_eq!(
        x("%%oa:start%%\n***\n# here the MD text\n***\n%%oa:end%%")[0].content,
        "# here the MD text"
    );
}

#[test]
fn rules_dropped_with_blanks() {
    assert_eq!(
        x("%%oa:start Dev%%\n\n***\n\npayload\n\n***\n\n%%oa:end%%"),
        vec![r("Dev", "payload")]
    );
}

#[test]
fn rule_dropped_anywhere() {
    assert_eq!(
        x("%%oa:start%%\n***\nintro\n***\noutro\n***\n%%oa:end%%")[0].content,
        "intro\noutro"
    );
}

#[test]
fn dashes_are_content() {
    assert_eq!(
        x("%%oa:start%%\n***\nintro\n\n---\n\noutro\n***\n%%oa:end%%")[0].content,
        "intro\n\n---\n\noutro"
    );
}

#[test]
fn rule_length() {
    assert_eq!(
        x("%%oa:start%%\n*****\nbody\n%%oa:end%%")[0].content,
        "body"
    );
    assert_eq!(
        x("%%oa:start%%\n**\nbody\n%%oa:end%%")[0].content,
        "**\nbody"
    );
}

#[test]
fn bold_italic_not_rule() {
    assert_eq!(
        x("%%oa:start%%\n***emphasis***\n%%oa:end%%")[0].content,
        "***emphasis***"
    );
}

#[test]
fn rule_in_fence_untouched() {
    assert_eq!(
        x("%%oa:start%%\n```md\n***\nstyled\n***\n```\n%%oa:end%%")[0].content,
        "```md\n***\nstyled\n***\n```"
    );
}

#[test]
fn rule_only_region_empty() {
    assert_eq!(x("%%oa:start%%\n***\n%%oa:end%%")[0].content, "");
}

#[test]
fn rule_between_regions_ignored() {
    let body = "%%oa:start A%%\none\n%%oa:end%%\n***\n%%oa:start B%%\ntwo\n%%oa:end%%";
    assert_eq!(x(body), vec![r("A", "one"), r("B", "two")]);
}

#[test]
fn crlf_same_as_lf() {
    assert_eq!(
        x("%%oa:start Dev%%\r\npayload\r\n%%oa:end%%\r\n"),
        vec![r("Dev", "payload")]
    );
}

#[test]
fn fence_outside_region_hides_next_start() {
    // The trap: an unclosed fence before the flag swallows it.
    assert_eq!(x("```\n%%oa:start%%\nx\n%%oa:end%%"), vec![]);
}

#[test]
fn fuzz_no_panic() {
    for s in common::fuzz_strings(0xF1A6, 10_000, 40) {
        let _ = x(&s);
    }
}

// ── Drift guard: only parse/flags.rs spells the marker ──────────────────────

/// Markers spelled in `src` once `//` comments are stripped.
fn offenders(src: &str) -> Vec<&'static str> {
    let code: String = src
        .lines()
        .map(|l| l.split_once("//").map_or(l, |(c, _)| c))
        .collect::<Vec<_>>()
        .join("\n");
    ["oa:start", "oa:end"]
        .into_iter()
        .filter(|m| code.contains(m))
        .collect()
}

#[test]
fn guard_detects_marker() {
    assert_eq!(
        offenders("let s = \"%%oa:start%%\"; // %%oa:end%%"),
        vec!["oa:start"]
    );
    assert!(offenders("// %%oa:start%%\nlet a = 1;").is_empty());
}

fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            rs_files(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    Ok(())
}

#[test]
fn only_flags_rs_spells_marker() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    rs_files(&root, &mut files).unwrap();
    assert!(!files.is_empty());
    let bad: Vec<_> = files
        .iter()
        .filter(|p| !p.ends_with("parse/flags.rs"))
        .filter(|p| !offenders(&std::fs::read_to_string(p).unwrap()).is_empty())
        .collect();
    assert!(bad.is_empty(), "marker re-spelled in {bad:?}");
}
