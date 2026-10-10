//! `vks` codec read side: dialects, typing, block scalars, serde shape (spec §9).
#![allow(clippy::expect_used, clippy::panic)] // reason: assertion helpers outside #[test] fns; a panic is the failure signal

mod common;
use mda_core::error::{VKS_CONTROL_CHAR, VarsError};
use mda_core::model::ParsedVar;
use mda_core::vks::{Dialect, VksList, VksRecord, VksValue, classify, read_fence, read_section};

fn s(x: &str) -> VksValue {
    VksValue::Str(x.into())
}

fn rec(fields: &[(&str, VksValue)]) -> VksRecord {
    VksRecord(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn strs(items: &[&str]) -> VksValue {
    VksValue::List(VksList::Strings(
        items.iter().map(|x| (*x).to_owned()).collect(),
    ))
}

/// `(name, value)` pairs of a fence read, or panic with the error.
fn read(body: &str) -> Vec<(String, VksValue)> {
    read_fence(body)
        .unwrap_or_else(|e| panic!("{body:?}: {e:?}"))
        .into_iter()
        .map(|v| (v.name, v.value))
        .collect()
}

fn one(body: &str) -> VksValue {
    let mut v = read(body);
    assert_eq!(v.len(), 1, "{body:?}");
    v.remove(0).1
}

fn err(body: &str) -> VarsError {
    read_fence(body).expect_err(body)
}

#[test]
fn t1_structured_users() {
    let got =
        read_fence("VK-users:\n  - id: 1\n    name: \"Alice Smith\"\n    tags:\n      - core\n")
            .unwrap();
    let want = vec![ParsedVar {
        name: "VK-users".into(),
        value: VksValue::List(VksList::Records(vec![rec(&[
            ("id", s("1")),
            ("name", s("Alice Smith")),
            ("tags", strs(&["core"])),
        ])])),
    }];
    assert_eq!(got, want);
}

#[test]
fn t1_spec_example() {
    let body = "VK-env:\n  - dev\n  - prod\nVK-users:\n  - id: 1\n    name: Alice Smith\n    tags:\n      - core\n      - ops\n  - id: 2\n    name: Bob Jones\n    tags: []\nVK-db:\n  host: localhost\n  port: \"5432\"\n";
    let v = read(body);
    assert_eq!(v[0], ("VK-env".into(), strs(&["dev", "prod"])));
    assert_eq!(
        v[2].1,
        VksValue::Record(rec(&[("host", s("localhost")), ("port", s("5432"))]))
    );
    let VksValue::List(VksList::Records(users)) = &v[1].1 else {
        panic!("users")
    };
    assert_eq!(users.len(), 2);
    assert_eq!(users[1].0[2], ("tags".into(), strs(&[])));
}

#[test]
fn t2_classify() {
    assert_eq!(classify("# c\n\nVK-a: 1"), Dialect::Yaml);
    assert_eq!(classify("VK-a:b=c"), Dialect::Legacy);
    assert_eq!(classify("KEY=value: x"), Dialect::Legacy);
    assert_eq!(classify("VK-url: http://a=b"), Dialect::Yaml);
    assert_eq!(classify("VK-a:"), Dialect::Yaml);
    assert_eq!(classify(""), Dialect::Legacy);
    assert_eq!(classify("  VK-a: 1"), Dialect::Legacy);
    assert_eq!(classify("1a: x"), Dialect::Legacy);
    assert_eq!(
        classify("VK-a:\u{85}x"),
        Dialect::Legacy,
        "U+0085 is not JS whitespace"
    );
}

#[test]
fn t3_typing() {
    assert_eq!(one("VK-value=\"active\""), s("\"active\""));
    assert_eq!(one("VK-value: \"active\""), s("active"));
    for (raw, want) in [
        ("1", "1"),
        ("true", "true"),
        ("~", "~"),
        ("null", "null"),
        ("99.95", "99.95"),
    ] {
        assert_eq!(one(&format!("VK-n: {raw}")), s(want));
    }
    assert_eq!(one("VK-n:"), s(""));
    assert_eq!(one("VK-n: []"), strs(&[]));
    assert_eq!(one("VK-n: 'it''s'"), s("it's"));
    assert_eq!(one("VK-n: \"a\\nb\\t\\\\\\\"\""), s("a\nb\t\\\""));
}

#[test]
fn t4_legacy_pins() {
    let v = read("# c\nVK-a = x=y \n=nameless\nno equals\nVK-b=1\nVK-b=2\n  # =hash\n");
    assert_eq!(
        v,
        vec![
            ("VK-a".into(), s("x=y")),
            ("VK-b".into(), s("1")),
            ("VK-b".into(), s("2"))
        ]
    );
    assert_eq!(read_section("VK-a: 1").unwrap(), vec![]);
    assert_eq!(read_section("VK-a=\"q\"").unwrap()[0].value, s("\"q\""));
}

#[test]
fn t5_legacy_control_chars() {
    let e = err("VK-a=\u{1b}x");
    assert_eq!((e.code, e.params["line"].as_str()), (VKS_CONTROL_CHAR, "1"));
    let e = err("VK-a=ok\nVK-b=\u{7f}");
    assert_eq!((e.code, e.params["line"].as_str()), (VKS_CONTROL_CHAR, "2"));
    assert_eq!(read("VK-a=a\tb"), vec![("VK-a".into(), s("a\tb"))]);
    assert_eq!(
        read("VK-a=ok\r\nVK-b=2\r\n").len(),
        2,
        "CRLF is never flagged"
    );
    let e = read_section("# c \u{1}\nnoequals").expect_err("section");
    assert_eq!((e.code, e.params["line"].as_str()), (VKS_CONTROL_CHAR, "1"));
    assert!(read_fence("VK-a=1\u{b}").is_err());
}

#[test]
fn t6_default_value_and_list_eq() {
    assert_eq!(s("a").default_value(), "a");
    assert_eq!(strs(&["x", "y"]).default_value(), "x");
    assert_eq!(strs(&[]).default_value(), "");
    assert_eq!(
        VksValue::List(VksList::Records(vec![rec(&[("a", s("1"))])])).default_value(),
        ""
    );
    assert_eq!(VksValue::Record(rec(&[("a", s("1"))])).default_value(), "");
    assert_eq!(VksList::Strings(vec![]), VksList::Records(vec![]));
    assert_ne!(VksList::Strings(vec!["a".into()]), VksList::Records(vec![]));
    assert_ne!(VksList::Strings(vec![]), VksList::Records(vec![rec(&[])]));
}

#[test]
fn t7_block_scalars() {
    assert_eq!(one("VK-a: |\n  l1\n  l2\n\n\n"), s("l1\nl2\n"));
    assert_eq!(one("VK-a: |-\n  l1\n  l2\n\n"), s("l1\nl2"));
    assert_eq!(
        one("VK-a: |\n  # not a comment\n  x"),
        s("# not a comment\nx\n")
    );
    let v = read("VK-a: |\nVK-b: x");
    assert_eq!(v, vec![("VK-a".into(), s("")), ("VK-b".into(), s("x"))]);
    assert_eq!(one("VK-a: |\n  a\n    b\n"), s("a\n  b\n"));
    assert_eq!(one("VK-a: |-\n  a\n\n  b"), s("a\n\nb"));
    let v = one("VK-r:\n  t: |\n    x\n    y\n  u: 1");
    assert_eq!(
        v,
        VksValue::Record(rec(&[("t", s("x\ny\n")), ("u", s("1"))]))
    );
}

#[test]
fn t8_serde_shape() {
    let v = ParsedVar {
        name: "VK-r".into(),
        value: VksValue::Record(rec(&[("zeta", s("1")), ("alpha", s("2"))])),
    };
    let j = serde_json::to_string(&v).unwrap();
    assert!(j.contains(r#""value":{"zeta":"1","alpha":"2"}"#), "{j}");
    assert!(j.contains(r#""defaultValue":"""#), "{j}");
    let j = serde_json::to_string(&ParsedVar::text("VK-a", "x")).unwrap();
    assert!(!j.contains(r#""value""#), "{j}");
}

#[test]
fn compact_records_and_comments() {
    let v = one("# top\nVK-a:\n  # inner\n  - k: 1\n    # between\n    m: 2\n\n  - k: 3\n");
    let VksValue::List(VksList::Records(rs)) = v else {
        panic!("records")
    };
    assert_eq!(
        rs,
        vec![rec(&[("k", s("1")), ("m", s("2"))]), rec(&[("k", s("3"))])]
    );
    assert_eq!(
        one("VK-a:\n  - 'x: y'\n  - \"a # b\"\n  - http://z\n"),
        strs(&["x: y", "a # b", "http://z"])
    );
    assert_eq!(one("VK-a: a=b  "), s("a=b"));
}

#[test]
fn fuzz_never_panics() {
    for b in common::fuzz_strings(0x5EED, 10_000, 40) {
        let _ = read_fence(&b);
        let _ = read_section(&b);
        let _ = read_fence(&format!("VK-a: 1\n{b}"));
    }
}
