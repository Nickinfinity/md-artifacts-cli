//! `leetcode::parse` assembly: gating, warning-slot order, bounds. Per-group behaviour is pinned
//! by the `conformance/leetcode` cases; these cover what only the assembly decides.

use mda_core::leetcode::{LeetcodeType, parse};

fn codes(src: &str) -> Vec<&'static str> {
    parse(src).warnings.iter().map(|w| w.code).collect()
}

#[test]
fn clean_function_has_empty_warnings_and_no_project_fields() {
    let e = parse("---\ntitle: T\n---\n");
    assert!(e.warnings.is_empty() && e.unimplemented_kinds.is_empty());
    assert!(e.files.is_none() && e.checks.is_none() && e.libs.is_none() && e.packages.is_none());
}

#[test]
fn project_parse_runs_only_for_multi_file_types() {
    // `packges:` is within two edits of a known key: W23 for a package, silent for a function.
    let body = "\n```yaml leetcode\npackges: x\n```\n";
    let f = format!("---\nartifactType: LeetCode\n---\n{body}");
    assert!(codes(&f).is_empty());
    let p = format!("---\nartifactType: LeetCode\nleetcodeType: package\n---\n{body}");
    assert_eq!(codes(&p), ["leetcode.config.unknown_key"]);
    let e = parse(&p);
    assert_eq!(e.leetcode_type, LeetcodeType::Package);
    assert_eq!(e.files, Some(vec![]));
    assert_eq!(e.libs.map(|l| l.0.len()), Some(0));
}

#[test]
fn warning_slots_are_legacy_extractor_program_type() {
    let src = "---\nparams: x\nleetcodeType: bogus\n---\n\n```yaml leetcode\nprogram:\n  entry: a\n```\n```yaml leetcode\nreturns: int\n```\n```yaml leetcode\nreturns: str\n```\n```yaml leetcode\n";
    assert_eq!(
        codes(src),
        [
            "leetcode.frontmatter.ignored_key",
            "leetcode.config.unterminated_fence",
            "leetcode.config.duplicate_key",
            "leetcode.program.no_channel",
            "leetcode.type.unknown",
        ]
    );
}

#[test]
fn hostile_input_returns_bounded_output_and_short_params() {
    let src = format!(
        "---\nleetcodeType: {}\n---\n{}",
        "x".repeat(50_000),
        "```yaml leetcode\n".repeat(20_000)
    );
    let e = parse(&src);
    assert!(
        e.warnings
            .iter()
            .flat_map(|w| w.params.values())
            .all(|v| v.chars().count() <= 200)
    );
    assert!(serde_json::to_string(&e).unwrap().len() <= 64 * src.len() + 4096);
}
