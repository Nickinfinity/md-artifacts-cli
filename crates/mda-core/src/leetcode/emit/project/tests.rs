//! `parse(emit(x)) == x` for every project group, against the engine's own parser (the inverse oracle).

use serde_json::{Value, json};

use super::super::testkit::{field, parsed_with};
use super::*;
use crate::leetcode::{
    BuildCheck, CheckBase, CssAssertCheck, DomAssertCheck, FileRole, FunctionCheck, HttpCheck,
    LeetcodeType, OrderedMap, PackageSpec, ProgramChannel, ProgramConfig, ProjectCheck, TestConfig,
    TestTypeId,
};

fn fence(groups: &[Option<String>]) -> String {
    let body: String = groups.iter().flatten().cloned().collect();
    format!("\n```yaml leetcode\n{body}```\n")
}

fn base(name: &str) -> CheckBase {
    CheckBase {
        name: name.into(),
        cases: vec![],
        public_count: 0,
    }
}

fn pkg(name: &str) -> PackageSpec {
    PackageSpec {
        name: name.into(),
        dir: "server".into(),
        install: vec!["pip".into(), "install".into()],
        start: vec!["run".into(), "${PORT}".into()],
        ready: None,
        expose_as: None,
        depends_on: None,
    }
}

fn s(v: &str) -> String {
    v.to_owned()
}

#[test]
fn test_scalars_round_trip() {
    let x = LeetCodeExercise {
        test: TestConfig {
            r#type: TestTypeId::Program,
            timeout_ms: 2000,
        },
        ..Default::default()
    };
    let g = test_group(&x).unwrap();
    assert!(
        g.as_deref().is_some_and(|t| t.starts_with("test:\n")),
        "{g:?}"
    );
    let v = parsed_with(LeetcodeType::Package, &fence(&[g]));
    assert_eq!(
        field(&v, "test"),
        json!({"type": "program", "timeoutMs": 2000})
    );
    assert_eq!(test_group(&LeetCodeExercise::default()).unwrap(), None);
}

fn all_checks() -> Vec<ProjectCheck> {
    vec![
        ProjectCheck::Call(FunctionCheck {
            base: base("catalogue filter"),
            file: "src/lib/c.ts".into(),
            function: "filterInStock".into(),
        }),
        ProjectCheck::Build(BuildCheck {
            base: base("builds"),
            argv: vec!["npx".into(), "tsc".into(), "--noEmit".into()],
            dir: Some("client".into()),
        }),
        ProjectCheck::Http(HttpCheck {
            base: base("api"),
            package: "api".into(),
        }),
        ProjectCheck::DomAssert(DomAssertCheck {
            base: base("page"),
            file: "client/App.jsx".into(),
            package: Some("api".into()),
        }),
        ProjectCheck::CssAssert(CssAssertCheck {
            base: base(" padded: name "),
            file: "a.css".into(),
            package: None,
        }),
    ]
}

#[test]
fn checks_round_trip_with_scalars() {
    let x = LeetCodeExercise {
        test: TestConfig {
            r#type: TestTypeId::Call,
            timeout_ms: 10_000,
        },
        checks: Some(all_checks()),
        ..Default::default()
    };
    let g = test_group(&x).unwrap();
    let v = parsed_with(LeetcodeType::Package, &fence(&[g]));
    assert_eq!(
        field(&v, "checks"),
        serde_json::to_value(all_checks()).unwrap()
    );
    assert_eq!(field(&v, "test")["timeoutMs"], 10_000);
}

#[test]
fn libs_round_trip_block_form_keeps_commas() {
    let libs = OrderedMap(vec![
        (
            "python".to_owned(),
            vec![s("numpy>=2,<3"), s("fastapi>=0.115,<0.116")],
        ),
        ("typescript".to_owned(), vec![s("react@^19.0.0")]),
    ]);
    let x = LeetCodeExercise {
        libs: Some(libs.clone()),
        ..Default::default()
    };
    let g = libs_group(&x).unwrap();
    let v = parsed_with(LeetcodeType::Package, &fence(&[g]));
    assert_eq!(field(&v, "libs"), serde_json::to_value(libs).unwrap());
    assert_eq!(libs_group(&LeetCodeExercise::default()).unwrap(), None);
}

