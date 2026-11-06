//! T4.1 tests: summary, discriminator, order (TS `frontmatter-order`, `leetcode-type-derivation`,
//! the frontmatter half of `leetcode-parser`).

use super::*;
use crate::leetcode::{LeetCodeDifficulty, LeetCodeStatus, LeetcodeType};
use crate::parse::decode;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}
fn ord(fm: &str) -> Option<(String, String)> {
    order_violation(fm).map(|v| (v.key, v.before))
}
fn pair(a: &str, b: &str) -> Option<(String, String)> {
    Some((a.to_owned(), b.to_owned()))
}

// ---- order_violation (frontmatter-order.test.ts) ----

#[test]
fn order_reports_first_violation() {
    assert_eq!(
        ord("title: X\nartifactType: LeetCode\n"),
        pair("artifactType", "title")
    );
}

#[test]
fn order_ok_cases() {
    let full = "artifactType: LeetCode\nleetcodeType: function\ntitle: Two Sum\ndifficulty: easy\nstatus: unsolved\nalgorithm: array\ntags: [a, b]\n";
    assert_eq!(ord(full), None);
    assert_eq!(ord("title: X\nstatus: unsolved\n"), None);
    assert_eq!(ord(""), None);
    assert_eq!(ord("\n\n\n"), None);
    assert_eq!(
        ord("artifactType: LeetCode\ncustomField: whatever\ntitle: X\n"),
        None
    );
}

#[test]
fn order_hostile() {
    assert_eq!(ord("artifactType: x\ntitle: X\ntitle: Y\n"), None);
    assert_eq!(
        ord("title: X\ntitle: Y\nartifactType: x\n"),
        pair("artifactType", "title")
    );
    assert_eq!(ord("__proto__: 1\nartifactType: x\ntitle: X\n"), None);
    assert_eq!(
        ord("tags: x\n__proto__: 1\ntitle: Y\n"),
        pair("title", "tags")
    );
    assert_eq!(
        ord("not-a-key-line-at-all\nartifactType: x\ntitle: X\n"),
        None
    );
    assert_eq!(
        ord("title: X\r\nartifactType: x\r\n"),
        pair("artifactType", "title")
    );
    let long = format!("{}: 1\nartifactType: x\ntitle: X\n", "a".repeat(10_000));
    assert_eq!(ord(&long), None);
    // an indented key is not top level
    assert_eq!(ord("title: X\n  artifactType: x\n"), None);
}

// ---- summary (parseFrontmatterOnly + leetcode-type-derivation end to end) ----

#[test]
fn summary_reads_the_retained_keys() {
    let md = "---\nartifactType: LeetCode\nleetcodeType: package\ntitle: Two Sum\ndifficulty: hard\nstatus: solved\nalgorithm: hash-map\ntags: [a, b]\n---\nBody\n";
    let x = summary(md);
    assert_eq!(x.title, "Two Sum");
    assert_eq!(x.leetcode_type, Some(LeetcodeType::Package));
    assert_eq!(x.difficulty, LeetCodeDifficulty::Hard);
    assert_eq!(x.status, LeetCodeStatus::Solved);
    assert_eq!(x.algorithm.as_deref(), Some("hash-map"));
    assert_eq!(x.tags, s(&["a", "b"]));
}

#[test]
fn summary_defaults_and_unknown_values() {
    let x = summary("---\ntitle: Demo\ndifficulty: extreme\nstatus: done\n---\n");
    assert_eq!(x.title, "Demo");
    assert_eq!(x.leetcode_type, None);
    assert_eq!(x.difficulty, LeetCodeDifficulty::Easy);
    assert_eq!(x.status, LeetCodeStatus::Unsolved);
    assert_eq!((x.algorithm, x.tags), (None, vec![]));
    assert_eq!(summary("no frontmatter").title, "");
}

