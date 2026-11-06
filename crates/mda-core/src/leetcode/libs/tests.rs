//! T4.6 unit tests: ports of `lib-spec.test.ts` and `lib-spec-grammars.test.ts` (Q-33).

use super::*;
use LibEcosystem::*;

const ALL: [LibEcosystem; 4] = [Pnpm, Pip, Cargo, Maven];

fn ok(e: LibEcosystem, raw: &str) -> ParsedLibSpec {
    parse_spec(e, raw).unwrap_or_else(|r| panic!("expected '{raw}' to parse: {r:?}"))
}
fn bad(e: LibEcosystem, raw: &str) -> SpecRefusal {
    match parse_spec(e, raw) {
        Err(r) => r,
        Ok(s) => panic!("expected '{raw}' refused, got {s:?}"),
    }
}
fn s(x: &str) -> String {
    x.to_owned()
}
fn v(x: &[&str]) -> Vec<String> {
    x.iter().map(|y| (*y).to_owned()).collect()
}

#[test]
fn seam_vars_pinned() {
    assert_eq!(
        LIB_SEAM_VARS,
        ["NODE_PATH", "VIRTUAL_ENV", "CLASSPATH", "CARGO_TARGET_DIR"]
    );
    assert_eq!(LIB_ECOSYSTEMS, ["pnpm", "pip", "cargo", "maven"]);
}

#[test]
fn ecosystem_lookup() {
    assert_eq!(ecosystem_for("python"), Some(Pip));
    assert_eq!(ecosystem_for("py"), Some(Pip));
    assert_eq!(ecosystem_for("typescriptreact"), Some(Pnpm));
    assert_eq!(ecosystem_for("TS"), Some(Pnpm));
    assert_eq!(ecosystem_for("rs"), Some(Cargo));
    assert_eq!(ecosystem_for("java"), Some(Maven));
    assert_eq!(ecosystem_for("cobol"), None);
    assert_eq!(ecosystem_for("__proto__"), None);
}

#[test]
fn validate_names() {
    assert!(validate_lib_names(&["lodash", "lodash@4", "@types/node@^20.1.0"]).is_empty());
    assert_eq!(
        validate_lib_names(&["lodash@4", "--target=/etc"]),
        ["--target=/etc"]
    );
    for h in [
        "-rf",
        ";rm -rf /",
        "../../etc/passwd",
        "/etc/passwd",
        "a/../../etc",
        "a/..",
        "lodash\n--evil",
        "`whoami`",
        "",
    ] {
        assert_eq!(validate_lib_names(&[h]), [h], "{h:?}");
    }
    assert_eq!(
        validate_lib_names(&["lodash", "-rf", "../evil", "ok-pkg"]),
        ["-rf", "../evil"]
    );
    assert!(validate_lib_names(&[]).is_empty());
}

#[test]
fn package_names() {
    for (a, b) in [
        ("lodash", "lodash"),
        ("react@^19.0.0", "react"),
        ("left-pad@1.0.0", "left-pad"),
        ("@types/node", "@types/node"),
        ("@types/node@^20.0.0", "@types/node"),
        ("@scope/pkg", "@scope/pkg"),
    ] {
        assert_eq!(package_name_of(a), b);
    }
}

#[test]
fn pnpm() {
    assert_eq!(
        ok(Pnpm, "react"),
        ParsedLibSpec::Pnpm {
            name: s("react"),
            range: None
        }
    );
    assert_eq!(
        ok(Pnpm, "react@^19.0.0"),
        ParsedLibSpec::Pnpm {
            name: s("react"),
            range: Some(s("^19.0.0"))
        }
    );
    assert_eq!(
        ok(Pnpm, "@types/node@^20"),
        ParsedLibSpec::Pnpm {
            name: s("@types/node"),
            range: Some(s("^20"))
        }
    );
    assert_eq!(
        ok(Pnpm, "@types/node"),
        ParsedLibSpec::Pnpm {
            name: s("@types/node"),
            range: None
        }
    );
    for h in [
        "file:../../etc",
        "link:/",
        "git+ssh://x/y",
        "workspace:*",
        "https://evil.example/x.tgz",
    ] {
        bad(Pnpm, h);
    }
}

#[test]
fn pip() {
    assert_eq!(
        ok(Pip, "requests[socks]==2.32.3"),
        ParsedLibSpec::Pip {
            name: s("requests"),
            extras: v(&["socks"]),
            predicates: v(&["==2.32.3"])
        }
    );
    assert_eq!(
        ok(Pip, "numpy>=2,<3"),
        ParsedLibSpec::Pip {
            name: s("numpy"),
            extras: vec![],
            predicates: v(&[">=2", "<3"])
        }
    );
    assert_eq!(
        ok(Pip, "numpy"),
        ParsedLibSpec::Pip {
            name: s("numpy"),
            extras: vec![],
            predicates: vec![]
        }
    );
    for h in [
        "requests; python_version < \"3\"",
        "numpy @ git+https://evil.example/x.git",
        "git+https://evil.example/x.git",
        "-r requirements.txt",
        "--index-url=https://evil.example",
        "./local",
        "/etc/passwd",
        "a/../../etc",
    ] {
        bad(Pip, h);
    }
}

