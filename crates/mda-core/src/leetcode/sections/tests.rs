use super::tree::split_before;
use super::*;
use serde_json::json;

fn j<T: serde::Serialize>(t: &T) -> serde_json::Value {
    serde_json::to_value(t).unwrap()
}

#[test]
fn heading_statics_compile() {
    for r in [&*TESTS_RE, &*FINAL_TESTS_RE, &*EXAMPLES_RE] {
        assert!(r.as_str().starts_with("(?m)"));
    }
    let _ = (&*EXAMPLE_FENCE, &*INPUT_RE, &*OUTPUT_RE, &*CASE_FENCE);
}

#[test]
fn description_stops_at_heading_and_drops_config_fence() {
    assert_eq!(
        extract_description("Some prose.\n\n## Examples"),
        "Some prose."
    );
    assert_eq!(
        extract_description("Prose.\n\n```yaml leetcode\nreturns: int\n# c\n```\n\n## Examples"),
        "Prose."
    );
    assert_eq!(extract_description("Only prose\n"), "Only prose");
    // a fenced column-0 `#` is a comment, not a heading
    assert_eq!(
        extract_description("A\n```py\n# c\n```\nB\n# H"),
        "A\n```py\n# c\n```\nB"
    );
    assert_eq!(
        extract_description("#nospace is prose\n# H"),
        "#nospace is prose"
    );
}

#[test]
fn examples_need_input_and_output() {
    let b = "## Examples\n```example\ninput: x = 1\noutput: 1\n```\n```example\ninput: only\n```\n```example\r\ninput: y\r\noutput:   2 \r\n```";
    assert_eq!(
        j(&extract_examples(b)),
        json!([{"input": "x = 1", "output": "1"}, {"input": "y", "output": "2"}])
    );
    assert!(extract_examples("no section").is_empty());
}

#[test]
fn tests_json_yaml_and_final_do_not_swallow_each_other() {
    let b = "## Tests\n```json\n[{\"input\": {\"x\": 1.0, \"y\": 2.5}, \"expected\": 2}]\n```\n## Final Tests\n```yaml\n- input:\n    x: 9\n  expected: 81\n```\n";
    assert_eq!(
        j(&extract_tests(b)),
        json!([{"input": {"x": 1, "y": 2.5}, "expected": 2}])
    );
    assert_eq!(
        j(&extract_final_tests(b)),
        json!([{"input": {"x": 9}, "expected": 81}])
    );
    assert_eq!(extract_final_tests("## Tests\n```json\n[]\n```").len(), 0);
}

#[test]
fn case_fences_check_attr_config_fence_and_malformed() {
    let b = "## Tests\n```json check=\"app builds\"\n[{\"input\":{},\"expected\":1}]\n```\n```json check=api\n{bad\n```\n```yaml leetcode\nreturns: int\n```\n```json\n[]\n```\n";
    let f = extract_case_fences(b, &TESTS_RE);
    assert_eq!(f.len(), 3);
    assert_eq!(f[0].check.as_deref(), Some("app builds"));
    assert_eq!(f[0].cases.len(), 1);
    assert_eq!(f[1].check.as_deref(), Some("api"));
    assert!(f[1].cases.is_empty());
    assert_eq!(f[2].check, None);
    assert!(extract_case_fences("nothing", &TESTS_RE).is_empty());
}

#[test]
fn raw_keeps_non_input_elements_and_json_proto_key_is_data() {
    let b = "## Tests\n```json\n[{\"request\": {}, \"expect\": 1}, 5, {\"input\": {\"__proto__\": 1}}]\n```";
    let f = &extract_case_fences(b, &TESTS_RE)[0];
    assert_eq!(f.raw.len(), 3);
    assert_eq!(f.cases.len(), 2);
    assert_eq!(f.cases[1].input.0[0].0, "__proto__");
}

#[test]
fn big_numbers_follow_js() {
    let b =
        "## Tests\n```json\n[{\"input\":{\"a\":9007199254740993,\"b\":-0},\"expected\":1e2}]\n```";
    let t = &extract_tests(b)[0];
    assert_eq!(
        serde_json::to_string(&t.input).unwrap(),
        r#"{"a":9007199254740992,"b":0}"#
    );
    assert_eq!(t.expected, CaseValue::Int(100));
}