#[test]
fn summary_block_tags_quoted_values_and_last_wins() {
    let x = summary("---\ntitle: \"Quoted: Title\"\ntitle: Second\ntags:\n  - a\n  - b\n---\n");
    assert_eq!(x.title, "Second");
    assert_eq!(x.tags, s(&["a", "b"]));
    assert_eq!(
        summary("---\ntitle: \"Q\"\nalgorithm: 'tp'\n---\n")
            .algorithm
            .as_deref(),
        Some("'tp'")
    );
}

#[test]
fn summary_type_is_declared_only_and_unknown_derives() {
    assert_eq!(
        summary("---\nleetcodeType: stack\n---\n").leetcode_type,
        Some(LeetcodeType::Stack)
    );
    // frontmatter-only: a v1 test: block does not make an undeclared type
    assert_eq!(
        summary("---\ntitle: D\ntest:\n  type: service\n---\n").leetcode_type,
        None
    );
    // declared but unknown: the parse's fallback (function), as the whole-file parse reports
    assert_eq!(
        summary("---\nleetcodeType: bogus\n---\n").leetcode_type,
        Some(LeetcodeType::Function)
    );
    assert_eq!(
        summary("---\nleetcodeType: __proto__\n---\n").leetcode_type,
        Some(LeetcodeType::Function)
    );
}

#[test]
fn summary_through_decode_bom_and_crlf() {
    let bytes =
        b"\xEF\xBB\xBF---\r\nartifactType: LeetCode\r\ntitle: Bom\r\ntags: [x]\r\n---\r\nBody\r\n";
    let text = decode(bytes);
    assert_eq!(summary(&text).title, "Bom");
    assert_eq!(summary(&text).tags, s(&["x"]));
    assert_eq!(discriminator(&text), Discriminator::Exact);
}

#[test]
fn summary_ignores_the_body_and_huge_frontmatter() {
    assert_eq!(
        summary("---\ntitle: A\n---\n```yaml leetcode\nparams: x\n```\ntitle: B\n").title,
        "A"
    );
    let big = format!("---\ntitle: A\n{}\n---\n", "k: v\n".repeat(20_000));
    assert_eq!(summary(&big).title, "");
}

// ---- discriminator ----

fn disc(fm: &str) -> Discriminator {
    discriminator(&format!("---\n{fm}\n---\nBody\n"))
}

#[test]
fn discriminator_classes() {
    assert_eq!(discriminator("# just a note\n"), Discriminator::Absent);
    assert_eq!(discriminator(""), Discriminator::Absent);
    assert_eq!(disc("title: X"), Discriminator::Absent);
    assert_eq!(disc("artifactType: LeetCode"), Discriminator::Exact);
    assert_eq!(disc("artifactType:   LeetCode  "), Discriminator::Exact);
    assert_eq!(disc("artifactType: leetcode"), Discriminator::Lowercase);
    assert_eq!(
        disc("artifactType: recipe"),
        Discriminator::Other("recipe".into())
    );
    assert_eq!(disc("artifactType:"), Discriminator::Other(String::new()));
}

#[test]
fn discriminator_quoted_and_commented_are_other() {
    assert_eq!(
        disc("artifactType: \"LeetCode\""),
        Discriminator::Other("\"LeetCode\"".into())
    );
    assert_eq!(
        disc("artifactType: 'leetcode'"),
        Discriminator::Other("'leetcode'".into())
    );
    assert_eq!(
        disc("artifactType: LeetCode # c"),
        Discriminator::Other("LeetCode # c".into())
    );
}

#[test]
fn discriminator_last_line_wins_and_agrees_with_rewrite() {
    assert_eq!(
        disc("artifactType: leetcode\nartifactType: LeetCode"),
        Discriminator::Exact
    );
    // rewrite leaves two artifactType lines alone, so "Lowercase" (migration pending) would never clear
    assert_eq!(
        disc("artifactType: LeetCode\nartifactType: leetcode"),
        Discriminator::Other("leetcode".into())
    );
    for fm in [
        "artifactType: leetcode",
        "artifactType: LeetCode",
        "artifactType: \"leetcode\"",
    ] {
        let src = format!("---\n{fm}\n---\n");
        let pending = matches!(crate::migrate::frontmatter::rewrite(&src), Ok(Some(_)));
        assert_eq!(
            pending,
            discriminator(&src) == Discriminator::Lowercase,
            "{fm}"
        );
    }
}

