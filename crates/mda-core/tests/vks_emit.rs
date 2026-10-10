//! The YAML vks emitter and the model-side `check_vars` (spec §9.5, W-10).
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)] // reason: assertion helpers outside #[test] fns; a panic is the failure signal

mod common;
use mda_core::error::{
    EmitReason, VKS_BAD_KEY, VKS_CONTROL_CHAR, VKS_DUPLICATE_KEY, VKS_LIMIT, VarsError,
};
use mda_core::model::ParsedVar;
use mda_core::vks::{VksList, VksRecord, VksValue, check_vars, emit_body, read_fence};

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

fn var(name: &str, value: VksValue) -> ParsedVar {
    ParsedVar {
        name: name.into(),
        value,
    }
}

fn emit1(value: VksValue) -> String {
    emit_body(&[var("VK-a", value)]).expect("emit")
}

fn reason(value: VksValue) -> EmitReason {
    emit_body(&[var("VK-a", value)])
        .expect_err("must refuse")
        .reason
}

#[test]
fn plain_string() {
    assert_eq!(
        emit_body(&[ParsedVar::text("VK-a", "x")]).unwrap(),
        "VK-a: x\n"
    );
}

#[test]
fn string_rule_table() {
    for (input, want) in [
        ("", "VK-a:\n"),
        ("1", "VK-a: '1'\n"),
        ("True", "VK-a: 'True'\n"),
        ("NULL", "VK-a: 'NULL'\n"),
        ("~", "VK-a: '~'\n"),
        (" x", "VK-a: ' x'\n"),
        ("x ", "VK-a: 'x '\n"),
        ("a: b", "VK-a: 'a: b'\n"),
        ("a #b", "VK-a: 'a #b'\n"),
        ("it's", "VK-a: it's\n"),
        ("'q", "VK-a: '''q'\n"),
        ("a\nb", "VK-a: |-\n  a\n  b\n"),
        ("a\n", "VK-a: |\n  a\n"),
        ("a\n\nb", "VK-a: |-\n  a\n\n  b\n"),
        ("a\n \nb", "VK-a: \"a\\n \\nb\"\n"),
        ("\n", "VK-a: \"\\n\"\n"),
        ("t\tx", "VK-a: t\tx\n"),
    ] {
        assert_eq!(emit1(s(input)), want, "{input:?}");
    }
}

#[test]
fn refusals() {
    assert_eq!(reason(s("a\n\n")), EmitReason::TrailingNewlines);
    assert_eq!(reason(s("x```y")), EmitReason::Backticks);
    assert_eq!(reason(strs(&["ok", "x```y"])), EmitReason::Backticks);
    assert_eq!(
        reason(VksValue::Record(VksRecord::default())),
        EmitReason::EmptyRecord
    );
    let nested = VksValue::Record(rec(&[("a", VksValue::Record(VksRecord::default()))]));
    assert_eq!(reason(nested), EmitReason::EmptyRecord);
    let e = emit_body(&[var(
        "VK-z",
        VksValue::List(VksList::Records(vec![VksRecord::default()])),
    )])
    .unwrap_err();
    assert_eq!(
        (e.var.as_str(), e.reason),
        ("VK-z", EmitReason::EmptyRecord)
    );
}

#[test]
fn lists_and_records() {
    assert_eq!(emit1(strs(&[])), "VK-a: []\n");
    assert_eq!(
        emit1(VksValue::List(VksList::Records(vec![]))),
        "VK-a: []\n"
    );
    assert_eq!(
        emit1(strs(&["x", "", "a\nb", "-", "---", "1", "# c"])),
        "VK-a:\n  - x\n  - ''\n  - \"a\\nb\"\n  - '-'\n  - '---'\n  - '1'\n  - '# c'\n"
    );
    let users = VksValue::List(VksList::Records(vec![
        rec(&[("id", s("x")), ("tags", strs(&["a"]))]),
        rec(&[("id", s("y")), ("tags", strs(&[]))]),
    ]));
    assert_eq!(
        emit1(users),
        "VK-a:\n  - id: x\n    tags:\n      - a\n  - id: y\n    tags: []\n"
    );
    let db = VksValue::Record(rec(&[
        ("host", s("h")),
        ("in", VksValue::Record(rec(&[("k", s("v"))]))),
    ]));
    assert_eq!(emit1(db), "VK-a:\n  host: h\n  in:\n    k: v\n");
}

#[test]
fn spec_example_round_trips() {
    let body = "VK-env:\n  - dev\n  - prod\nVK-users:\n  - id: 1\n    name: Alice Smith\n    tags:\n      - core\n      - ops\n  - id: 2\n    name: Bob Jones\n    tags: []\nVK-db:\n  host: localhost\n  port: \"5432\"\n";
    let vars = read_fence(body).unwrap();
    assert_eq!(read_fence(&emit_body(&vars).unwrap()).unwrap(), vars);
}

/// Strings the readers define as errors (controls other than `\n`/`\t`, JS line separators).
fn skipped(x: &str) -> bool {
    x.chars().any(|c| {
        (c < ' ' && c != '\n' && c != '\t') || c == '\u{7f}' || c == '\u{2028}' || c == '\u{2029}'
    })
}

