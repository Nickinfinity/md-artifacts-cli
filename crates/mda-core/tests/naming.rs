//! Output file naming (T3.2): precedence per type, injection, final validation.
#![allow(clippy::unwrap_used)] // reason: test file

mod common;
use mda_core::naming::{
    NameError as E, NameField as F, check_injection, output_name, validate_name,
};
use mda_core::parse::parse_from_content;

/// A synthesized Template at `Templates/<stem>.md`; `fm` = extra frontmatter lines.
fn tpl(stem: &str, fm: &str) -> mda_core::model::ParsedArtifact {
    parse_from_content(
        &format!("---\nartifactType: Template\n{fm}\n---\n```code\nx\n```\n"),
        &format!("Templates/{stem}.md"),
    )
}

fn agent(stem: &str, fm: &str) -> mda_core::model::ParsedArtifact {
    parse_from_content(
        &format!("---\nartifactType: AIAgentsConfig\n{fm}\n---\n%%oa:start%%\nx\n%%oa:end%%\n"),
        &format!("AIAgentsConf/{stem}.md"),
    )
}

fn name(p: &mda_core::model::ParsedArtifact, typed: Option<&str>) -> Result<String, E> {
    output_name(p, typed)
}

#[test]
fn template_basics() {
    let p = tpl("f", "title: Button\nextension: tsx");
    assert_eq!(name(&p, None).unwrap(), "Button.tsx");
    assert_eq!(name(&p, Some("Card.css")).unwrap(), "Card.css");
    assert_eq!(name(&p, Some("Card")).unwrap(), "Card.tsx");
    assert_eq!(
        name(&tpl("f", "title: B\nextension: .mjs"), None).unwrap(),
        "B.mjs"
    );
    assert_eq!(
        name(&tpl("f", "title: B\nextension: mjs"), None).unwrap(),
        "B.mjs"
    );
    assert_eq!(
        name(&tpl("f", "title: script\nlanguage: python"), None).unwrap(),
        "script.py"
    );
    assert_eq!(
        name(&tpl("f", "title: component.test\nextension: tsx"), None).unwrap(),
        "component.test.tsx"
    );
    assert_eq!(
        name(&tpl("f", "title: my.module\nextension: mjs"), None).unwrap(),
        "my.module.mjs"
    );
    let p = tpl("f", "title: B\nextension: tsx");
    assert_eq!(name(&p, Some("component.test")).unwrap(), "component.test");
    let mut bare = tpl("f", "title: B");
    bare.frontmatter.language = None; // the fence info-string would otherwise supply one
    assert_eq!(name(&bare, None).unwrap(), "B");
    assert_eq!(
        name(&tpl("f", "title: B\nextension: ."), None).unwrap(),
        "B"
    );
}

#[test]
fn template_fallbacks() {
    assert_eq!(
        name(&tpl("button", "extension: tsx"), None).unwrap(),
        "button.tsx"
    );
    assert_eq!(
        name(&tpl("button", "title: Real\nextension: tsx"), None).unwrap(),
        "Real.tsx"
    );
    let mut p = tpl("button", "extension: tsx");
    p.frontmatter.title = Some("   ".into()); // truthy in JS, trims to "": not the file name
    assert_eq!(name(&p, None).unwrap(), "template.tsx");
    assert_eq!(
        name(&tpl("f", "title: B.\nextension: tsx"), None).unwrap(),
        "B.tsx"
    );
}

#[test]
fn template_injection() {
    for bad in ["../x", "a/b", "a\\b", "a\0b", "a..b"] {
        let p = tpl("f", "title: B\nextension: tsx");
        assert_eq!(
            name(&p, Some(bad)),
            Err(E::PathInjection(F::FileName)),
            "{bad:?}"
        );
    }
    for bad in [
        "../../etc/passwd",
        "..\\..\\win.ini",
        "a/b",
        "a\\b",
        "x\0.js",
        "a..b",
    ] {
        let mut p = tpl("f", "title: B");
        p.frontmatter.extension = Some(bad.into());
        assert_eq!(
            name(&p, None),
            Err(E::PathInjection(F::Extension)),
            "{bad:?}"
        );
    }
    for bad in ["a\\b", "a..b"] {
        let mut p = agent("r", "title: x");
        p.frontmatter.target = Some(bad.into());
        assert_eq!(name(&p, None), Err(E::PathInjection(F::Target)), "{bad:?}");
    }
    assert_eq!(
        name(&tpl("f", "title: ../evil\nlanguage: txt"), None),
        Err(E::PathInjection(F::Title))
    );
}

