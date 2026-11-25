//! T4.3 unit tests for `checks:` and the `check=` binding: `project-parser.test.ts` (checks, binding,
//! unimplementedKinds), `project-checks.test.ts`, `project-build-check.test.ts`.

use crate::leetcode::ProjectCheck;
use crate::leetcode::project::tests::{codes, parse};

fn checks(entries: &[&str]) -> String {
    format!("test:\n  checks:\n{}", entries.join("\n"))
}

#[test]
fn each_kind_gets_its_own_fields() {
    let s = parse(
        &checks(&[
            "    - name: c\n      kind: call\n      file: a.ts\n      function: f",
            "    - name: b\n      kind: build\n      argv: [\"npx\", \"tsc\"]\n      dir: client",
            "    - name: d\n      kind: dom-assert\n      file: a.tsx\n      package: api",
            "    - name: s\n      kind: css-assert\n      file: a.tsx",
            "    - name: h\n      kind: http\n      package: \"api\"",
        ]),
        "",
    );
    assert_eq!(s.checks.len(), 5, "{:?}", s.warnings);
    let v = serde_json::to_value(&s.checks).unwrap();
    assert_eq!(v[0]["function"], "f");
    assert_eq!(v[1]["argv"], serde_json::json!(["npx", "tsc"]));
    assert_eq!(v[1]["dir"], "client");
    assert_eq!(v[2]["package"], "api");
    assert!(v[3].get("package").is_none());
    assert_eq!(v[4]["package"], "api");
    assert!(matches!(s.checks[4], ProjectCheck::Http(_)));
}

#[test]
fn dropped_checks_warn_with_their_codes() {
    let s = parse(
        &checks(&[
            "    - name: x\n      kind: telepathy",
            "    - name: y\n      kind: build\n      argv: rm -rf /",
            "    - name: z\n      kind: build\n      argv: [1]",
            "    - name: h\n      kind: http",
            "    - name: f\n      kind: call\n      function: g",
            "    - name: g\n      kind: call\n      file: a",
            "    - kind: build\n      argv: []",
            "    - name: p\n      kind: project",
        ]),
        "",
    );
    assert!(s.checks.is_empty());
    assert_eq!(
        codes(&s),
        [
            "leetcode.checks.unknown_kind",
            "leetcode.checks.build_needs_argv",
            "leetcode.checks.build_needs_argv",
            "leetcode.checks.http_needs_package",
            "leetcode.checks.needs_file",
            "leetcode.checks.call_needs_function",
            "leetcode.checks.unknown_kind",
        ]
    );
    assert!(s.unimplemented_kinds.is_empty());
}

#[test]
fn not_a_list_and_proto_field() {
    let s = parse("test:\n  checks: \"not a list\"", "");
    assert_eq!(codes(&s), ["leetcode.checks.not_a_list"]);
    assert_eq!(s.warnings[0].params["got"], "\"not a list\"");
    let s = parse(
        &checks(&["    - name: x\n      kind: build\n      argv: []\n      __proto__: polluted"]),
        "",
    );
    assert_eq!(s.checks.len(), 1);
}

#[test]
fn reserved_kinds_are_recorded_once_in_first_seen_order() {
    let s = parse(
        &checks(&[
            "    - name: app\n      kind: build\n      argv: []",
            "    - name: a\n      kind: class\n      file: x",
            "    - name: b\n      kind: program",
            "    - name: c\n      kind: class",
            "    - name: d\n      kind: in-place",
        ]),
        "",
    );
    assert_eq!(s.unimplemented_kinds, ["class", "program", "in-place"]);
    assert_eq!(s.checks.len(), 1);
    assert_eq!(s.warnings[0].code, "leetcode.checks.reserved_kind");
}

const CF: &str = "test:\n  checks:\n    - name: catalogue filter\n      kind: call\n      file: a.ts\n      function: f\n    - name: alternates\n      kind: dom-assert\n      file: a.ts";

#[test]
fn cases_bind_by_name_public_before_final() {
    let body = "## Tests\n```json check=\"catalogue filter\"\n[{\"input\":{\"a\":1},\"expected\":1},{\"input\":{\"a\":2},\"expected\":2}]\n```\n```json check=alternates\n[{\"input\":{\"a\":3},\"expected\":3}]\n```\n## Final Tests\n```json check=alternates\n[{\"input\":{\"a\":9},\"expected\":9}]\n```\n";
    let s = parse(CF, body);
    let got: Vec<_> = s
        .checks
        .iter()
        .map(|c| {
            let v = serde_json::to_value(c).unwrap();
            (
                v["cases"].as_array().unwrap().len(),
                v["publicCount"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(got, [(2, 2), (2, 1)]);
    // only hidden cases: publicCount 0
    let body = "## Final Tests\n```json check=alternates\n[{\"input\":{},\"expected\":1}]\n```\n";
    let v = serde_json::to_value(&parse(CF, body).checks[1]).unwrap();
    assert_eq!(
        (
            v["cases"].as_array().unwrap().len(),
            v["publicCount"].as_u64().unwrap()
        ),
        (1, 0)
    );
}

#[test]
fn bare_fence_binds_to_the_sole_check_else_warns() {
    let one = "test:\n  checks:\n    - name: only\n      kind: build\n      argv: []";
    let body = "## Tests\n```json\n[{\"input\":{},\"expected\":1}]\n```\n";
    let v = serde_json::to_value(&parse(one, body).checks[0]).unwrap();
    assert_eq!(v["cases"].as_array().unwrap().len(), 1);
    let s = parse(CF, body);
    assert_eq!(codes(&s), ["leetcode.checks.fence_unbound"]);
    let s = parse(
        CF,
        "## Tests\n```json check=ghost\n[{\"input\":{},\"expected\":1}]\n```\n",
    );
    assert_eq!(codes(&s), ["leetcode.checks.fence_unknown_check"]);
    assert_eq!(s.warnings[0].params["name"], "ghost");
}

#[test]
fn public_count_counts_cases_never_raw() {
    // an http-style element has no input/expected, still a raw element; `cases` is the engine view
    let one = "test:\n  checks:\n    - name: only\n      kind: build\n      argv: []";
    let body = "## Tests\n```json\n[{\"input\":{},\"expected\":1}, 5]\n```\n";
    let v = serde_json::to_value(&parse(one, body).checks[0]).unwrap();
    assert_eq!(v["publicCount"], v["cases"].as_array().unwrap().len());
}

#[test]
fn duplicate_names_bind_to_the_first() {
    let cfg = "checks:\n  - name: a\n    kind: build\n    argv: []\n  - name: a\n    kind: build\n    argv: [\"x\"]";
    let body = "## Tests\n```json check=a\n[{\"input\":{},\"expected\":1}]\n```\n";
    let s = parse(cfg, body);
    let n: Vec<_> = s
        .checks
        .iter()
        .map(|c| {
            serde_json::to_value(c).unwrap()["cases"]
                .as_array()
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(n, [1, 0]);
}
