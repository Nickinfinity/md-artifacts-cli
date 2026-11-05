#![allow(clippy::unwrap_used)] // helper `v` is not a #[test] fn, so clippy.toml's test exemption misses it

//! Wire shape of the LeetCode model: `to_value` against hand-written, TS-shaped JSON (D-41).
//! Spec line numbers are `spec/LEETCODE_FILE_FORMAT.md`; TS lines are the extension's working tree.

use mda_core::leetcode::*;
use serde_json::{Value, json};

fn v<T: serde::Serialize>(t: T) -> Value {
    serde_json::to_value(t).unwrap()
}

fn base(name: &str) -> CheckBase {
    CheckBase {
        name: name.into(),
        cases: vec![],
        public_count: 1,
    }
}

#[test]
fn default_exercise_json() {
    // Hand-written, never built from `Default`. Every required TS field present, every optional
    // (functions, algorithm, files, libs, solutionFiles, checks, packages, program) absent.
    let expected = json!({
        "title": "",                       // spec:185, TS parser.service:91
        "leetcodeType": "function",        // spec:225 (absent => single-buffer default), derived
        "difficulty": "easy",              // spec:186
        "functionName": "",                // spec:457, TS parser.service:94
        "status": "unsolved",              // spec:187
        "params": [],                      // spec:459 (`params` default `[]`)
        "returns": "",                     // spec:460, TS parser.service:99
        "description": "",                 // TS parser.service:100 (extractDescription of empty body)
        "examples": [],                    // spec:574 (no `## Examples`)
        "tests": [],                       // spec:588
        "finalTests": [],                  // spec:588, TS types:`finalTests` raw `[]`
        "test": {"type": "call", "timeoutMs": 5000},   // spec:354 (`function` = TS `call`), spec:355
        "setups": [],                      // spec:703
        "practice": {                      // spec:421-423
            "options": ["noCompletion", "noAiAgents"],
            "timeLimitMinutes": 0,
            "locked": false
        },
        "solutions": [],                   // spec:710
        "attempts": [],                    // spec:718
        "tags": [],                        // spec:189
        "warnings": [],                    // D-24: always an array
        "unimplementedKinds": []           // D-24: always an array
    });
    assert_eq!(v(LeetCodeExercise::default()), expected);
}

#[test]
fn full_exercise_carries_every_optional() {
    let e = LeetCodeExercise {
        leetcode_type: LeetcodeType::Stack,
        functions: Some(OrderedMap(vec![("python".to_owned(), "f".to_owned())])),
        algorithm: Some("dp".into()),
        files: Some(vec![]),
        libs: Some(OrderedMap(vec![(
            "python".to_owned(),
            vec!["numpy".to_owned()],
        )])),
        solution_files: Some(vec![]),
        checks: Some(vec![]),
        packages: Some(vec![]),
        program: Some(ProgramConfig {
            channel: ProgramChannel::Flags,
            entry: Some("m.py".into()),
            flags: Some(vec!["a".into()]),
        }),
        warnings: vec![mda_core::render::Warning {
            code: "leetcode.x",
            params: [("k".to_owned(), "v".to_owned())].into(),
        }],
        unimplemented_kinds: vec!["class".into()],
        ..LeetCodeExercise::default()
    };
    let j = v(e);
    assert_eq!(j["leetcodeType"], "stack");
    assert_eq!(j["functions"], json!({"python": "f"}));
    assert_eq!(j["algorithm"], "dp");
    assert_eq!(j["libs"], json!({"python": ["numpy"]}));
    assert_eq!(
        j["program"],
        json!({"channel": "flags", "entry": "m.py", "flags": ["a"]})
    );
    assert_eq!(
        j["warnings"],
        json!([{"code": "leetcode.x", "params": {"k": "v"}}])
    );
    assert_eq!(j["unimplementedKinds"], json!(["class"]));
    for k in ["files", "solutionFiles", "checks", "packages"] {
        assert_eq!(j[k], json!([]), "{k}");
    }
}

#[test]
fn test_type_ids_are_ts_spellings() {
    let all = [
        (TestTypeId::Call, "call"),
        (TestTypeId::Program, "program"),
        (TestTypeId::Http, "http"),
        (TestTypeId::Build, "build"),
        (TestTypeId::DomAssert, "dom-assert"),
        (TestTypeId::CssAssert, "css-assert"),
        (TestTypeId::Class, "class"),
        (TestTypeId::InPlace, "in-place"),
    ];
    for (id, s) in all {
        assert_eq!(v(id), s);
    }
}