#[test]
fn libs_the_parser_would_drop_are_refused() {
    for lang in ["__proto__", "klingon", "py:thon", "python\nx"] {
        let x = LeetCodeExercise {
            libs: Some(OrderedMap(vec![(lang.to_owned(), vec![s("numpy")])])),
            ..Default::default()
        };
        assert!(libs_group(&x).is_err(), "{lang:?}");
    }
    let bad_spec = LeetCodeExercise {
        libs: Some(OrderedMap(vec![(s("python"), vec![s("numpy; rm -rf /")])])),
        ..Default::default()
    };
    assert!(libs_group(&bad_spec).is_err());
}

#[test]
fn packages_round_trip_every_field() {
    let mut a = pkg("api");
    a.ready = Some("Uvicorn running".into());
    a.expose_as = Some(OrderedMap(vec![(
        s("VITE_API_URL"),
        s("http://127.0.0.1:${PORT}/x"),
    )]));
    let mut b = pkg("web");
    b.depends_on = Some(vec![s("api")]);
    let x = LeetCodeExercise {
        packages: Some(vec![a, b]),
        ..Default::default()
    };
    let g = packages_group(&x).unwrap();
    let v = parsed_with(LeetcodeType::Stack, &fence(&[g]));
    assert_eq!(
        field(&v, "packages"),
        serde_json::to_value(&x.packages).unwrap()
    );
    assert_eq!(packages_group(&LeetCodeExercise::default()).unwrap(), None);
}

#[test]
fn packages_the_parser_would_drop_are_refused() {
    let with = |f: &dyn Fn(&mut PackageSpec)| {
        let mut p = pkg("api");
        f(&mut p);
        packages_group(&LeetCodeExercise {
            packages: Some(vec![p]),
            ..Default::default()
        })
    };
    assert!(with(&|p| p.dir = s("../x")).is_err());
    assert!(with(&|p| p.start = vec![s("${HOME}")]).is_err());
    assert!(with(&|p| p.install = vec![]).is_err());
    assert!(with(&|p| p.name = String::new()).is_err());
    for bad in ["__proto__", "PATH", "lower", "DYLD_X", "A\nB: c"] {
        let r = with(&|p| {
            p.expose_as = Some(OrderedMap(vec![(s(bad), s("http://127.0.0.1:${PORT}"))]))
        });
        assert!(r.is_err(), "{bad}");
    }
    assert!(with(&|p| p.expose_as = Some(OrderedMap(vec![(s("OK"), s("http://evil"))]))).is_err());
}

#[test]
fn program_round_trips() {
    let p = ProgramConfig {
        channel: ProgramChannel::Flags,
        entry: Some("Main.java".into()),
        flags: Some(vec![s("--nums"), s("--target")]),
    };
    let x = LeetCodeExercise {
        program: Some(p),
        ..Default::default()
    };
    let g = program_group(&x).unwrap();
    let v = parsed_with(LeetcodeType::Package, &fence(&[g]));
    assert_eq!(
        field(&v, "program"),
        serde_json::to_value(&x.program).unwrap()
    );
    assert_eq!(program_group(&LeetCodeExercise::default()).unwrap(), None);
    let bare = LeetCodeExercise {
        program: Some(ProgramConfig {
            channel: ProgramChannel::Argv,
            entry: None,
            flags: None,
        }),
        ..Default::default()
    };
    let v = parsed_with(
        LeetcodeType::Package,
        &fence(&[program_group(&bare).unwrap()]),
    );
    assert_eq!(field(&v, "program"), json!({"channel": "argv"}));
}

#[test]
fn program_unreadable_values_are_refused() {
    for (entry, flags) in [
        (Some("../x"), None),
        (Some(" x"), None),
        (Some("a\nb"), None),
        (None, Some(vec!["a,b"])),
        (None, Some(vec!["\"q\""])),
        (None, Some(vec![""])),
    ] {
        let x = LeetCodeExercise {
            program: Some(ProgramConfig {
                channel: ProgramChannel::Flags,
                entry: entry.map(s),
                flags: flags.clone().map(|f| f.into_iter().map(s).collect()),
            }),
            ..Default::default()
        };
        assert!(program_group(&x).is_err(), "{entry:?} {flags:?}");
    }
}

fn file(path: &str, role: FileRole, content: &str) -> FileSpec {
    FileSpec {
        path: path.into(),
        language: "python".into(),
        role,
        content: content.into(),
    }
}