#[test]
fn agent_chain() {
    assert_eq!(
        name(&agent("r", "title: Reviewer\ntarget: CLAUDE.md"), None).unwrap(),
        "CLAUDE.md"
    );
    let c = name(&agent("r", "title: R\ntarget: .cursorrules"), None).unwrap();
    assert_eq!(c, ".cursorrules");
    assert_eq!(validate_name(&c), Ok(()));
    assert_eq!(
        name(&agent("r", "title: Claude reviewer"), None).unwrap(),
        "Claude reviewer.md"
    );
    assert_eq!(
        name(&agent("r", "title: notes.txt"), None).unwrap(),
        "notes.txt"
    );
    assert_eq!(name(&agent("stem", ""), None).unwrap(), "stem.md");
    let mut a = agent("r", "");
    a.frontmatter.title = Some("  ".into());
    assert_eq!(name(&a, None).unwrap(), "agent.md");
    assert_eq!(
        name(&agent("r", "target: X.md"), Some("Mine")).unwrap(),
        "Mine"
    );
    assert_eq!(
        name(&agent("r", "target: ../../etc/passwd"), None),
        Err(E::PathInjection(F::Target))
    );
    assert_eq!(
        name(&agent("r", "target: a/b.md"), None),
        Err(E::PathInjection(F::Target))
    );
    assert_eq!(
        name(&agent("r", "title: x"), Some("a/b")),
        Err(E::PathInjection(F::FileName))
    );
}

#[test]
fn validate_cases() {
    for ok in [
        "Button.tsx",
        "README.md",
        "café.ts",
        "a.b.c",
        "CONsole",
        ".cursorrules",
    ] {
        assert_eq!(validate_name(ok), Ok(()), "{ok}");
    }
    assert_eq!(validate_name(""), Err(E::Empty));
    assert_eq!(validate_name("   "), Err(E::Empty));
    assert_eq!(validate_name(" a"), Err(E::EdgeSpace));
    assert_eq!(validate_name("a "), Err(E::EdgeSpace));
    assert_eq!(validate_name("a."), Err(E::EdgeDot));
    assert_eq!(validate_name(".."), Err(E::EdgeDot));
    for c in [
        "a:b", "a*b", "a\"b", "a|b", "a?b", "a<b", "a>b", "a/b", "a\\b",
    ] {
        assert_eq!(validate_name(c), Err(E::IllegalChar), "{c}");
    }
    assert_eq!(validate_name("a\x01b"), Err(E::ControlChar));
    assert_eq!(validate_name("a\x7fb"), Err(E::ControlChar));
    for r in ["CON", "con", "Com1", "LPT9", "CON.txt", "nul.tar.gz"] {
        assert_eq!(validate_name(r), Err(E::Reserved), "{r}");
    }
    assert_eq!(validate_name(&"a".repeat(256)), Err(E::TooLong));
    assert_eq!(validate_name(&"a".repeat(255)), Ok(()));
}

#[test]
fn code_strings_pinned() {
    assert_eq!(E::PathInjection(F::Title).code(), "naming.path_injection");
    assert_eq!(E::TooLong.code(), "naming.too_long");
    assert_eq!(F::FileName.as_str(), "fileName");
    assert_eq!(F::Extension.as_str(), "extension");
    assert_eq!(F::Target.as_str(), "target");
    assert_eq!(F::Title.as_str(), "title");
}

#[test]
fn check_injection_substring_rule() {
    assert_eq!(
        check_injection("a..b", F::Title),
        Err(E::PathInjection(F::Title))
    );
    assert_eq!(check_injection("a.b", F::Title), Ok(()));
}

#[test]
fn fuzz_ok_names_are_safe() {
    for s in common::fuzz_strings(0x7A31, 5_000, 30) {
        let q = |v: &str| v.replace(['\n', '\r'], " ").replace('\'', "''");
        let q = q(&s);
        let arts = [
            tpl("f", &format!("title: '{q}'")),
            tpl("f", &format!("title: B\nextension: '{q}'")),
            agent("f", &format!("target: '{q}'")),
            agent("f", &format!("title: '{q}'")),
        ];
        for p in &arts {
            for typed in [None, Some(s.as_str())] {
                if let Ok(n) = output_name(p, typed) {
                    assert_eq!(validate_name(&n), Ok(()), "{n:?}");
                    assert!(!n.contains(['/', '\\', '\0']) && !n.contains(".."), "{n:?}");
                }
            }
        }
    }
}
