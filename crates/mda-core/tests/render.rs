//! Render engine, T3.1: plain tokens, paths, Choice, warnings, limits, block selection.
#![allow(clippy::indexing_slicing, clippy::unwrap_used)] // reason: test assertions index fixed shapes and unwrap known-good renders

mod common;
use mda_core::error::{
    LimitExceeded, RENDER_NOT_SCALAR, RENDER_UNKNOWN_VAR, RenderLimit, VKS_BAD_KEY,
    VKS_CONTROL_CHAR, VKS_LIMIT,
};
use mda_core::model::ParsedVar;
use mda_core::parse::parse_from_content;
use mda_core::render::{
    MAX_OUTPUT_BYTES, RenderError, Values, Warning, check_values, render, render_artifact,
};
use mda_core::vks::{VksList, VksRecord, VksValue};

fn s(x: &str) -> VksValue {
    VksValue::Str(x.into())
}

fn strs(items: &[&str]) -> VksValue {
    VksValue::List(VksList::Strings(
        items.iter().map(|i| (*i).into()).collect(),
    ))
}

fn vals(pairs: &[(&str, VksValue)]) -> Values {
    pairs
        .iter()
        .map(|(k, v)| ((*k).into(), v.clone()))
        .collect()
}

fn var(name: &str, v: VksValue) -> ParsedVar {
    ParsedVar {
        name: name.into(),
        value: v,
    }
}

/// Spec §9.3's `VK-env`, `VK-users`, `VK-db` as declared defaults.
fn sample_vars() -> Vec<ParsedVar> {
    let rec = |f: &[(&str, VksValue)]| {
        VksRecord(f.iter().map(|(k, v)| ((*k).into(), v.clone())).collect())
    };
    vec![
        var("VK-env", strs(&["dev", "prod"])),
        var(
            "VK-users",
            VksValue::List(VksList::Records(vec![
                rec(&[
                    ("id", s("1")),
                    ("name", s("Alice Smith")),
                    ("tags", strs(&["core", "ops"])),
                ]),
                rec(&[
                    ("id", s("2")),
                    ("name", s("Bob Jones")),
                    ("tags", strs(&[])),
                ]),
            ])),
        ),
        var(
            "VK-db",
            VksValue::Record(rec(&[("host", s("localhost")), ("port", s("5432"))])),
        ),
    ]
}

fn out(code: &str, vars: &[ParsedVar], values: &Values) -> String {
    render(code, vars, values).unwrap().output
}

fn warns(code: &str, vars: &[ParsedVar]) -> Vec<Warning> {
    render(code, vars, &Values::new()).unwrap().warnings
}

#[test]
fn plain_token_substitutes() {
    let vars = [ParsedVar::text("VK-a", "x")];
    assert_eq!(out("echo <VK-a>", &vars, &Values::new()), "echo x");
}

