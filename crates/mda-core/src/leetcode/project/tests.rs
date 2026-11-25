//! T4.3 unit tests: `project-parser.test.ts`, `project-libs-multi.test.ts` and the conformance
//! cross-check (files, libs, solutionFiles, checks, warnings, unimplementedKinds).

use super::*;
use crate::leetcode::{FileRole, ProjectCheck};
use crate::render::Warning;

pub(crate) fn parse(cfg: &str, body: &str) -> ProjectSections {
    parse_project_artifact(cfg, body)
}
pub(crate) fn codes(s: &ProjectSections) -> Vec<&'static str> {
    s.warnings.iter().map(|w| w.code).collect()
}
fn param<'a>(w: &'a Warning, k: &str) -> &'a str {
    w.params.get(k).map_or("", String::as_str)
}

const FILES_SECTION: &str = "## Files\n\n```typescript path=src/lib/catalogue.ts role=editable\nexport function filterInStock() { return []; }\n```\n\n```typescript path=src/app/api/route.ts role=readonly\nexport const GET = () => null;\n```\n\n```css path=src/app/globals.css\nbody { margin: 0; }\n```\n";

const FM: &str = "libs:\n  typescript:\n    - react@^19.0.0\n    - \"@types/react@^19.0.0\"\ntest:\n  type: project\n  checks:\n    - name: catalogue filter\n      kind: call\n      file: src/lib/catalogue.ts\n      function: filterInStock\n    - name: app builds\n      kind: build\n      dir: client\n      argv: [\"npx\", \"tsc\", \"--noEmit\"]";

#[test]
fn every_path_fence_is_a_file_with_role_and_language() {
    let f = parse(FM, FILES_SECTION).files;
    let paths: Vec<_> = f.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "src/lib/catalogue.ts",
            "src/app/api/route.ts",
            "src/app/globals.css"
        ]
    );
    let roles: Vec<_> = f.iter().map(|x| x.role).collect();
    assert_eq!(
        roles,
        [FileRole::Editable, FileRole::Readonly, FileRole::Editable]
    );
    let langs: Vec<_> = f.iter().map(|x| x.language.as_str()).collect();
    assert_eq!(langs, ["typescript", "typescript", "css"]);
    assert_eq!(f[2].content, "body { margin: 0; }\n");
}

#[test]
fn a_fence_without_path_is_not_a_file() {
    let s = parse("", "## Files\n\n```typescript\nnot a file\n```\n");
    assert!(s.files.is_empty() && s.warnings.is_empty());
}

#[test]
fn unknown_role_is_editable_with_a_warning() {
    let s = parse(
        "",
        "## Files\n\n```typescript path=a.ts role=sudo\nx\n```\n",
    );
    assert_eq!(s.files[0].role, FileRole::Editable);
    assert_eq!(codes(&s), ["leetcode.files.unknown_role"]);
    assert_eq!(param(&s.warnings[0], "role"), "sudo");
}

#[test]
fn quoted_path_and_empty_role_and_crlf() {
    let body = "## Files\r\n\r\n```ts path=\"a b.ts\" role=\"\"\r\nx\r\n```\r\n";
    let s = parse("", body);
    assert_eq!(s.files[0].path, "a b.ts");
    assert_eq!(param(&s.warnings[0], "role"), "");
    // `xpath=` also matches the unanchored TS `path=` regex
    let s = parse("", "## Files\n```ts xpath=a.ts\nx\n```\n");
    assert_eq!(s.files[0].path, "a.ts");
    // an empty quoted path is no file
    assert!(
        parse("", "## Files\n```ts path=\"\"\nx\n```\n")
            .files
            .is_empty()
    );
}

#[test]
fn traversal_path_is_kept_for_the_writer() {
    let s = parse(
        "",
        "## Files\n\n```typescript path=../../etc/passwd\nx\n```\n",
    );
    assert_eq!(s.files[0].path, "../../etc/passwd");
}

#[test]
fn files_section_ends_at_a_heading_outside_a_fence() {
    let body =
        "## Files\n```ts path=a.ts\n## not a heading\n```\n## Other\n```ts path=b.ts\ny\n```\n";
    let s = parse("", body);
    assert_eq!(s.files.len(), 1);
}

#[test]
fn solution_files_are_path_fences_of_the_top_level_solutions_section() {
    let body = "# Solutions\n\n## Python\n```python path=a.py\nx\n```\n```python\nplain\n```\n# Attempts\n```ts path=z.ts\nq\n```\n";
    let s = parse("", body);
    assert_eq!(s.solution_files.len(), 1);
    assert_eq!(s.solution_files[0].path, "a.py");
    assert!(s.files.is_empty());
}

