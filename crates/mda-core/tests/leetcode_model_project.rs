#![allow(clippy::unwrap_used)] // helper `v` is not a #[test] fn, so clippy.toml's test exemption misses it

//! Wire shape of the LeetCode model (project half): `to_value` against hand-written, TS-shaped JSON (D-41).
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