#[test]
fn discriminator_only_reads_frontmatter_and_truncates() {
    assert_eq!(
        discriminator("---\ntitle: X\n---\nartifactType: LeetCode\n"),
        Discriminator::Absent
    );
    let long = format!("artifactType: {}", "é".repeat(500));
    let Discriminator::Other(found) = disc(&long) else {
        panic!("not other")
    };
    assert_eq!(found.chars().count(), 200);
    assert!(is_exercise("---\r\nartifactType: LeetCode\r\n---\r\n"));
    assert!(!is_exercise("---\nartifactType: leetcode\n---\n"));
}

// ---- the fields T4.1 produces, against conformance/leetcode/<group>/ expected JSON ----

fn conformance_cases(group: &str) -> Vec<(String, String, serde_json::Value)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/leetcode")
        .join(group);
    let mut out = vec![];
    for e in std::fs::read_dir(dir).expect("group dir") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "md") {
            let want =
                std::fs::read_to_string(p.with_extension("expected.json")).expect("expected");
            let src = decode(&std::fs::read(&p).expect("md"));
            out.push((
                p.display().to_string(),
                src,
                serde_json::from_str(&want).expect("json"),
            ));
        }
    }
    assert!(!out.is_empty(), "{group}");
    out
}

#[test]
fn summary_matches_conformance_fields() {
    for group in [
        "frontmatter",
        "frontmatter-order",
        "type-derivation",
        "rules",
    ] {
        for (id, src, want) in conformance_cases(group) {
            let got = serde_json::to_value(summary(&src)).expect("to_value");
            for k in ["title", "difficulty", "status", "algorithm", "tags"] {
                assert_eq!(got.get(k), want.get(k), "{id}: {k}");
            }
        }
    }
}

#[test]
fn fm_type_and_w02_match_type_derivation_cases() {
    use crate::leetcode::config::{
        extract_config_blocks, split_frontmatter, without_body_set_keys,
    };
    for (id, src, want) in conformance_cases("type-derivation") {
        let (fm_raw, body) = split_frontmatter(&src);
        let text = format!(
            "{}\n{}",
            without_body_set_keys(fm_raw),
            extract_config_blocks(body).raw
        );
        let fm = parse_frontmatter(&text);
        assert_eq!(
            serde_json::to_value(fm.leetcode_type).expect("v"),
            want["leetcodeType"],
            "{id}"
        );
        let w02: Vec<_> = want["warnings"]
            .as_array()
            .expect("arr")
            .iter()
            .filter(|w| w["code"] == "leetcode.type.unknown")
            .collect();
        assert_eq!(fm.type_warning.is_some(), !w02.is_empty(), "{id}");
        if let (Some(got), Some(w)) = (&fm.type_warning, w02.first()) {
            assert_eq!(
                serde_json::to_value(&got.params).expect("p"),
                w["params"],
                "{id}"
            );
        }
    }
}

#[test]
fn rules_cases_report_the_expected_rule() {
    for (id, src, _) in conformance_cases("rules") {
        let codes: Vec<_> = crate::leetcode::frontmatter_rules(&src)
            .iter()
            .map(|w| w.code)
            .collect();
        // lowercase discriminator: no rule (migrate.pending only, D-37); legacy `type:`: rule 1
        let want: &[&str] = if id.contains("legacy-type") {
            &["leetcode.rule.legacy_type"]
        } else {
            &[]
        };
        assert_eq!(codes, want, "{id}");
    }
    for (id, src, _) in conformance_cases("frontmatter-order") {
        assert_eq!(
            order_violation(split_fm(&src)).map(|v| v.key),
            Some("artifactType".to_owned()),
            "{id}"
        );
    }
}

fn split_fm(src: &str) -> &str {
    crate::leetcode::config::split_frontmatter(src).0
}