const TREE: &str = "# Setup\n\n## JavaScript\n```javascript\nfunction f() {}\n```\n```javascript\nsecond\n```\n## Python\nno fence\n\n# Solutions\n\n## Java\n```java\nA\n```\n<!-- meta: {\"solved_at\": \"t\", \"duration\": \"1s\"} -->\n```java\nB\n```\n## Python\n```python\nP\n```\n### Fast\n<!-- meta: {\"duration\": \"3m\"} -->\n```python\nQ  \n```\n### Empty\nnothing\n\n# Attempts\n\n## Java\n<!-- attempt: {\"at\": \"a\", \"duration\": \"1m\", \"passed\": true, \"bigO\": \"O(n)\"} -->\n```java\nx\n```\n<!-- attempt: {\"at\": \"a\", \"passed\": true} -->\n```java\nbad\n```\n```java\nbare\n```\n";

#[test]
fn setups_first_fence_per_language() {
    assert_eq!(
        j(&extract_setups(TREE)),
        json!([{"language": "javascript", "code": "function f() {}"}])
    );
}

#[test]
fn solutions_labels_numbering_and_meta() {
    assert_eq!(
        j(&extract_solutions(TREE)),
        json!([
            {"language": "java", "label": "Solution #1", "code": "A"},
            {"language": "java", "label": "Solution #2", "code": "B", "solvedAt": "t", "duration": "1s"},
            {"language": "python", "code": "P"},
            {"language": "python", "label": "Fast", "code": "Q", "duration": "3m"},
        ])
    );
}

#[test]
fn attempts_need_comment_and_required_fields() {
    assert_eq!(
        j(&extract_attempts(TREE)),
        json!([{"language": "java", "at": "a", "duration": "1m", "passed": true, "bigO": "O(n)", "code": "x"}])
    );
}

#[test]
fn split_before_is_the_lookahead_split() {
    let (lead, c) = split_before("pre\n## a\nx\n## b\n", "## ");
    assert_eq!((lead, c), ("pre\n", vec!["## a\nx\n", "## b\n"]));
    assert_eq!(split_before("", "## "), ("", vec![]));
    assert_eq!(split_before("## a", "## "), ("", vec!["## a"]));
    assert_eq!(split_before("x ## a\n", "## ").1.len(), 0);
}

#[test]
fn crlf_and_hostile_inputs_do_not_panic() {
    let b = "# Setup\r\n\r\n## Java\r\n```java\r\ncode\r\n```\r\n";
    assert_eq!(extract_setups(b)[0].code, "code");
    for s in [
        "# Setup\n## \n```",
        "# Solutions\n## é\n### \n```é",
        "## Tests\n```json\n[[[[",
        "## Examples\n```example\ninput:",
    ] {
        let _ = (
            extract_setups(s),
            extract_solutions(s),
            extract_tests(s),
            extract_examples(s),
            extract_attempts(s),
        );
    }
    let big = format!("# Solutions\n## J\n{}", "```java\nx\n```\n".repeat(20_000));
    assert_eq!(extract_solutions(&big).len(), 20_000);
}

/// The engine fields these helpers feed, checked against every exported case that has a body.
#[test]
fn conformance_cases_agree_on_section_fields() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/leetcode");
    let mut checked = 0;
    for group in ["sections", "section-bounds", "yaml-cases", "golden"] {
        let Ok(dir) = std::fs::read_dir(root.join(group)) else {
            continue;
        };
        for e in dir.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let src = crate::parse::decode(&std::fs::read(&path).unwrap());
            let want: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(path.with_extension("expected.json")).unwrap(),
            )
            .unwrap();
            // function exercises only: a project's suites bind to checks (T4.3)
            if want["leetcodeType"] != "function" {
                continue;
            }
            let (_, body) = crate::leetcode::config::split_frontmatter(&src);
            let got = json!({
                "description": extract_description(body),
                "examples": j(&extract_examples(body)),
                "tests": j(&extract_tests(body)),
                "finalTests": j(&extract_final_tests(body)),
                "setups": j(&tree::extract_setups(body)),
                "solutions": j(&tree::extract_solutions(body)),
                "attempts": j(&tree::extract_attempts(body)),
            });
            for (k, v) in got.as_object().unwrap() {
                assert_eq!(v, &want[k], "{} field {k}", path.display());
            }
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} cases");
}