#[test]
fn libs_parse_per_language_and_drop_bad_specs() {
    let s = parse(FM, FILES_SECTION);
    let want = vec![(
        "typescript".to_owned(),
        vec![
            "react@^19.0.0".to_owned(),
            "@types/react@^19.0.0".to_owned(),
        ],
    )];
    assert_eq!(s.libs.0, want);
    let s = parse(
        "libs:\n  typescript:\n    - react@^19.0.0\n    - --target=/etc",
        "",
    );
    assert_eq!(s.libs.0[0].1, ["react@^19.0.0"]);
    assert_eq!(codes(&s), ["leetcode.libs.rejected"]);
    assert_eq!(param(&s.warnings[0], "specs"), "--target=/etc");
    assert_eq!(param(&s.warnings[0], "ecosystem"), "pnpm");
    assert!(
        parse("libs:\n  typescript:\n    - ../../x", "")
            .libs
            .0
            .is_empty()
    );
}

#[test]
fn libs_proto_key_never_lands_and_no_registry_warns() {
    let s = parse("libs:\n  __proto__:\n    - react@^19.0.0", "");
    assert!(s.libs.0.is_empty() && s.warnings.is_empty());
    let s = parse("libs:\n  cobol: [x]", "");
    assert_eq!(codes(&s), ["leetcode.libs.no_registry"]);
    assert_eq!(param(&s.warnings[0], "language"), "cobol");
}

#[test]
fn libs_inline_lists_respect_quotes_and_alias_keys() {
    let s = parse(
        "libs:\n  python3: [\"numpy>=2,<3\", pandas]\n  js: [lodash]",
        "",
    );
    assert_eq!(
        s.libs.0[0],
        (
            "python".to_owned(),
            vec!["numpy>=2,<3".to_owned(), "pandas".to_owned()]
        )
    );
    assert_eq!(s.libs.0[1].0, "javascript");
}

#[test]
fn libs_rejected_specs_are_joined_and_each_truncated() {
    let long = "!".repeat(300);
    let s = parse(&format!("libs:\n  python: [a, {long}, b!, c!]"), "");
    let specs = param(&s.warnings[0], "specs");
    assert!(specs.chars().count() <= 200);
    let s = parse("libs:\n  java: [guava, \"a:b\"]", "");
    assert_eq!(param(&s.warnings[0], "specs"), "guava, a:b");
}

#[test]
fn libs_duplicate_language_keeps_first_position_last_value() {
    let s = parse(
        "libs:\n  python: [numpy]\n  rust: [serde]\n  python: [pandas]",
        "",
    );
    assert_eq!(
        s.libs.0[0],
        ("python".to_owned(), vec!["pandas".to_owned()])
    );
    assert_eq!(s.libs.0.len(), 2);
}

#[test]
fn near_miss_key_suggests_first_known_within_two_edits() {
    let s = parse("packagse:\n  - name: api", "");
    assert_eq!(codes(&s), ["leetcode.config.unknown_key"]);
    assert_eq!(param(&s.warnings[0], "key"), "packagse");
    assert_eq!(param(&s.warnings[0], "suggestion"), "packages");
    assert!(
        parse("source: https://example.com\n", "")
            .warnings
            .is_empty()
    );
    assert!(parse("title: x\n  packagse: 1", "").warnings.is_empty());
    // short keys match almost anything: reproduced, not fixed (first in array order)
    let s = parse("te:", "");
    assert_eq!(param(&s.warnings[0], "suggestion"), "test");
}

#[test]
fn packages_warnings_and_list_come_through() {
    let s = parse("packages: nope", "");
    assert_eq!(codes(&s), ["leetcode.packages.not_a_list"]);
    let s = parse(
        "packages:\n  - name: api\n    dir: s\n    install: [\"i\"]\n    start: [\"node\", \"x.js\"]",
        "",
    );
    assert_eq!(s.packages.len(), 1);
}

#[test]
fn warning_order_is_near_miss_checks_binding_packages_files_libs_solutions() {
    let cfg = "titel: x\nlibs:\n  cobol: [x]\npackages: nope\nchecks:\n  - name: a\n    kind: build\n    argv: []\n  - name: b\n    kind: build\n    argv: []";
    let body = "## Tests\n```json\n[{\"input\":{},\"expected\":1}]\n```\n## Files\n```ts path=a role=x\n```\n# Solutions\n```ts path=b role=y\n```\n";
    let s = parse(cfg, body);
    assert_eq!(
        codes(&s),
        [
            "leetcode.config.unknown_key",
            "leetcode.checks.fence_unbound",
            "leetcode.packages.not_a_list",
            "leetcode.files.unknown_role",
            "leetcode.libs.no_registry",
            "leetcode.files.unknown_role",
        ]
    );
}

