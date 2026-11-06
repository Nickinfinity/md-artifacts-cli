use super::*;
use crate::parse::text::decode;

fn st(raw: &str, s: LeetCodeStatus) -> String {
    patch_status(raw, s).unwrap()
}
fn att(code: &str) -> AttemptEntry {
    AttemptEntry {
        at: "2026-07-10T14:32:00Z".into(),
        duration: "8m22s".into(),
        passed: true,
        big_o: None,
        confidence: None,
        code: code.into(),
    }
}
const AT: &str =
    "<!-- attempt: {\"at\":\"2026-07-10T14:32:00Z\",\"duration\":\"8m22s\",\"passed\":true} -->";

// ── status ─────────────────────────────────────────────────────────────────

#[test]
fn status_inserted_after_difficulty() {
    let raw = "---\ntitle: X\ndifficulty: easy\nalgorithm: a\ncustom: 1\n---\nbody\n";
    assert_eq!(
        st(raw, LeetCodeStatus::Solved),
        "---\ntitle: X\ndifficulty: easy\nstatus: solved\nalgorithm: a\ncustom: 1\n---\nbody\n"
    );
}

#[test]
fn status_replaced_in_place_all_duplicates() {
    let raw = "---\nstatus: unsolved\ntitle: X\nstatus: attempted\n---\n";
    assert_eq!(
        st(raw, LeetCodeStatus::Solved),
        "---\nstatus: solved\ntitle: X\nstatus: solved\n---\n"
    );
}

#[test]
fn status_appended_when_nothing_outranks_and_unknown_keys_skipped() {
    assert_eq!(
        st("---\ncustom: 1\n---\n", LeetCodeStatus::Attempted),
        "---\ncustom: 1\nstatus: attempted\n---\n"
    );
    // `statusX:` is another key, `tags` outranks status.
    assert_eq!(
        st("---\nstatusX: 1\ntags: [a]\n---\n", LeetCodeStatus::Solved),
        "---\nstatusX: 1\nstatus: solved\ntags: [a]\n---\n"
    );
}

#[test]
fn status_keeps_crlf() {
    let raw = "---\r\ntitle: X\r\nstatus: unsolved\r\n---\r\nb\r\n";
    assert_eq!(
        st(raw, LeetCodeStatus::Solved),
        "---\r\ntitle: X\r\nstatus: solved\r\n---\r\nb\r\n"
    );
    let raw = "---\r\ntitle: X\r\ntags: [a]\r\n---\r\n";
    assert_eq!(
        st(raw, LeetCodeStatus::Solved),
        "---\r\ntitle: X\r\nstatus: solved\r\ntags: [a]\r\n---\r\n"
    );
    assert_eq!(
        st("---\r\ntitle: X\r\n---\r\n", LeetCodeStatus::Solved),
        "---\r\ntitle: X\r\nstatus: solved\r\n---\r\n"
    );
}

#[test]
fn status_decoded_bom_is_dropped() {
    let raw = decode(b"\xEF\xBB\xBF---\ntitle: X\n---\n");
    assert_eq!(
        st(&raw, LeetCodeStatus::Solved),
        "---\ntitle: X\nstatus: solved\n---\n"
    );
}

#[test]
fn status_nested_key_untouched() {
    let raw = "---\npractice:\n  status: x\ntags: [a]\n---\n";
    assert_eq!(
        st(raw, LeetCodeStatus::Solved),
        "---\npractice:\n  status: x\nstatus: solved\ntags: [a]\n---\n"
    );
    let raw = "---\nstatus : x\ntags: [a]\n---\n";
    assert_eq!(
        st(raw, LeetCodeStatus::Solved),
        "---\nstatus : x\nstatus: solved\ntags: [a]\n---\n"
    );
}

#[test]
fn status_empty_frontmatter_inserted() {
    assert_eq!(
        st("---\n---\nb\n", LeetCodeStatus::Solved),
        "---\nstatus: solved\n---\nb\n"
    );
    assert_eq!(
        st("---\r\n---\r\n", LeetCodeStatus::Attempted),
        "---\r\nstatus: attempted\r\n---\r\n"
    );
}