#[test]
fn practice_uses_wire_name_time_limit_minutes() {
    let p = PracticeConfig {
        options: vec![
            PracticeOptionId::NoSnippets,
            PracticeOptionId::NoParameterHints,
        ],
        time_limit_minutes: 30,
        locked: true,
    };
    assert_eq!(
        v(p),
        json!({"options": ["noSnippets", "noParameterHints"], "timeLimitMinutes": 30, "locked": true})
    );
}

#[test]
fn test_config_has_no_raw_type() {
    assert_eq!(
        v(TestConfig {
            r#type: TestTypeId::Program,
            timeout_ms: 100
        }),
        json!({"type": "program", "timeoutMs": 100})
    );
}

#[test]
fn case_keeps_ints_floats_and_nesting() {
    let c = TestCase {
        input: OrderedMap(vec![(
            "xs".to_owned(),
            CaseValue::List(vec![
                CaseValue::Int(1),
                CaseValue::Float(2.5),
                CaseValue::Null,
            ]),
        )]),
        expected: CaseValue::Object(OrderedMap(vec![
            ("ok".to_owned(), CaseValue::Bool(true)),
            ("s".to_owned(), CaseValue::Str("a".into())),
        ])),
    };
    assert_eq!(
        v(c),
        json!({"input": {"xs": [1, 2.5, null]}, "expected": {"ok": true, "s": "a"}})
    );
}

#[test]
fn body_types_match_ts_samples() {
    assert_eq!(
        v(Example {
            input: "a".into(),
            output: "b".into()
        }),
        json!({"input": "a", "output": "b"})
    );
    assert_eq!(
        v(ParamDef {
            name: "n".into(),
            r#type: "int".into()
        }),
        json!({"name": "n", "type": "int"})
    );
    assert_eq!(
        v(ExerciseSetup {
            language: "rust".into(),
            code: "c".into()
        }),
        json!({"language": "rust", "code": "c"})
    );
    let s = LeetCodeSolution {
        language: "java".into(),
        label: Some("hm".into()),
        code: "c".into(),
        solved_at: Some("t".into()),
        duration: Some("1s".into()),
    };
    assert_eq!(
        v(s),
        json!({"language": "java", "label": "hm", "code": "c", "solvedAt": "t", "duration": "1s"})
    );
    let a = Attempt {
        language: "java".into(),
        at: "t".into(),
        duration: "8m22s".into(),
        passed: false,
        big_o: Some("O(n)".into()),
        confidence: Some("high".into()),
        code: "c".into(),
    };
    assert_eq!(
        v(a),
        json!({"language": "java", "at": "t", "duration": "8m22s", "passed": false, "bigO": "O(n)", "confidence": "high", "code": "c"})
    );
    let e = AttemptEntry {
        at: "t".into(),
        duration: "1s".into(),
        passed: true,
        big_o: None,
        confidence: None,
        code: "c".into(),
    };
    assert_eq!(
        v(e),
        json!({"at": "t", "duration": "1s", "passed": true, "code": "c"})
    );
}

#[test]
fn summary_matches_ts_sample() {
    let s = LeetCodeSummary {
        title: "T".into(),
        leetcode_type: Some(LeetcodeType::Package),
        difficulty: LeetCodeDifficulty::Hard,
        status: LeetCodeStatus::Attempted,
        algorithm: Some("dp".into()),
        tags: vec!["a".into()],
    };
    assert_eq!(
        v(s),
        json!({"title": "T", "leetcodeType": "package", "difficulty": "hard", "status": "attempted", "algorithm": "dp", "tags": ["a"]})
    );
    let bare = LeetCodeSummary {
        title: "".into(),
        leetcode_type: None,
        difficulty: LeetCodeDifficulty::Easy,
        status: LeetCodeStatus::Solved,
        algorithm: None,
        tags: vec![],
    };
    assert_eq!(
        v(bare),
        json!({"title": "", "difficulty": "easy", "status": "solved", "tags": []})
    );
}