#[test]
fn file_fences_round_trip() {
    let fs = vec![
        file(
            "main.py",
            FileRole::Editable,
            "x = 1\n# not a heading? it is\n",
        ),
        file("a b/c.py", FileRole::Readonly, ""),
        file("h.py", FileRole::Hidden, "y\r\nz\n"),
    ];
    let body: String = fs.iter().map(|f| file_fence(f).unwrap() + "\n").collect();
    let v = parsed_with(LeetcodeType::Package, &format!("\n## Files\n\n{body}"));
    assert_eq!(field(&v, "files"), serde_json::to_value(&fs).unwrap());
}

#[test]
fn file_fence_refuses_what_cannot_read_back() {
    use crate::error::NotSerializableReason as Why;
    let f = |p: &str, c: &str| file_fence(&file(p, FileRole::Editable, c));
    assert_eq!(f("a.py", "```\nboom\n"), Err(Why::Fence));
    assert_eq!(f("a.py", "``````"), Err(Why::Fence));
    assert!(f("a.py", "no newline").is_err());
    assert!(f("a b\"c.py", "x\n").is_err());
    assert!(f("a\nb.py", "x\n").is_err());
    assert!(f("", "x\n").is_err());
}

const HOSTILE: &[&str] = &[
    "x", " x", "\"x\"", "'x'", "a\nb", "---", "```", "a\"b", "\u{2028}", "#x", "x # y", "a: b",
    "-", "- x",
];

#[test]
fn hostile_scalars_parse_back_or_err_never_a_fence_run() {
    for h in HOSTILE {
        let mut p = pkg("api");
        p.ready = Some(s(h));
        p.dir = s(h);
        let x = LeetCodeExercise {
            packages: Some(vec![p]),
            checks: Some(vec![ProjectCheck::Call(FunctionCheck {
                base: base(h),
                file: s(h),
                function: s(h),
            })]),
            libs: Some(OrderedMap(vec![(s("python"), vec![s(h)])])),
            ..Default::default()
        };
        for g in [test_group(&x), libs_group(&x), packages_group(&x)] {
            if let Ok(Some(t)) = g {
                assert!(!t.contains("```"), "{h:?}");
            }
        }
        let y = LeetCodeExercise {
            checks: Some(vec![ProjectCheck::Call(FunctionCheck {
                base: base(h),
                file: s(h),
                function: s(h),
            })]),
            ..Default::default()
        };
        if let Ok(g @ Some(_)) = test_group(&y) {
            let v = parsed_with(LeetcodeType::Package, &fence(&[g]));
            assert_eq!(
                field(&v, "checks"),
                serde_json::to_value(&y.checks).unwrap(),
                "{h:?}"
            );
        }
        let f = file_fence(&file(h, FileRole::Editable, h));
        if let Ok(t) = f {
            assert_eq!(t.matches("```").count(), 2, "{h:?}");
        }
    }
}

fn strip_cases(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(a) = v.as_array_mut() {
        for c in a {
            if let Some(o) = c.as_object_mut() {
                o.remove("cases");
                o.remove("publicCount");
            }
        }
    }
    v
}

#[test]
fn every_conformance_project_value_round_trips() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/leetcode");
    let mut n = 0;
    for dir in std::fs::read_dir(&root).unwrap().flatten() {
        for f in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let p = f.path();
            if !p.to_string_lossy().ends_with(".expected.json") {
                continue;
            }
            let want: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let x: LeetCodeExercise = serde_json::from_value(want.clone()).unwrap();
            let groups = [
                test_group(&x),
                libs_group(&x),
                packages_group(&x),
                program_group(&x),
            ]
            .map(|g| g.unwrap_or_else(|e| panic!("{p:?}: {e:?}")));
            let files: String = x
                .files
                .iter()
                .flatten()
                .map(|f| file_fence(f).unwrap_or_else(|e| panic!("{p:?}: {e:?}")) + "\n")
                .collect();
            let extra = format!("{}\n## Files\n\n{files}", fence(&groups));
            let got = parsed_with(x.leetcode_type, &extra);
            let ctx = format!("{p:?}\n{extra}");
            assert_eq!(field(&got, "test"), field(&want, "test"), "{ctx}");
            assert_eq!(
                strip_cases(&field(&got, "checks")),
                strip_cases(&field(&want, "checks")),
                "{ctx}"
            );
            for k in ["libs", "packages", "program", "files"] {
                let (g, w) = (field(&got, k), field(&want, k));
                assert!(
                    g == w || (g == json!({}) || g == json!([])) && w.is_null(),
                    "{k} {ctx}"
                );
            }
            n += 1;
        }
    }
    assert!(n >= 79, "{n}");
}
