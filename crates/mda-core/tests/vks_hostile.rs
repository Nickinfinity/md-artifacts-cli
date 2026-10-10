//! `vks` §9.1 rejection table: one case per code and per param value. Every rejection is an `Err`
//! (the sink: no partial `Vec` exists), carrying exactly the documented params.
#![allow(clippy::expect_used, clippy::panic)] // reason: assertion helpers outside #[test] fns; a panic is the failure signal
mod common;

use mda_core::error::*;
use mda_core::vks::{read_fence, read_section};

/// Assert `body` is rejected with `code`, `line`, and exactly the `extra` params.
fn case(body: &str, code: &str, line: usize, extra: &[(&str, &str)]) {
    let e = read_fence(body).expect_err(&format!("accepted {body:?}"));
    assert_eq!(e.code, code, "{body:?}: {e:?}");
    assert_eq!(
        e.params.get("line"),
        Some(&line.to_string()),
        "{body:?}: {e:?}"
    );
    for (k, v) in extra {
        assert_eq!(
            e.params.get(*k).map(String::as_str),
            Some(*v),
            "{body:?}: {e:?}"
        );
    }
    assert_eq!(e.params.len(), 1 + extra.len(), "{body:?}: {e:?}");
}

fn syntax(body: &str, line: usize, rule: SyntaxRule) {
    case(body, VKS_SYNTAX, line, &[("rule", rule.as_str())]);
}

fn unsupported(body: &str, line: usize, c: Construct) {
    case(body, VKS_UNSUPPORTED, line, &[("construct", c.as_str())]);
}

fn limit(body: &str, line: usize, l: Limit, max: usize) {
    case(
        body,
        VKS_LIMIT,
        line,
        &[("limit", l.as_str()), ("max", &max.to_string())],
    );
}

#[test]
fn syntax_rules() {
    syntax("VK-a: 1\n- a", 2, SyntaxRule::DashAtColumn0);
    syntax("VK-a:\n- a", 2, SyntaxRule::DashAtColumn0);
    syntax("VK-a: 1\n  b: 2", 2, SyntaxRule::ValueAndChildren);
    syntax("VK-a: \"abc", 1, SyntaxRule::UnclosedQuote);
    syntax("VK-a: 1\nVK-b: 'abc", 2, SyntaxRule::UnclosedQuote);
    syntax("VK-a: \"\\u001b\"", 1, SyntaxRule::BadEscape);
    for v in [
        "@x", "`x", "a: b", "x:", "%x", "]x", "}x", ",x", "?x", "- x",
    ] {
        syntax(&format!("VK-a: {v}"), 1, SyntaxRule::BadPlain);
    }
    syntax("VK-a: 1\njust text", 2, SyntaxRule::ExpectedEntry);
    syntax("VK-a: \"x\" y", 1, SyntaxRule::ExpectedEntry);
    syntax("VK-a: 'x'y", 1, SyntaxRule::ExpectedEntry);
    syntax("VK-a:\n  -", 2, SyntaxRule::ExpectedEntry);
    syntax("VK-a:\n  - x\n  y", 3, SyntaxRule::ExpectedEntry);
}

#[test]
fn indent_rules() {
    case("VK-a:\n\tb: 1", VKS_INDENT, 2, &[]);
    case("VK-a:\n \tb: 1", VKS_INDENT, 2, &[]);
    case("VK-a:\n   b: 1", VKS_INDENT, 2, &[]);
    case("VK-a:\n    b: 1", VKS_INDENT, 2, &[]);
    case("VK-a:\n  k:\n  - a", VKS_INDENT, 3, &[]);
    case(
        "VK-a:\n  b: 1\n    c: 2",
        VKS_SYNTAX,
        3,
        &[("rule", "value_and_children")],
    );
    case("VK-a:\n  - x\n   - y", VKS_INDENT, 3, &[]);
}

#[test]
fn key_rules() {
    case(
        "VK-a: 1\n__proto__: x",
        VKS_BAD_KEY,
        2,
        &[("key", "__proto__")],
    );
    case("VK-a: 1\nVK-ñ: x", VKS_BAD_KEY, 2, &[("key", "VK-ñ")]);
    case("VK-a:\n  my-key: x", VKS_BAD_KEY, 2, &[("key", "my-key")]);
    case("VK-a: 1\n: x", VKS_BAD_KEY, 2, &[("key", "")]);
    assert!(read_fence("VK-a:\n  constructor: x").is_ok());
    assert!(read_fence("VK-a-b: x").is_ok(), "top-level dash is legal");
    // line 1 of a fence is the classifier's: it is legacy, skipped (TS parity), never an error
    assert_eq!(read_fence("VK-ñ: x").unwrap(), vec![]);
}

#[test]
fn duplicate_keys() {
    case("VK-a: 1\nVK-a: 2", VKS_DUPLICATE_KEY, 2, &[("key", "VK-a")]);
    case(
        "VK-a:\n  x: 1\n  x: 2",
        VKS_DUPLICATE_KEY,
        3,
        &[("key", "x")],
    );
    case(
        "VK-a:\n  - x: 1\n    x: 2",
        VKS_DUPLICATE_KEY,
        3,
        &[("key", "x")],
    );
}