#[test]
fn project_check_tagged_by_kind() {
    let case = TestCase {
        input: OrderedMap(vec![]),
        expected: CaseValue::Int(1),
    };
    let mut b = base("c");
    b.cases.push(case);
    let call = ProjectCheck::Call(FunctionCheck {
        base: b,
        file: "a.py".into(),
        function: "f".into(),
    });
    assert_eq!(
        v(call),
        json!({"kind": "call", "name": "c", "cases": [{"input": {}, "expected": 1}], "publicCount": 1, "file": "a.py", "function": "f"})
    );
    let build = ProjectCheck::Build(BuildCheck {
        base: base("b"),
        argv: vec!["make".into()],
        dir: Some("s".into()),
    });
    assert_eq!(
        v(build),
        json!({"kind": "build", "name": "b", "cases": [], "publicCount": 1, "argv": ["make"], "dir": "s"})
    );
    let dom = ProjectCheck::DomAssert(DomAssertCheck {
        base: base("d"),
        file: "a.jsx".into(),
        package: None,
    });
    assert_eq!(
        v(dom),
        json!({"kind": "dom-assert", "name": "d", "cases": [], "publicCount": 1, "file": "a.jsx"})
    );
    let css = ProjectCheck::CssAssert(CssAssertCheck {
        base: base("s"),
        file: "a.jsx".into(),
        package: Some("api".into()),
    });
    assert_eq!(
        v(css),
        json!({"kind": "css-assert", "name": "s", "cases": [], "publicCount": 1, "file": "a.jsx", "package": "api"})
    );
    let http = ProjectCheck::Http(HttpCheck {
        base: base("h"),
        package: "api".into(),
    });
    assert_eq!(
        v(http),
        json!({"kind": "http", "name": "h", "cases": [], "publicCount": 1, "package": "api"})
    );
}

#[test]
fn files_packages_and_steps() {
    let f = FileSpec {
        path: "a/b.py".into(),
        language: "python".into(),
        role: FileRole::Hidden,
        content: "x".into(),
    };
    assert_eq!(
        v(f),
        json!({"path": "a/b.py", "language": "python", "role": "hidden", "content": "x"})
    );
    let p = PackageSpec {
        name: "api".into(),
        dir: "s".into(),
        install: vec!["npm".into(), "i".into()],
        start: vec!["node".into(), "${PORT}".into()],
        ready: Some("up".into()),
        expose_as: Some(OrderedMap(vec![(
            "API_URL".to_owned(),
            "http://127.0.0.1:${PORT}".to_owned(),
        )])),
        depends_on: Some(vec!["db".into()]),
    };
    assert_eq!(
        v(p),
        json!({"name": "api", "dir": "s", "install": ["npm", "i"], "start": ["node", "${PORT}"], "ready": "up",
        "exposeAs": {"API_URL": "http://127.0.0.1:${PORT}"}, "dependsOn": ["db"]})
    );
    let steps = [
        (
            RenderStep::Click {
                selector: "s".into(),
            },
            json!({"op": "click", "selector": "s"}),
        ),
        (
            RenderStep::Change {
                selector: "s".into(),
                value: "x".into(),
            },
            json!({"op": "change", "selector": "s", "value": "x"}),
        ),
        (
            RenderStep::Text {
                selector: "s".into(),
            },
            json!({"op": "text", "selector": "s"}),
        ),
        (
            RenderStep::Count {
                selector: "s".into(),
            },
            json!({"op": "count", "selector": "s"}),
        ),
        (
            RenderStep::Attr {
                selector: "s".into(),
                name: "n".into(),
            },
            json!({"op": "attr", "selector": "s", "name": "n"}),
        ),
        (
            RenderStep::Style {
                selector: "s".into(),
                property: "p".into(),
            },
            json!({"op": "style", "selector": "s", "property": "p"}),
        ),
    ];
    for (step, j) in steps {
        assert_eq!(v(step), j);
    }
}

#[test]
fn program_config_minimal() {
    assert_eq!(
        v(ProgramConfig {
            channel: ProgramChannel::Stdin,
            entry: None,
            flags: None
        }),
        json!({"channel": "stdin"})
    );
}

#[test]
fn maps_keep_declared_order() {
    // Non-alphabetical on purpose: clients print these as written (`{target, nums}`, not sorted).
    let c = TestCase {
        input: OrderedMap(vec![
            ("target".to_owned(), CaseValue::Int(9)),
            ("nums".to_owned(), CaseValue::Null),
        ]),
        expected: CaseValue::Object(OrderedMap(vec![
            ("z".to_owned(), CaseValue::Int(1)),
            ("a".to_owned(), CaseValue::Int(2)),
        ])),
    };
    assert_eq!(
        serde_json::to_string(&c).unwrap(),
        r#"{"input":{"target":9,"nums":null},"expected":{"z":1,"a":2}}"#
    );
    let libs: LibSpec = OrderedMap(vec![
        ("python".to_owned(), vec![]),
        ("java".to_owned(), vec![]),
    ]);
    assert_eq!(
        serde_json::to_string(&libs).unwrap(),
        r#"{"python":[],"java":[]}"#
    );
}
