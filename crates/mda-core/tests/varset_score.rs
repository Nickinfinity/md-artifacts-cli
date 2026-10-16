//! T5b.2: var-set scoring, order and the render variable rule (apply tests: `varset_apply.rs`) (spec §13.3, X-16, X-17).
#![allow(clippy::indexing_slicing, clippy::unwrap_used)] // reason: test assertions index fixed shapes

use std::cmp::Ordering;
use std::time::{Duration, Instant};

use mda_core::model::ParsedVar;
use mda_core::parse::parse_from_content;
use mda_core::render::{RenderError, select_block, variable_names};
use mda_core::varset::{Profile, order, score};

fn sv(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| (*x).to_owned()).collect()
}

fn vars(xs: &[&str]) -> Vec<ParsedVar> {
    xs.iter().map(|x| ParsedVar::text(*x, "")).collect()
}

/// Score `art`/`atags` against `set`/`stags`; `unmatched` = artifact names minus matched (the
/// response carries none), so it is derived here like TS `unmatchedVars`.
fn run(
    art: &[&str],
    atags: &[&str],
    set: &[&str],
    stags: &[&str],
) -> (mda_core::varset::Score, Vec<String>) {
    let p = Profile::new(sv(art), &sv(atags));
    let s = score(&p, &vars(set), &sv(stags));
    let unmatched = p
        .names()
        .iter()
        .filter(|n| !s.matched_vars.contains(n))
        .cloned()
        .collect();
    (s, unmatched)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
#[allow(clippy::cognitive_complexity)] // reason: one flat sequence of ported TS cases, no branching
fn scorer_ports_ts() {
    // perfect var match -> matchRatio 1.0
    let (s, un) = run(
        &["VK-host", "VK-port", "VK-path"],
        &[],
        &["VK-host", "VK-port", "VK-path"],
        &[],
    );
    close(s.match_ratio, 1.0);
    assert_eq!(sorted(s.matched_vars), ["VK-host", "VK-path", "VK-port"]);
    assert!(un.is_empty() && s.extra_vars.is_empty());
    // perfect var match with no tags -> 0.7
    close(
        run(&["VK-a", "VK-b"], &[], &["VK-a", "VK-b"], &[]).0.score,
        0.7,
    );
    // 3 of 4 -> 0.75, missing var unmatched
    let (s, un) = run(
        &["VK-host", "VK-port", "VK-path", "VK-token"],
        &[],
        &["VK-host", "VK-port", "VK-path"],
        &[],
    );
    close(s.match_ratio, 0.75);
    assert_eq!(sorted(s.matched_vars), ["VK-host", "VK-path", "VK-port"]);
    assert_eq!(un, ["VK-token"]);
    assert!(s.extra_vars.is_empty());
    // partial: 0.5 + 0 tags -> 0.35
    let s = run(&["VK-a", "VK-b"], &[], &["VK-a"], &[]).0;
    close(s.match_ratio, 0.5);
    close(s.score, 0.35);
    // zero overlap -> 0
    let (s, un) = run(&["VK-host"], &[], &["VK-other"], &[]);
    close(s.match_ratio, 0.0);
    close(s.score, 0.0);
    assert!(s.matched_vars.is_empty());
    assert_eq!(un, ["VK-host"]);
    assert_eq!(s.extra_vars, ["VK-other"]);
    // zero var overlap but tags match -> 0.3
    let s = run(
        &["VK-host"],
        &["api", "web"],
        &["VK-other"],
        &["api", "web"],
    )
    .0;
    close(s.match_ratio, 0.0);
    close(s.score, 0.3);
    assert_eq!(s.tag_matches, 2);
    // extra vars listed, do not lower score
    let (s, un) = run(
        &["VK-host", "VK-port"],
        &[],
        &["VK-host", "VK-port", "VK-extra1", "VK-extra2"],
        &[],
    );
    close(s.match_ratio, 1.0);
    close(s.score, 0.7);
    assert_eq!(sorted(s.extra_vars), ["VK-extra1", "VK-extra2"]);
    assert!(un.is_empty());
    // 2 of 3 artifact tags -> 2
    let s = run(
        &["VK-x"],
        &["api", "web", "auth"],
        &["VK-x"],
        &["api", "web", "unrelated"],
    )
    .0;
    assert_eq!(s.tag_matches, 2);
    // all artifact tags match -> total
    assert_eq!(run(&[], &["a", "b"], &[], &["a", "b"]).0.tag_matches, 2);
    // zero tag overlap -> 0
    assert_eq!(run(&[], &["a", "b"], &[], &["c", "d"]).0.tag_matches, 0);
    // combined formula 0.725
    let s = run(
        &["VK-a", "VK-b", "VK-c", "VK-d"],
        &["t1", "t2", "t3"],
        &["VK-a", "VK-b", "VK-c"],
        &["t1", "t2", "other"],
    )
    .0;
    close(s.match_ratio, 0.75);
    assert_eq!(s.tag_matches, 2);
    close(s.score, 0.725);
    // full + full -> 1.0
    let s = run(&["VK-a"], &["api"], &["VK-a"], &["api"]).0;
    close(s.match_ratio, 1.0);
    close(s.score, 1.0);
    // artifact with no tags -> 0.7
    let s = run(
        &["VK-a", "VK-b", "VK-c", "VK-d"],
        &[],
        &["VK-a", "VK-b", "VK-c", "VK-d"],
        &["anything", "else"],
    )
    .0;
    assert_eq!(s.tag_matches, 0);
    close(s.score, 0.7);
    // artifact with no vars -> tag only
    let s = run(&[], &["only-tag"], &["VK-x"], &["only-tag"]).0;
    close(s.match_ratio, 0.0);
    assert_eq!(s.tag_matches, 1);
    close(s.score, 0.3);
    // no vars and no tags -> 0
    let s = run(&[], &[], &["VK-x", "VK-y"], &["t1", "t2"]).0;
    close(s.match_ratio, 0.0);
    assert_eq!(s.tag_matches, 0);
    close(s.score, 0.0);
    // empty set vars -> all artifact vars unmatched
    let (s, un) = run(&["VK-host", "VK-port"], &[], &[], &[]);
    close(s.match_ratio, 0.0);
    assert!(s.matched_vars.is_empty() && s.extra_vars.is_empty());
    assert_eq!(sorted(un), ["VK-host", "VK-port"]);
    // :262 empty set vars but tags match -> tag only
    let s = run(&["VK-x"], &["t1"], &[], &["t1"]).0;
    close(s.match_ratio, 0.0);
    assert_eq!(s.tag_matches, 1);
    close(s.score, 0.3);
    // :276 exact name, no case-folding
    let (s, un) = run(&["VK-host"], &[], &["VK-Host"], &[]);
    close(s.match_ratio, 0.0);
    assert!(s.matched_vars.is_empty());
    assert_eq!(un, ["VK-host"]);
    assert_eq!(s.extra_vars, ["VK-Host"]);
    let s = run(&["VK-a"], &[], &["vk-a"], &[]).0;
    close(s.match_ratio, 0.0);
    assert!(s.matched_vars.is_empty());
    // VK- prefix required
    let s = run(&["VK-host"], &[], &["host"], &[]).0;
    close(s.match_ratio, 0.0);
    assert!(s.matched_vars.is_empty());
    // matched + unmatched partition the artifact names
    let (s, un) = run(&["VK-a", "VK-b", "VK-c"], &[], &["VK-a", "VK-c"], &[]);
    assert_eq!(s.matched_vars, ["VK-a", "VK-c"]);
    assert_eq!(un, ["VK-b"]);
}

#[test]
fn matched_in_artifact_order_extra_in_set_order() {
    let p = Profile::new(sv(&["VK-c", "VK-a", "VK-b"]), &[]);
    let s = score(&p, &vars(&["VK-b", "VK-x", "VK-a", "VK-x"]), &[]);
    assert_eq!(s.matched_vars, ["VK-a", "VK-b"]);
    assert_eq!(s.extra_vars, ["VK-x"]);
}

#[test]
fn tag_count_like_ts() {
    let p = Profile::new(vec![], &sv(&["api", "api", "ops"]));
    let s = score(&p, &[], &sv(&["api", "api"]));
    assert_eq!(s.tag_matches, 2);
    assert_eq!(s.score, (2.0 / 3.0) * 0.3);
}

#[test]
fn order_score_path_index() {
    assert_eq!(order((0.7, "z", 9), (0.3, "b", 0)), Ordering::Less);
    assert_eq!(order((0.3, "b", 0), (0.7, "z", 9)), Ordering::Greater);
    assert_eq!(order((0.5, "V/a", 0), (0.5, "V/b", 0)), Ordering::Less);
    assert_eq!(order((0.5, "V/a", 1), (0.5, "V/a", 2)), Ordering::Less);
    assert_eq!(order((0.5, "V/a", 1), (0.5, "V/a", 1)), Ordering::Equal);
}

fn t(n: &str, v: &str) -> ParsedVar {
    ParsedVar::text(n, v)
}

#[test]
fn variable_names_rule() {
    let n = variable_names(
        "echo <VK-a> <VK-b.c> </VK-a> <VK-each:d>",
        &[t("VK-z", ""), t("VK-a", "")],
    );
    assert_eq!(n, ["VK-z", "VK-a", "VK-b", "VK-d"]);
}

#[test]
fn select_block_matches_render_artifact() {
    let none = parse_from_content(
        "---\nartifactType: Snippet\n---\n```sh\nx\n```\n",
        "Snippets/a.md",
    );
    assert!(matches!(
        select_block(&none, Some(0)),
        Err(RenderError::BlockNotFound(Some(0)))
    ));
    assert!(select_block(&none, None).is_ok());
    let two = parse_from_content(
        "---\nartifactType: Snippet\n---\n## A\n```sh\na\n```\n## B\n```sh\nb\n```\n",
        "Snippets/a.md",
    );
    assert!(matches!(
        select_block(&two, None),
        Err(RenderError::BlockNotFound(None))
    ));
    assert!(matches!(
        select_block(&two, Some(5)),
        Err(RenderError::BlockNotFound(Some(5)))
    ));
    assert_eq!(select_block(&two, Some(1)).unwrap().0.trim(), "b");
    let one = parse_from_content(
        "---\nartifactType: Snippet\n---\n## A\n```sh\na\n```\n",
        "Snippets/a.md",
    );
    assert_eq!(select_block(&one, None).unwrap().0.trim(), "a");
}

#[test]
fn score_is_linear() {
    let names: Vec<String> = (0..100_000).map(|i| format!("VK-n{i}")).collect();
    let p = Profile::new(names, &[]);
    let sets: Vec<Vec<ParsedVar>> = (0..20_000)
        .map(|i| vec![t(&format!("VK-n{i}"), "")])
        .collect();
    let start = Instant::now();
    for s in &sets {
        let r = score(&p, s, &[]);
        assert_eq!(r.matched_vars.len(), 1);
    }
    assert!(start.elapsed() < Duration::from_secs(2));
}
