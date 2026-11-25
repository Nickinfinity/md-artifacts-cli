//! Round-trip tests for the canonical items: each emit parses back equal on its field.

use super::*;
use crate::leetcode::emit::testkit::field;
use crate::leetcode::{LeetCodeDifficulty, LeetCodeStatus, LeetcodeType, parse};
use serde_json::{Value, json, to_value};

const FM: &str = "---\nartifactType: LeetCode\n---\n\n";

fn parsed(text: &str) -> Value {
    to_value(parse(text)).unwrap_or(Value::Null)
}
fn sol(lang: &str, label: Option<&str>, code: &str) -> LeetCodeSolution {
    LeetCodeSolution {
        language: lang.into(),
        label: label.map(Into::into),
        code: code.into(),
        solved_at: None,
        duration: None,
    }
}

#[test]
fn frontmatter_parses_back_every_field() {
    for (st, ty) in [
        (LeetCodeStatus::Solved, LeetcodeType::Package),
        (LeetCodeStatus::Attempted, LeetcodeType::Stack),
        (LeetCodeStatus::Unsolved, LeetcodeType::Function),
    ] {
        let x = LeetCodeExercise {
            title: "Two Sum: #1 \"x\"".into(),
            leetcode_type: ty,
            difficulty: LeetCodeDifficulty::Hard,
            status: st,
            algorithm: Some("hash-map".into()),
            tags: vec!["a".into(), "b c".into()],
            ..LeetCodeExercise::default()
        };
        let fm = frontmatter(&x).unwrap();
        assert!(
            fm.starts_with("---\nartifactType: LeetCode\nleetcodeType: ") && fm.ends_with("---\n")
        );
        let (got, want) = (parsed(&fm), to_value(&x).unwrap());
        for k in [
            "title",
            "leetcodeType",
            "difficulty",
            "status",
            "algorithm",
            "tags",
        ] {
            assert_eq!(field(&got, k), field(&want, k), "{k}");
        }
    }
    let bare = frontmatter(&LeetCodeExercise {
        title: "T".into(),
        ..Default::default()
    })
    .unwrap();
    assert!(!bare.contains("algorithm") && !bare.contains("tags"));
}

#[test]
fn frontmatter_refuses_what_it_cannot_carry() {
    let t = |title: &str| LeetCodeExercise {
        title: title.into(),
        ..Default::default()
    };
    for bad in [
        t(""),
        t("a\nb"),
        t("a  b"),
        LeetCodeExercise {
            tags: vec!["a,b".into()],
            ..t("T")
        },
        LeetCodeExercise {
            algorithm: Some(String::new()),
            ..t("T")
        },
    ] {
        assert!(frontmatter(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn description_parses_back_or_is_refused() {
    let fm = frontmatter(&LeetCodeExercise {
        title: "T".into(),
        ..Default::default()
    })
    .unwrap();
    let wrap = |d: &str| format!("{fm}\n{d}\n\n## Examples\n");
    for d in [
        "Given nums.\n\nReturn *indices*.",
        "",
        "x\n```text\n# not a heading\n```\ny",
        "#hashtag ok",
    ] {
        let x = LeetCodeExercise {
            description: d.into(),
            ..Default::default()
        };
        let out = description(&x).unwrap();
        assert_eq!(field(&parsed(&wrap(&out)), "description"), d, "{d:?}");
    }
    for bad in [
        "a\n# H\nb",
        "a\n## Examples\nb",
        "open\n```text\nx",
        " lead",
        "trail\n",
        "a\n```yaml leetcode\nreturns: int\n```\nb",
    ] {
        let x = LeetCodeExercise {
            description: bad.into(),
            ..Default::default()
        };
        assert!(description(&x).is_err(), "{bad:?}");
    }
}

#[test]
fn example_parses_back_or_is_refused() {
    let e = Example {
        input: "nums = [2,7], target = 9".into(),
        output: "[0,1]".into(),
    };
    let text = example(&e).unwrap();
    let v = parsed(&format!("{FM}## Examples\n\n{text}"));
    assert_eq!(
        field(&v, "examples"),
        json!([{"input": e.input, "output": e.output}])
    );
    for (i, o) in [
        ("a\nb", "x"),
        (" a", "b"),
        ("a```", "b"),
        ("a", "b\u{2028}c"),
        ("", "b"),
    ] {
        let bad = Example {
            input: i.into(),
            output: o.into(),
        };
        assert!(example(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn setup_parses_back_or_is_refused() {
    let s = ExerciseSetup {
        language: "python".into(),
        code: "def f():\n    # c\n    pass".into(),
    };
    let v = parsed(&format!("{FM}# Setup\n\n{}", setup(&s).unwrap()));
    assert_eq!(
        field(&v, "setups"),
        json!([{"language": "python", "code": s.code}])
    );
    for (l, c) in [
        ("python", "a```b"),
        ("python", "x\n"),
        ("Python", "x"),
        ("c sharp", "x"),
        ("", "x"),
    ] {
        let bad = ExerciseSetup {
            language: l.into(),
            code: c.into(),
        };
        assert!(setup(&bad).is_err(), "{bad:?}");
    }
    assert_eq!(
        setup(&ExerciseSetup {
            language: "py".into(),
            code: "a```".into()
        }),
        Err(crate::error::NotSerializableReason::Fence)
    );
}

#[test]
fn solution_parses_back_or_is_refused() {
    let mut meta = sol("python", Some("Hash map"), "pass");
    meta.solved_at = Some("2026-11-05T10:00:00Z".into());
    meta.duration = Some("5m".into());
    for s in [
        sol("python", None, "x = 1"),
        sol("java", Some("Two pointers"), "int a;\n// c"),
        meta,
    ] {
        let v = parsed(&format!("{FM}# Solutions\n\n{}", solution(&s).unwrap()));
        assert_eq!(field(&v, "solutions"), json!([to_value(&s).unwrap()]));
    }
    let half = LeetCodeSolution {
        solved_at: Some("t".into()),
        ..sol("python", None, "x")
    };
    let arrow = LeetCodeSolution {
        duration: Some("5m --> x".into()),
        solved_at: Some("t".into()),
        ..sol("python", None, "x")
    };
    for bad in [
        sol("python", None, "a```"),
        sol("python", Some("a\nb"), "x"),
        sol("Python", None, "x"),
        half,
        arrow,
    ] {
        assert!(solution(&bad).is_err(), "{bad:?}");
    }
}
