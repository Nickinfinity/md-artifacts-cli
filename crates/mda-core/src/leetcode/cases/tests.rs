//! Parse half of TS `test/yaml-cases.test.ts` (Q-33), plus the Rust-only caps.

use serde_json::{Value, json};

use super::{MAX_NESTING, parse_yaml_cases};

fn p(raw: &str) -> Value {
    serde_json::to_value(parse_yaml_cases(raw)).unwrap()
}

fn expected_of(text: &str) -> Value {
    p(&format!("- expected: {text}"))[0]["expected"].clone()
}

#[test]
fn unquoted_scalars_take_json_types_only() {
    let typed = [
        ("true", json!(true)),
        ("false", json!(false)),
        ("null", Value::Null),
        ("~", Value::Null),
        ("0", json!(0)),
        ("-1", json!(-1)),
        ("3.5", json!(3.5)),
        ("1e3", json!(1000)),
        ("5.0", json!(5)),
        ("-0.5", json!(-0.5)),
    ];
    for (text, want) in typed {
        assert_eq!(expected_of(text), want, "{text}");
    }
    let strings = [
        "yes", "no", "on", "off", "y", "n", "Yes", "NO", "True", "FALSE", "NULL", "0051", "012",
        "1:1", "2:30", ".inf", ".nan", "0x1F", "1.0.0",
    ];
    for text in strings {
        assert_eq!(expected_of(text), json!(text), "{text}");
    }
}

#[test]
fn quoted_scalars_are_never_retyped() {
    for text in ["3", "true", "null", "0051", "no"] {
        assert_eq!(expected_of(&format!("\"{text}\"")), json!(text));
        assert_eq!(expected_of(&format!("'{text}'")), json!(text));
    }
}

#[test]
fn structure() {
    assert_eq!(
        p("- input:\n    arr: [1, -2, 0, 3]\n  expected: 3"),
        json!([{"input": {"arr": [1, -2, 0, 3]}, "expected": 3}])
    );
    assert_eq!(
        p("- input:\n    x: 1\n  expected: 2\n- input:\n    x: 3\n  expected: 4"),
        json!([{"input": {"x": 1}, "expected": 2}, {"input": {"x": 3}, "expected": 4}])
    );
    assert_eq!(
        p("- input:\n    a: [1, [2, [3]]]\n  expected: {k: [null, true, \"x\"]}"),
        json!([{"input": {"a": [1, [2, [3]]]}, "expected": {"k": [null, true, "x"]}}])
    );
    assert_eq!(p(""), json!([]));
    assert_eq!(p("\n\n  \n"), json!([]));
    assert_eq!(expected_of("\"a # b\"  # trailing"), json!("a # b"));
    let lf = "- input:\n    x: 1\n  expected: 2";
    assert_eq!(p(&lf.replace('\n', "\r\n")), p(lf));
    assert_eq!(expected_of("1:1"), json!("1:1"));
}

#[test]
fn declared_key_order_and_duplicates() {
    let v = parse_yaml_cases("- expected: 1\n  input: {b: 1, a: 2, b: 3}");
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        r#"[{"expected":1,"input":{"b":3,"a":2}}]"#
    );
}

#[test]
fn unquoted_comma_is_not_a_top_level_separator() {
    assert_eq!(
        p("- input:\n    line: 1,2,3,4,5\n  expected: 15"),
        json!([{"input": {"line": "1,2,3,4,5"}, "expected": 15}])
    );
    assert_eq!(
        p("- input:\n    s: hello, world\n  expected: 12"),
        json!([{"input": {"s": "hello, world"}, "expected": 12}])
    );
}

#[test]
fn malformed_input_degrades_never_panics() {
    for bad in ["- input: [1, 2", "- {unclosed: ", ": no key", "\t- tabbed"] {
        let _ = parse_yaml_cases(bad);
    }
    assert_eq!(p("- expected: 1\n   - odd"), Value::Null);
    assert_eq!(p("  - expected: 1\n- expected: 2"), Value::Null);
}

#[test]
fn proto_keys_never_land() {
    assert_eq!(
        p("- input:\n    __proto__: polluted\n  expected: 1"),
        json!([{"input": {}, "expected": 1}])
    );
    assert_eq!(
        p("- input: {__proto__: polluted}\n"),
        json!([{"input": {}}])
    );
    assert_eq!(
        p("- input: {constructor: 1, prototype: 2, k: 3}"),
        json!([{"input": {"k": 3}}])
    );
}

#[test]
fn big_scalar_parses_whole() {
    assert_eq!(
        expected_of(&"a".repeat(1_000_000)),
        json!("a".repeat(1_000_000))
    );
}

#[test]
fn hostile_100k_keys_is_linear() {
    let fence: String = (0..100_000).map(|n| format!("k{n}: 1\n")).collect();
    let t = std::time::Instant::now();
    let v = p(&fence);
    assert!(t.elapsed().as_secs() < 1, "{:?}", t.elapsed());
    assert_eq!(v.as_object().unwrap().len(), 100_000);
    let flow = format!(
        "- input: {{{}}}",
        (0..100_000)
            .map(|n| format!("k{n}: 1"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let t = std::time::Instant::now();
    let v = p(&flow);
    assert!(t.elapsed().as_secs() < 1, "{:?}", t.elapsed());
    assert_eq!(v[0]["input"].as_object().unwrap().len(), 100_000);
}

#[test]
fn array_index_keys_come_first_on_the_wire() {
    let v = parse_yaml_cases("- input: {b: 1, 2: x, 1: y}");
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        r#"[{"input":{"1":"y","2":"x","b":1}}]"#
    );
}

fn nested_flow(n: usize) -> String {
    format!("- expected: {}1{}", "[".repeat(n), "]".repeat(n))
}

#[test]
fn flow_nesting_cap_64_ok_65_null() {
    assert_eq!(MAX_NESTING, 64);
    assert_ne!(p(&nested_flow(64)), Value::Null);
    assert_eq!(p(&nested_flow(65)), Value::Null);
    assert_eq!(p(&nested_flow(10_000)), Value::Null);
    let maps = format!("- expected: {}1{}", "{a: ".repeat(65), "}".repeat(65));
    assert_eq!(p(&maps), Value::Null);
}

#[test]
fn block_nesting_cap() {
    let deep = |n: usize| -> String {
        (0..n)
            .map(|d| format!("{}k: ", " ".repeat(d)) + "\n")
            .collect::<String>()
            + &" ".repeat(n)
            + "k: 1"
    };
    // deep(n) nests n + 1 block maps.
    assert_ne!(p(&deep(63)), Value::Null);
    assert_eq!(p(&deep(64)), Value::Null);
}

#[test]
fn numbers_follow_js() {
    let wire =
        |t: &str| serde_json::to_string(&parse_yaml_cases(&format!("- expected: {t}"))).unwrap();
    for (t, w) in [
        ("5.0", "5"),
        ("2.5", "2.5"),
        ("-0", "0"),
        ("9007199254740992", "9007199254740992"),
        ("9007199254740994", "9007199254740994"),
        ("12345678901234567890", "12345678901234567000"),
    ] {
        assert_eq!(wire(t), format!(r#"[{{"expected":{w}}}]"#), "{t}");
    }
}