#[test]
fn cargo() {
    assert_eq!(
        ok(Cargo, "serde_json@1.0"),
        ParsedLibSpec::Cargo {
            name: s("serde_json"),
            req: Some(s("1.0")),
            features: vec![]
        }
    );
    assert_eq!(
        ok(Cargo, "serde@^1+derive+std"),
        ParsedLibSpec::Cargo {
            name: s("serde"),
            req: Some(s("^1")),
            features: v(&["derive", "std"])
        }
    );
    assert_eq!(
        ok(Cargo, "rand"),
        ParsedLibSpec::Cargo {
            name: s("rand"),
            req: None,
            features: vec![]
        }
    );
    for h in [
        "serde = { path = \"/\" }",
        "serde = { git = \"https://evil.example\" }",
        "serde = { registry = \"evil\" }",
        "serde\n[build-dependencies]\nx = \"1\"",
        "serde@1\"",
        "serde@1+default-features=false",
    ] {
        bad(Cargo, h);
    }
}

#[test]
fn maven() {
    assert_eq!(
        ok(Maven, "com.google.guava:guava:33.3.1-jre"),
        ParsedLibSpec::Maven {
            group_id: s("com.google.guava"),
            artifact_id: s("guava"),
            version: s("33.3.1-jre"),
            packaging: None,
            classifier: None,
        }
    );
    assert_eq!(
        ok(Maven, "org.x:y:1.0:jar:sources"),
        ParsedLibSpec::Maven {
            group_id: s("org.x"),
            artifact_id: s("y"),
            version: s("1.0"),
            packaging: Some(s("jar")),
            classifier: Some(s("sources")),
        }
    );
    for h in [
        "guava",
        "com.google:guava",
        "a:b:c:d:e:f",
        "com.x:y:LATEST",
        "com.x:y:RELEASE",
        "com.x:y:</version><plugin>",
        "com.x:y:1.0\"",
    ] {
        bad(Maven, h);
    }
}

#[test]
fn shared_refusals() {
    for e in ALL {
        for h in [
            "../../etc/passwd",
            "a/../../etc",
            "a:b:..",
            "--target=/etc",
            "-rf",
            "requests; rm -rf /",
            "`whoami`",
            "lodash\n--evil",
            "a b",
            "",
            "   ",
            "pkg\u{0}",
            "pkg%00",
        ] {
            bad(e, h);
        }
    }
}

#[test]
fn long_specs_return() {
    let long = "a".repeat(10_000);
    for e in ALL {
        let _ = parse_spec(e, &long);
        let _ = parse_spec(e, &format!("{long}@{long}"));
        let _ = parse_spec(e, &format!("{long}!"));
    }
}

#[test]
fn refusal_variants() {
    use SpecRefusal as R;
    assert_eq!(bad(Pip, "-r requirements.txt"), R::Name);
    assert_eq!(bad(Maven, "guava"), R::Coordinate);
    assert_eq!(bad(Maven, "a:b:c:d:e:f"), R::Coordinate);
    assert_eq!(bad(Maven, "a:b:<"), R::Segment);
    assert_eq!(bad(Maven, "com.x:y:LATEST"), R::Floating);
    assert_eq!(bad(Pnpm, "a/../b"), R::Traversal);
    assert_eq!(bad(Pnpm, "react@"), R::Range);
    assert_eq!(bad(Pip, "a[b"), R::UnterminatedExtras);
    assert_eq!(bad(Pip, "a[-x]"), R::Extra);
    assert_eq!(bad(Pip, "a;b"), R::Name);
    assert_eq!(bad(Pip, "a>=x"), R::Predicate);
    assert_eq!(bad(Cargo, "a@x"), R::Req);
    assert_eq!(bad(Cargo, "a@1+X"), R::Feature);
}

#[test]
fn length_cap() {
    let ok = "a".repeat(MAX_SPEC_CHARS);
    assert!(parse_spec(Pnpm, &ok).is_ok());
    let long = "a".repeat(MAX_SPEC_CHARS + 1);
    for e in ALL {
        assert_eq!(bad(e, &long), SpecRefusal::TooLong);
    }
    // chars, not bytes
    assert!(
        parse_spec(Pnpm, &"é".repeat(MAX_SPEC_CHARS)).is_err_and(|r| r != SpecRefusal::TooLong)
    );
    assert_eq!(validate_lib_names(&[long.as_str()]), [long.as_str()]);
}