#[test]
fn unsupported_constructs() {
    use Construct::*;
    for (v, c) in [
        ("{a: b}", FlowMap),
        ("{}", FlowMap),
        ("[x, y]", FlowList),
        ("&x v", Anchor),
        ("*x", Alias),
        ("!!str v", Tag),
        (">", Folded),
        ("|+", BlockIndicator),
        ("|2", BlockIndicator),
    ] {
        unsupported(&format!("VK-z: 1\nVK-a: {v}"), 2, c);
    }
    unsupported("VK-z: 1\n<<: *x", 2, MergeKey);
    unsupported("VK-z: 1\n? k", 2, ComplexKey);
    unsupported("VK-z: 1\n---", 2, DocumentMarker);
    unsupported("VK-z: 1\n...", 2, DocumentMarker);
    unsupported("VK-z: 1\n%YAML 1.2", 2, Directive);
    unsupported("VK-a:\n  - |", 2, BlockInList);
    unsupported("VK-z: 1\nVK-a:\n  - - a", 3, NestedList);
    unsupported("VK-a:\n  - &x v", 2, Anchor);
}

#[test]
fn mixed_lists_and_comments() {
    case("VK-a:\n  - x\n  - k: v", VKS_MIXED_LIST, 3, &[]);
    case("VK-a:\n  - k: v\n  - x", VKS_MIXED_LIST, 3, &[]);
    case("VK-a: a # y", VKS_INLINE_COMMENT, 1, &[]);
    case("VK-a:\n  color: #fff", VKS_INLINE_COMMENT, 2, &[]);
    case("VK-a: \"a\" # y", VKS_INLINE_COMMENT, 1, &[]);
    case("VK-a:\n  - x # y", VKS_INLINE_COMMENT, 2, &[]);
    case("VK-a:\n  - #a: b\n    c: d", VKS_INLINE_COMMENT, 2, &[]);
    case("VK-a:\n  - #a: b", VKS_INLINE_COMMENT, 2, &[]);
    case("VK-a: 1\nVK-b=2", VKS_MIXED_DIALECT, 2, &[]);
}

#[test]
fn control_chars_in_yaml() {
    case("VK-a: 1\nVK-b: x\x1b", VKS_CONTROL_CHAR, 2, &[]);
    case("VK-a: |\n  x\x01y", VKS_CONTROL_CHAR, 2, &[]);
    case("VK-a: \"a\x1bb\"", VKS_CONTROL_CHAR, 1, &[]);
    case("VK-a: 1\n# c\x7f", VKS_CONTROL_CHAR, 2, &[]);
    // first violation in line order wins, whichever kind
    case(
        "VK-a: 1\nbad line\nVK-b: \x1b",
        VKS_SYNTAX,
        2,
        &[("rule", "expected_entry")],
    );
    case("VK-a: 1\nVK-b: \x1b\nbad line", VKS_CONTROL_CHAR, 2, &[]);
    assert!(
        read_fence("VK-a: x\ty").is_ok(),
        "TAB inside a value is fine"
    );
}

#[test]
fn limits() {
    let big = format!("VK-a: {}", "x".repeat(65_537 - 6));
    assert_eq!(big.len(), 65_537);
    limit(&big, 0, Limit::BodyBytes, 65_536);
    assert!(read_fence(&big[1..]).is_ok(), "exactly 64 KiB passes");
    let legacy_big = format!("K={}", "x".repeat(65_537));
    limit(&legacy_big, 0, Limit::BodyBytes, 65_536);
    assert_eq!(
        read_section(&legacy_big).unwrap_err().params["limit"],
        "body_bytes"
    );

    let nest = |records: usize| {
        let mut s = String::from("VK-a:\n");
        for i in 1..=records {
            s.push_str(&format!("{}a:\n", "  ".repeat(i)));
        }
        s
    };
    assert!(read_fence(&nest(6)).is_ok());
    limit(&nest(7), 8, Limit::Depth, 6);

    let nodes: String = (0..200)
        .map(|i| format!("VK-k{i}:\n{}", "  - x\n".repeat(51)))
        .collect();
    assert!(nodes.len() < 64 * 1024);
    limit(&nodes, 10_001, Limit::Nodes, 10_000);

    let items = format!("VK-a:\n{}", "  - x\n".repeat(1_001));
    limit(&items, 1_002, Limit::ListItems, 1_000);
    assert!(read_fence(&format!("VK-a:\n{}", "  - x\n".repeat(1_000))).is_ok());

    let keys = |n: usize| (1..=n).map(|i| format!("VK-k{i}: 1\n")).collect::<String>();
    limit(&keys(256), 256, Limit::MapKeys, 255);
    assert_eq!(read_fence(&keys(255)).unwrap().len(), 255);
    let rec_keys: String = (1..=256).map(|i| format!("  k{i}: 1\n")).collect();
    limit(&format!("VK-a:\n{rec_keys}"), 257, Limit::MapKeys, 255);
}

#[test]
fn comments_are_linear() {
    let body = format!("VK-a: 1\n{}", "# c\n".repeat(10_000));
    let v = read_fence(&body).unwrap();
    assert_eq!((v.len(), v[0].name.as_str()), (1, "VK-a"));
}

#[test]
fn crlf_equals_lf() {
    let lf = "VK-a:\n  - id: 1\n    n: x\nVK-b: |\n  l\n";
    assert_eq!(
        read_fence(&lf.replace('\n', "\r\n")).unwrap(),
        read_fence(lf).unwrap()
    );
    let lf = "VK-a: 1\nVK-b: two\nVK-c:\n";
    assert_eq!(
        read_fence(&lf.replace('\n', "\r\n")).unwrap(),
        read_fence(lf).unwrap()
    );
    assert_eq!(
        read_fence("VK-a: x   \nVK-b:   y\t\n").unwrap()[0]
            .value
            .default_value(),
        "x"
    );
}

#[test]
fn fuzz_all_inputs() {
    for b in common::fuzz_strings(0x5EED, 10_000, 40) {
        let _ = read_fence(&b);
        let _ = read_section(&b);
    }
    for b in common::fuzz_strings(7, 5_000, 40) {
        let _ = read_fence(&format!("VK-a:\n  - {b}"));
    }
}