#[test]
fn status_empty_frontmatter_closer_is_exact() {
    // TS `/^---$/m`: `----` is a body line, so the status line is appended after `x: 1`.
    assert_eq!(
        st("---\n----\nx: 1\n---\n", LeetCodeStatus::Solved),
        "---\n----\nx: 1\nstatus: solved\n---\n"
    );
    for raw in ["---\n---x\n", "---\n--- \n"] {
        assert_eq!(
            patch_status(raw, LeetCodeStatus::Solved),
            Err(Why::Frontmatter)
        );
    }
}

#[test]
fn status_without_frontmatter_refused() {
    for raw in ["body\n", "---\nstatus: x\n", ""] {
        assert_eq!(
            patch_status(raw, LeetCodeStatus::Solved),
            Err(Why::Frontmatter)
        );
    }
}

// ── solved meta ────────────────────────────────────────────────────────────

const SOL: &str = "# Setup\n```python\nsetup\n```\n# Solutions\n\n## Python\n```python title=x\nskip\n```\n```python\nreal\n```\n# Attempts\n```python\nno\n```\n";

#[test]
fn meta_targets_first_bare_fence_inside_solutions_resolved() {
    let out = insert_solved_meta(SOL, "py", "T", "5m").unwrap();
    let want =
        "<!-- meta: { \"solved_at\": \"T\", \"duration\": \"5m\" } -->\n```python\nreal\n```\n";
    assert_eq!(out, SOL.replace("```python\nreal\n```\n", want));
}

#[test]
fn second_solve_replaces_meta() {
    let once = insert_solved_meta(SOL, "python", "T1", "5m").unwrap();
    let twice = insert_solved_meta(&once, "python", "T2", "7m").unwrap();
    assert_eq!(twice.matches("<!-- meta:").count(), 1);
    assert!(twice.contains("{ \"solved_at\": \"T2\", \"duration\": \"7m\" }"));
    assert_eq!(twice.replace("T2", "T1").replace("7m", "5m"), once);
}

#[test]
fn meta_no_duration_is_noop_and_no_fence_refused() {
    assert_eq!(insert_solved_meta(SOL, "python", "T", "").unwrap(), SOL);
    assert_eq!(insert_solved_meta(SOL, "java", "T", "1m"), Err(Why::Meta));
    assert_eq!(
        insert_solved_meta("# Setup\n```python\nx\n```\n", "python", "T", "1m"),
        Err(Why::Meta)
    );
    // Only an attributed fence inside Solutions.
    assert_eq!(
        insert_solved_meta("# Solutions\n```python x\n```\n", "python", "T", "1m"),
        Err(Why::Meta)
    );
}

#[test]
fn meta_crlf_and_json_escaping() {
    let raw = "# Solutions\r\n```python\r\nx\r\n```\r\n";
    let out = insert_solved_meta(raw, "python", "T", "a\"b\\c\n# Solutions").unwrap();
    assert!(out.starts_with("# Solutions\r\n<!-- meta: { \"solved_at\": \"T\", \"duration\": \"a\\\"b\\\\c\\n# Solutions\" } -->\r\n```python\r\n"));
    // The injected heading text never becomes a line.
    assert_eq!(out.lines().filter(|l| *l == "# Solutions").count(), 1);
}

#[test]
fn meta_hostile_refused() {
    for (s, d) in [("T --> x", "1m"), ("T", "1m-->")] {
        assert_eq!(insert_solved_meta(SOL, "python", s, d), Err(Why::Meta));
    }
    assert_eq!(
        insert_solved_meta(SOL, "py\n# Setup", "T", "1m"),
        Err(Why::Other)
    );
}

// ── attempts ───────────────────────────────────────────────────────────────

#[test]
fn attempt_creates_section_after_solutions() {
    let raw = "# Solutions\n\n## Java\n```java\nA\n```\n\n# Notes\ntail\n";
    let out = prepend_attempt(raw, "python", &att("def f(): ...\n\n")).unwrap();
    assert_eq!(
        out,
        format!(
            "# Solutions\n\n## Java\n```java\nA\n```\n\n# Attempts\n\n## python\n{AT}\n```python\ndef f(): ...\n```\n\n# Notes\ntail\n"
        )
    );
}