fn sink(s: &ProjectSections) -> usize {
    s.checks.len() + s.files.len() + s.warnings.len() + s.libs.0.len()
}

#[test]
fn hostile_inputs_are_linear_and_bounded() {
    let t = std::time::Instant::now();
    // 100 000 checks, all reserved: one warning each, one deduped kind, no quadratic scan
    let mut cfg = String::from("checks:\n");
    for i in 0..100_000 {
        cfg.push_str(&format!("  - name: n{i}\n    kind: class\n"));
    }
    let s = parse(&cfg, "");
    assert_eq!(s.warnings.len(), 100_000);
    assert_eq!(s.unimplemented_kinds, ["class"]);
    // 100 000 valid checks and 100 000 fences binding to the last one by name
    let mut cfg = String::from("checks:\n");
    let mut body = String::from("## Tests\n");
    for i in 0..100_000 {
        cfg.push_str(&format!("  - name: n{i}\n    kind: build\n    argv: []\n"));
    }
    for _ in 0..1_000 {
        body.push_str("```json check=n99999\n[{\"input\":{},\"expected\":1}]\n```\n");
    }
    let s = parse(&cfg, &body);
    assert_eq!(s.checks.len(), 100_000);
    // 100 000 files, 100 000 lib keys, 100 000 unknown near-miss keys
    let body: String = (0..100_000)
        .map(|i| format!("```ts path=f{i}\n```\n"))
        .collect();
    let s = parse("", &format!("## Files\n{body}"));
    assert_eq!(s.files.len(), 100_000);
    let libs: String = (0..100_000).map(|i| format!("  cobol{i}: [x]\n")).collect();
    let s = parse(&format!("libs:\n{libs}"), "");
    assert_eq!(s.warnings.len(), 100_000);
    let keys: String = (0..100_000).map(|i| format!("titl{i}: 1\n")).collect();
    assert!(sink(&parse(&keys, "")) < 200_000);
    // a 1 MiB key and a 1 MiB info string: no panic, bounded
    let big = "a".repeat(1 << 20);
    let s = parse(
        &format!("{big}: 1\nlibs:\n  {big}: [x]"),
        &format!("## Files\n```{big} path={big}\n```\n"),
    );
    assert!(
        s.warnings
            .iter()
            .all(|w| w.params.values().all(|v| v.chars().count() <= 200))
    );
    assert!(t.elapsed().as_secs() < 20, "took {:?}", t.elapsed());
}

fn cross(id: &str, dir: &str) {
    use crate::leetcode::config::{
        extract_config_blocks, split_frontmatter, without_body_set_keys,
    };
    let root = format!(
        "{}/../../conformance/leetcode/{dir}/{id}",
        env!("CARGO_MANIFEST_DIR")
    );
    let md = std::fs::read_to_string(format!("{root}.md")).unwrap();
    let want: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{root}.expected.json")).unwrap())
            .unwrap();
    let (fm, body) = split_frontmatter(&md);
    let cfg = extract_config_blocks(body);
    let raw = format!("{}\n{}", without_body_set_keys(fm), cfg.raw);
    let s = parse_project_artifact(&raw, body);
    let got = serde_json::json!({
        "files": s.files, "libs": s.libs, "solutionFiles": s.solution_files, "checks": s.checks,
        "warnings": s.warnings, "unimplementedKinds": s.unimplemented_kinds,
    });
    for k in [
        "files",
        "libs",
        "solutionFiles",
        "checks",
        "warnings",
        "unimplementedKinds",
    ] {
        assert_eq!(got[k], want[k], "{id}: {k}");
    }
}

#[test]
fn conformance_project_cases() {
    for id in [
        "lc-project-parser-checks-kinds",
        "lc-project-parser-checks-not-a-list",
        "lc-project-parser-fence-binding",
        "lc-project-parser-files-and-call-check",
        "lc-project-parser-libs-rejected",
        "lc-project-parser-near-miss-key",
        "lc-project-parser-unknown-role",
    ] {
        cross(id, "project-parser");
    }
    cross("lc-lib-spec-libs-bad-maven", "lib-spec");
    cross("lc-lib-spec-libs-ecosystems", "lib-spec");
    cross("lc-shape-package-files-only", "shape");
}

#[test]
fn edit_distance_is_levenshtein() {
    for (a, b, d) in [
        ("titel", "title", 2),
        ("packagse", "packages", 2),
        ("ab", "ab", 0),
        ("kitten", "sitting", 3),
        ("te", "test", 2),
        ("abc", "xyz", 3),
    ] {
        assert_eq!(edit_distance(a, b), d, "{a} {b}");
    }
}

#[test]
fn a_check_kind_alone_is_typed() {
    let s = parse(FM, FILES_SECTION);
    assert!(matches!(s.checks[0], ProjectCheck::Call(_)));
}