#[test]
fn ts_parity_table() {
    // (code, client values, expected); R-12: "" counts as not supplied.
    let v = |p: &[(&str, &str)]| vals(&p.iter().map(|(k, x)| (*k, s(x))).collect::<Vec<_>>());
    let repo = v(&[("VK-repo", "my-app"), ("VK-branch", "main")]);
    let cases: &[(&str, Values, &str)] = &[
        (
            "Review <VK-repo></VK-repo> now.",
            repo.clone(),
            "Review my-app now.",
        ),
        ("Review </VK-repo> now.", repo.clone(), "Review my-app now."),
        ("<VK-repo></VK-branch>", repo.clone(), "my-appmain"),
        ("<VK-repo> on </VK-repo>", repo.clone(), "my-app on my-app"),
        (
            "<VK-nope></VK-nope> </VK-nope>",
            repo.clone(),
            "<VK-nope></VK-nope> </VK-nope>",
        ),
        (
            "You work on <VK-repo></VK-repo>, branch </VK-branch>.",
            repo.clone(),
            "You work on my-app, branch main.",
        ),
        ("a<VK-x></VK-x>b", v(&[("VK-x", "")]), "a<VK-x></VK-x>b"),
        ("<VK-name>", v(&[("VK-name", "Alice")]), "Alice"),
        (
            "Host: <VK-host>, Port: <VK-port>",
            v(&[("VK-host", "localhost"), ("VK-port", "8080")]),
            "Host: localhost, Port: 8080",
        ),
        (
            "<VK-env>/<VK-env>/<VK-env>",
            v(&[("VK-env", "prod")]),
            "prod/prod/prod",
        ),
        ("<VK-unknown>", Values::new(), "<VK-unknown>"),
        (
            "Bearer <VK-token> for <VK-user>",
            v(&[("VK-token", "abc123")]),
            "Bearer abc123 for <VK-user>",
        ),
        (
            "console.log(\"hello world\");",
            v(&[("VK-x", "y")]),
            "console.log(\"hello world\");",
        ),
        (
            "const url = <VK-host>/<VK-path>",
            Values::new(),
            "const url = <VK-host>/<VK-path>",
        ),
    ];
    for (code, values, expected) in cases {
        assert_eq!(&out(code, &[], values), expected, "code: {code}");
    }
    // Defaults: typed wins, empty falls back, none stays literal, absent falls back.
    let host = [ParsedVar::text("VK-host", "localhost")];
    assert_eq!(
        out("<VK-host>", &host, &v(&[("VK-host", "typed")])),
        "typed"
    );
    assert_eq!(out("<VK-host>", &host, &v(&[("VK-host", "")])), "localhost");
    assert_eq!(out("<VK-host>", &host, &Values::new()), "localhost");
    assert_eq!(
        out(
            "<VK-host>",
            &[ParsedVar::text("VK-host", "")],
            &Values::new()
        ),
        "<VK-host>"
    );
    // One unknown_var for three spellings.
    let w = warns("<VK-nope> </VK-nope> <VK-nope></VK-nope>", &[]);
    assert_eq!(
        w,
        vec![Warning::new(RENDER_UNKNOWN_VAR, 1).with("name", "VK-nope")]
    );
    // R-26: `\r` is not a value character (a tab is: `is_control` admits it; plan said tab).
    let e = check_values(&vals(&[("VK-a", s("a\rb"))])).unwrap_err();
    assert_eq!(e.code, VKS_CONTROL_CHAR);
}

#[test]
fn inserted_values_are_not_rescanned() {
    let values = vals(&[("VK-x", s("<VK-y>")), ("VK-y", s("Y"))]);
    assert_eq!(out("<VK-x></VK-x> <VK-x>", &[], &values), "<VK-y> <VK-y>");
}

#[test]
fn code_is_scanned_for_variables_not_only_defaults() {
    let vars = [ParsedVar::text("VK-greeting", "marker")];
    let values = vals(&[("VK-name", s("bob")), ("VK-zzz", s("ignored"))]);
    assert_eq!(
        out("echo <VK-greeting> <VK-name>", &vars, &values),
        "echo marker bob"
    );
}

#[test]
fn choice_and_paths() {
    let vars = sample_vars();
    let none = Values::new();
    assert_eq!(out("<VK-env>", &vars, &none), "dev");
    assert_eq!(
        out("<VK-env>", &vars, &vals(&[("VK-env", s("staging"))])),
        "staging"
    );
    assert_eq!(out("<VK-db.host>", &vars, &none), "localhost");
    let empty = [var("VK-l", strs(&[]))];
    assert_eq!(out("<VK-l>", &empty, &none), "<VK-l>");
    assert_eq!(
        warns("<VK-l>", &empty),
        vec![Warning::new(RENDER_UNKNOWN_VAR, 1).with("name", "VK-l")]
    );
    let ns = |k: &str, v: &str| Warning::new(RENDER_NOT_SCALAR, 1).with(k, v);
    assert_eq!(warns("<VK-db>", &vars), vec![ns("name", "VK-db")]);
    assert_eq!(
        warns("<VK-users.name>", &vars),
        vec![ns("path", "VK-users.name")]
    );
    assert_eq!(warns("<VK-env.x>", &vars), vec![ns("path", "VK-env.x")]);
    assert_eq!(
        warns("<VK-db.nope>", &vars),
        vec![Warning::new(RENDER_UNKNOWN_VAR, 1).with("path", "VK-db.nope")]
    );
    assert_eq!(
        warns("<VK-db.host.x>", &vars),
        vec![Warning::new(RENDER_UNKNOWN_VAR, 1).with("path", "VK-db.host.x")]
    );
    assert_eq!(out("<VK-db.nope>", &vars, &none), "<VK-db.nope>");
}