#[test]
fn attempt_without_solutions_goes_to_end() {
    let out = prepend_attempt("# Setup\ntext", "js", &att("x")).unwrap();
    assert_eq!(
        out,
        format!("# Setup\ntext\n\n# Attempts\n\n## javascript\n{AT}\n```javascript\nx\n```\n")
    );
}

#[test]
fn attempt_prepended_newest_first_under_aliased_heading() {
    let first = prepend_attempt("# Solutions\n", "javascript", &att("one")).unwrap();
    let raw = first.replace("## javascript", "## JS");
    let out = prepend_attempt(&raw, "javascript", &att("two")).unwrap();
    assert_eq!(out.matches("## ").count(), 1);
    assert!(out.find("two").unwrap() < out.find("one").unwrap());
    assert!(
        out.contains("```\n\n<!-- attempt:"),
        "blank line between entries: {out}"
    );
}

#[test]
fn attempt_new_language_chunk_and_empty_chunk() {
    let raw = format!("# Attempts\n\n## Java\n{AT}\n```java\nj\n```\n");
    let out = prepend_attempt(&raw, "python", &att("p")).unwrap();
    assert_eq!(out, format!("{raw}\n## python\n{AT}\n```python\np\n```\n"));
    let out = prepend_attempt("# Attempts\n## python\n# Next\n", "python", &att("p")).unwrap();
    assert_eq!(
        out,
        format!("# Attempts\n## python\n{AT}\n```python\np\n```\n# Next\n")
    );
}

#[test]
fn attempt_optional_fields_and_order() {
    let mut a = att("x");
    a.big_o = Some("O(n)".into());
    a.confidence = Some("high".into());
    let out = prepend_attempt("", "python", &a).unwrap();
    assert!(out.contains("\"passed\":true,\"bigO\":\"O(n)\",\"confidence\":\"high\"}"));
}

#[test]
fn attempt_heading_inside_fence_is_not_a_chunk() {
    let raw = "# Attempts\n\n## Java\n<!-- attempt: {} -->\n```java\n## python\n```\n";
    let out = prepend_attempt(raw, "python", &att("p")).unwrap();
    assert!(out.ends_with(&format!("```\n\n## python\n{AT}\n```python\np\n```\n")));
}

#[test]
fn attempt_crlf_uses_file_eol() {
    let out = prepend_attempt("# Solutions\r\n", "python", &att("a\nb")).unwrap();
    assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
}

#[test]
fn attempt_hostile_refused() {
    for code in ["a\n```\nb", "x ``` y", "````"] {
        assert_eq!(
            prepend_attempt("# Attempts\n", "python", &att(code)),
            Err(Why::Fence)
        );
    }
    for f in 0..4 {
        let mut a = att("x");
        let bad = "end --> # Solutions".to_owned();
        match f {
            0 => a.at = bad,
            1 => a.duration = bad,
            2 => a.big_o = Some(bad),
            _ => a.confidence = Some(bad),
        }
        assert_eq!(prepend_attempt("", "python", &a), Err(Why::Meta));
    }
    assert_eq!(prepend_attempt("", "a`b", &att("x")), Err(Why::Other));
}

#[test]
fn attempt_duration_newline_cannot_forge_heading() {
    let mut a = att("x");
    a.duration = "1m\n# Solutions".into();
    let out = prepend_attempt("# Solutions\n", "python", &a).unwrap();
    assert_eq!(out.lines().filter(|l| *l == "# Solutions").count(), 1);
}

// ── language ───────────────────────────────────────────────────────────────

#[test]
fn language_validated_after_resolve() {
    assert_eq!(validate_language("py").unwrap(), "python");
    assert_eq!(validate_language("c++").unwrap(), "cpp");
    assert_eq!(validate_language("a_1").unwrap(), "a_1");
    for bad in ["a\nb", "a`b", "# x", "a b", &"x".repeat(33)] {
        assert_eq!(validate_language(bad), Err(BadField::Language), "{bad:?}");
    }
}
