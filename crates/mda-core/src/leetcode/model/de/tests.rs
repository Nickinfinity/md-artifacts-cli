use serde_json::{Value, json};

use crate::leetcode::{CaseValue, LeetCodeExercise, ProjectCheck};

/// `warnings` / `unimplementedKinds` are derived and ignored on input (Q-71): strip both sides.
fn strip(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("warnings");
        o.remove("unimplementedKinds");
    }
    v
}

fn expected_files() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/leetcode");
    let mut out = vec![];
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().ends_with(".expected.json") {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn every_conformance_exercise_round_trips_at_json_level() {
    let files = expected_files();
    assert!(files.len() > 70, "{}", files.len());
    for p in files {
        let j: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let x: LeetCodeExercise =
            serde_json::from_value(j.clone()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert!(x.warnings.is_empty() && x.unimplemented_kinds.is_empty());
        assert_eq!(
            strip(serde_json::to_value(&x).unwrap()),
            strip(j),
            "{}",
            p.display()
        );
    }
}

#[test]
fn unknown_exercise_field_is_refused() {
    let mut j = serde_json::to_value(LeetCodeExercise::default()).unwrap();
    j["bogus"] = json!(1);
    assert!(serde_json::from_value::<LeetCodeExercise>(j).is_err());
}

#[test]
fn derived_fields_are_accepted_and_ignored() {
    let mut j = serde_json::to_value(LeetCodeExercise::default()).unwrap();
    j["warnings"] = json!([{ "code": "leetcode.type.unknown", "params": {} }, 7]);
    j["unimplementedKinds"] = json!(["render"]);
    let x: LeetCodeExercise = serde_json::from_value(j).unwrap();
    assert!(x.warnings.is_empty() && x.unimplemented_kinds.is_empty());
}

#[test]
fn case_values_keep_the_int_float_rule() {
    let v: CaseValue =
        serde_json::from_value(json!([5, 2.5, 10000000000000000000u64, {"b": 1, "a": 2}])).unwrap();
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        r#"[5,2.5,10000000000000000000,{"b":1,"a":2}]"#
    );
}

#[test]
fn a_check_takes_exactly_its_kind_fields() {
    let ok = json!({"name": "b", "kind": "build", "argv": ["make"], "cases": [], "publicCount": 0, "dir": "web"});
    let c: ProjectCheck = serde_json::from_value(ok.clone()).unwrap();
    assert_eq!(serde_json::to_value(&c).unwrap(), ok);
    for bad in [
        json!({"name": "b", "kind": "build", "argv": ["make"], "function": "f", "cases": [], "publicCount": 0}),
        json!({"name": "c", "kind": "call", "file": "a.ts", "cases": [], "publicCount": 0}),
        json!({"name": "x", "kind": "render", "cases": [], "publicCount": 0}),
        json!({"name": "h", "kind": "http", "package": "api", "cases": [], "publicCount": 0, "extra": 1}),
    ] {
        assert!(
            serde_json::from_value::<ProjectCheck>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}