/// The DRY guard between emitter and reader: whatever the emitter accepts, the reader returns.
fn assert_round_trip(v: &ParsedVar, ctx: &str) {
    match emit_body(std::slice::from_ref(v)) {
        Ok(b) => assert_eq!(read_fence(&b).as_ref(), Ok(&vec![v.clone()]), "{ctx} {b:?}"),
        Err(e) => assert!(
            matches!(
                e.reason,
                EmitReason::Backticks | EmitReason::TrailingNewlines
            ),
            "{ctx} {e:?}"
        ),
    }
}

#[test]
fn consistency_property() {
    for x in common::fuzz_strings(0xE417, 5_000, 30)
        .iter()
        .filter(|x| !skipped(x))
    {
        assert_round_trip(&var("VK-a", s(x)), "str");
        assert_round_trip(&var("VK-a", strs(&[x])), "item");
        let r = rec(&[("k", s(x)), ("l", strs(&[x]))]);
        assert_round_trip(&var("VK-a", VksValue::Record(r.clone())), "field");
        assert_round_trip(
            &var("VK-a", VksValue::List(VksList::Records(vec![r]))),
            "rec item",
        );
    }
}

fn err_of(vars: &[ParsedVar]) -> VarsError {
    check_vars(vars).expect_err("must refuse")
}

fn assert_err(vars: &[ParsedVar], code: &str, var_name: &str) {
    let e = err_of(vars);
    assert_eq!(e.code, code, "{e:?}");
    assert_eq!(e.params["line"], "0");
    assert_eq!(e.params["var"], var_name);
}

#[test]
fn check_vars_keys() {
    for bad in ["VK-ñ", "__proto__", "my.key", "", "1a"] {
        assert_err(&[var(bad, s("x"))], VKS_BAD_KEY, bad);
    }
    assert_err(
        &[var("VK-a", s("x")), var("VK-a", s("y"))],
        VKS_DUPLICATE_KEY,
        "VK-a",
    );
    // nested keys: no dash allowed
    let nested = VksValue::Record(rec(&[("a-b", s("x"))]));
    assert_err(&[var("VK-a", nested)], VKS_BAD_KEY, "VK-a");
    let dup = VksValue::Record(rec(&[("a", s("x")), ("a", s("y"))]));
    assert_err(&[var("VK-a", dup)], VKS_DUPLICATE_KEY, "VK-a");
    assert!(check_vars(&[var("VK-a", s("a\nb")), var("VK-b", s("\t"))]).is_ok());
}

#[test]
fn check_vars_limits() {
    let deep = (0..7).fold(s("x"), |v, _| VksValue::Record(rec(&[("a", v)])));
    let e = err_of(&[var("VK-a", deep)]);
    assert_eq!((e.code, e.params["limit"].as_str()), (VKS_LIMIT, "depth"));
    let ok6 = (0..6).fold(s("x"), |v, _| VksValue::Record(rec(&[("a", v)])));
    assert!(check_vars(&[var("VK-a", ok6)]).is_ok());

    let items: Vec<String> = (0..1001).map(|i| i.to_string()).collect();
    let big = VksValue::List(VksList::Strings(items));
    assert_eq!(err_of(&[var("VK-a", big)]).params["limit"], "list_items");

    let keys: Vec<(String, VksValue)> = (0..256).map(|i| (format!("k{i}"), s("x"))).collect();
    assert_eq!(
        err_of(&[var("VK-a", VksValue::Record(VksRecord(keys)))]).params["limit"],
        "map_keys"
    );

    let many: Vec<ParsedVar> = (0..256).map(|i| var(&format!("VK-{i}"), s("x"))).collect();
    let e = err_of(&many);
    assert_eq!(
        (
            e.code,
            e.params["limit"].as_str(),
            e.params["line"].as_str()
        ),
        (VKS_LIMIT, "map_keys", "0")
    );

    // 11 lists of 1000 strings: over 10 000 nodes in total
    let lists: Vec<ParsedVar> = (0..11)
        .map(|i| {
            var(
                &format!("VK-{i}"),
                VksValue::List(VksList::Strings(vec!["x".into(); 1000])),
            )
        })
        .collect();
    assert_eq!(err_of(&lists).params["limit"], "nodes");
}

#[test]
fn check_vars_control_chars() {
    for bad in ["\x1b", "\r", "a\u{2028}b", "a\u{2029}", "\x7f", "\0"] {
        assert_err(&[var("VK-a", s(bad))], VKS_CONTROL_CHAR, "VK-a");
        assert_err(&[var("VK-a", strs(&[bad]))], VKS_CONTROL_CHAR, "VK-a");
    }
    let key = VksValue::Record(VksRecord(vec![("a\x1b".into(), s("x"))]));
    assert!(check_vars(&[var("VK-a", key)]).is_err());
}

#[test]
fn record_deserialize() {
    assert!(serde_json::from_str::<VksValue>(r#"{"a":"1","a":"2"}"#).is_err());
    assert_eq!(
        serde_json::from_str::<VksValue>("[]").unwrap(),
        VksValue::List(VksList::Strings(vec![]))
    );
    assert!(serde_json::from_str::<VksValue>(r#"["a",{"b":"c"}]"#).is_err());
}

#[test]
fn fuzz_no_panic() {
    for x in common::fuzz_strings(0xE417, 10_000, 40) {
        let _ = emit_body(&[var("VK-a", s(&x)), var("VK-b", strs(&[&x]))]);
        let _ = check_vars(&[var(&x, s(&x))]);
    }
}