#[test]
fn warning_lines_dedupe_and_order() {
    let w = warns("a\n\n<VK-b>\r\n<VK-a> <VK-b>\n<VK-a>", &[]);
    let got: Vec<_> = w
        .iter()
        .map(|w| (w.params["line"].as_str(), w.params["name"].as_str()))
        .collect();
    assert_eq!(got, [("3", "VK-b"), ("4", "VK-a")]);
}

#[test]
fn contains_escape_flag() {
    assert!(
        render("a\u{1b}[0m", &[], &Values::new())
            .unwrap()
            .contains_escape
    );
    assert!(
        !render("plain", &[], &Values::new())
            .unwrap()
            .contains_escape
    );
}

#[test]
fn output_limit_refuses_and_boundary_passes() {
    let big = vals(&[("VK-v", s(&"a".repeat(64 * 1024)))]);
    let err = render(&"<VK-v>".repeat(17), &[], &big).unwrap_err();
    assert_eq!(
        err,
        LimitExceeded {
            limit: RenderLimit::OutputBytes,
            max: 1 << 20
        }
    );
    let ok = render(&"<VK-v>".repeat(16), &[], &big).unwrap();
    assert_eq!(ok.output.len(), MAX_OUTPUT_BYTES);
}

#[test]
fn check_values_rules() {
    let code = |v: Values| check_values(&v).unwrap_err().code;
    assert_eq!(code(vals(&[("VK-a", s("\u{1b}"))])), VKS_CONTROL_CHAR);
    assert_eq!(code(vals(&[("a.b", s("x"))])), VKS_BAD_KEY);
    let many: Values = (0..256).map(|i| (format!("VK-k{i}"), s("x"))).collect();
    assert_eq!(code(many), VKS_LIMIT);
    assert!(check_values(&vals(&[("VK-a", s("ok\nmulti"))])).is_ok());
}

fn artifact(blocks: usize) -> mda_core::model::ParsedArtifact {
    let body = match blocks {
        0 => "```sh\ntop <VK-a>\n```\n".to_owned(),
        n => (0..n)
            .map(|i| format!("## B{i}\n```sh\nblock{i} <VK-a>\n```\n"))
            .collect(),
    };
    parse_from_content(
        &format!("---\nartifactType: Snippet\n---\n{body}"),
        "Snippets/a.md",
    )
}

#[test]
fn block_selection() {
    let v = Values::new();
    let nf = |b| Err(RenderError::BlockNotFound(b));
    let text = |p, b, c: Option<&str>| render_artifact(p, b, c, &v).map(|r| r.output);
    let none = artifact(0);
    assert_eq!(text(&none, None, None), Ok("top <VK-a>".into()));
    assert_eq!(text(&none, Some(0), None), nf(Some(0)));
    let one = artifact(1);
    assert_eq!(text(&one, None, None), Ok("block0 <VK-a>".into()));
    assert_eq!(text(&one, Some(0), None), Ok("block0 <VK-a>".into()));
    let two = artifact(2);
    assert_eq!(text(&two, None, None), nf(None));
    assert_eq!(text(&two, Some(5), None), nf(Some(5)));
    assert_eq!(text(&two, Some(1), None), Ok("block1 <VK-a>".into()));
    // `code` replaces the text and its tokens are detected.
    let vals = vals(&[("VK-a", s("A"))]);
    let r = render_artifact(&two, Some(1), Some("hi <VK-a>"), &vals).unwrap();
    assert_eq!(r.output, "hi A");
}

#[test]
fn fuzz_never_panics_and_stays_bounded() {
    for code in common::fuzz_strings(0x4E7D, 5_000, 40) {
        for vars in [Vec::new(), sample_vars()] {
            let r = render(&code, &vars, &Values::new()).unwrap();
            assert!(r.output.len() <= MAX_OUTPUT_BYTES);
        }
    }
}
