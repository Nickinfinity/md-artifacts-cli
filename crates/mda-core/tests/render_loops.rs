//! Directive engine, T3.3: `each`/`end`, scopes, block mode, `join`, failures, limits.
#![allow(clippy::indexing_slicing, clippy::unwrap_used)] // reason: test assertions index fixed shapes and unwrap known-good renders

mod common;
use mda_core::error::{
    LimitExceeded, RENDER_EACH_NOT_LIST, RENDER_JOIN_EMPTY, RENDER_JOIN_RECORD, RENDER_SELF_NESTED,
    RENDER_UNMATCHED_END, RENDER_UNTERMINATED, RenderLimit,
};
use mda_core::model::ParsedVar;
use mda_core::render::{MAX_OUTPUT_BYTES, Values, Warning, render};
use mda_core::vks::{VksList, VksRecord, VksValue};

fn s(x: &str) -> VksValue {
    VksValue::Str(x.into())
}

fn strs(items: &[&str]) -> VksValue {
    VksValue::List(VksList::Strings(
        items.iter().map(|i| (*i).into()).collect(),
    ))
}

fn rec(f: &[(&str, VksValue)]) -> VksRecord {
    VksRecord(f.iter().map(|(k, v)| ((*k).into(), v.clone())).collect())
}

fn var(name: &str, v: VksValue) -> ParsedVar {
    ParsedVar {
        name: name.into(),
        value: v,
    }
}

/// Spec §9.3's `VK-env`, `VK-users`, `VK-db`.
fn sample() -> Vec<ParsedVar> {
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
        var("VK-db", VksValue::Record(rec(&[("host", s("localhost"))]))),
        var("VK-none", strs(&[])),
    ]
}

fn out(code: &str) -> String {
    render(code, &sample(), &Values::new()).unwrap().output
}

fn warns(code: &str) -> Vec<Warning> {
    render(code, &sample(), &Values::new()).unwrap().warnings
}

fn w(code: &'static str, line: &str, path: &str) -> Warning {
    Warning::new(code, line.parse().unwrap()).with("path", path)
}

fn limit(code: &str, vars: &[ParsedVar]) -> LimitExceeded {
    render(code, vars, &Values::new()).unwrap_err()
}

#[test]
fn worked_example_is_byte_exact() {
    let code = "const users = [\n<VK-each:users:\",\">\n  { id: <VK-users.id>, name: \"<VK-users.name>\", tags: [<VK-each:users.tags:\", \">\"<VK-users.tags>\"<VK-end:users.tags>] }\n<VK-end:users>\n];\n// all tags: <VK-join:users.tags:\" | \">";
    let want = "const users = [\n  { id: 1, name: \"Alice Smith\", tags: [\"core\", \"ops\"] },\n  { id: 2, name: \"Bob Jones\", tags: [] }\n];\n// all tags: core | ops";
    let r = render(code, &sample(), &Values::new()).unwrap();
    assert_eq!(r.output, want);
    assert!(r.warnings.is_empty());
}

#[test]
fn inline_block_join_and_scopes() {
    assert_eq!(
        out("[<VK-each:env:\", \">\"<VK-env>\"<VK-end:env>]"),
        "[\"dev\", \"prod\"]"
    );
    assert_eq!(out("[<VK-each:none>x<VK-end:none>]"), "[]");
    assert_eq!(
        out("a\n<VK-each:env:\",\">\n- <VK-env>\n<VK-end:env>\nb"),
        "a\n- dev,\n- prod\nb"
    );
    assert_eq!(
        out("a\n<VK-each:env>\n- <VK-env>\n<VK-end:env>\nb"),
        "a\n- dev\n- prod\nb"
    );
    assert_eq!(
        out("i:\r\n<VK-each:env>\r\n- <VK-env>\r\n<VK-end:env>\r\ndone"),
        "i:\r\n- dev\r\n- prod\r\ndone"
    );
    assert_eq!(out("a\n<VK-each:none>\nx\n<VK-end:none>\nb"), "a\nb");
    assert_eq!(
        out(
            "<VK-each:users>\n<VK-users.name>:<VK-each:users.tags:\"/\">[<VK-users.tags>]<VK-end:users.tags>\n<VK-end:users>\nend"
        ),
        "Alice Smith:[core]/[ops]\nBob Jones:\nend"
    );
    assert_eq!(out("<VK-join:users.tags:\" | \">"), "core | ops");
    assert_eq!(out("<VK-join:users.tags>"), "core, ops");
    assert_eq!(
        out("<VK-each:users:\"; \">(<VK-users.name>: <VK-join:users.tags>)<VK-end:users>"),
        "(Alice Smith: core, ops); (Bob Jones: <VK-join:users.tags>)"
    );
}

#[test]
fn h15_literals_and_warnings() {
    assert_eq!(out("<VK-each:x:\"a>b\">"), "<VK-each:x:\"a>b\">");
    assert!(warns("<VK-each:x:\"a>b\">").is_empty());
    assert_eq!(out("<VK-each:env>x"), "<VK-each:env>x");
    assert_eq!(
        warns("<VK-each:env>x"),
        [w(RENDER_UNTERMINATED, "1", "VK-env")]
    );
    assert_eq!(
        out("<VK-each:users>[<VK-each:env>B<VK-end:users>C<VK-end:env>]"),
        "<VK-each:users>[B<VK-end:users>CB<VK-end:users>C]"
    );
    assert_eq!(
        warns("<VK-each:users>[<VK-each:env>B<VK-end:users>C<VK-end:env>]"),
        [
            w(RENDER_UNMATCHED_END, "1", "VK-users"),
            w(RENDER_UNTERMINATED, "1", "VK-users")
        ]
    );
    let code = "<VK-each:env>(<VK-each:env>x<VK-end:env>)<VK-end:env>";
    assert_eq!(
        out(code),
        "(<VK-each:env>x<VK-end:env>)(<VK-each:env>x<VK-end:env>)"
    );
    assert_eq!(warns(code), [w(RENDER_SELF_NESTED, "1", "VK-env")]);
    assert_eq!(out("<VK-each:db>x<VK-end:db>"), "<VK-each:db>x<VK-end:db>");
    assert_eq!(
        warns("<VK-each:db>x<VK-end:db>"),
        [w(RENDER_EACH_NOT_LIST, "1", "VK-db")]
    );
    assert_eq!(out("<VK-join:db>"), "<VK-join:db>");
    assert_eq!(warns("<VK-join:db>"), [w(RENDER_JOIN_RECORD, "1", "VK-db")]);
    assert_eq!(out("<VK-join:users.missing>"), "<VK-join:users.missing>");
    assert_eq!(
        warns("<VK-join:users.missing>"),
        [w(RENDER_JOIN_EMPTY, "1", "VK-users.missing")]
    );
}

fn numbered(n: usize) -> VksValue {
    VksValue::List(VksList::Strings((0..n).map(|i| i.to_string()).collect()))
}

#[test]
fn limits_refuse_and_return_nothing() {
    // iterations: 1 000 x 1 000 nested.
    let vars = [var("VK-o", numbered(1000)), var("VK-p", numbered(1000))];
    let e = limit("<VK-each:o><VK-each:p>x<VK-end:p><VK-end:o>", &vars);
    assert_eq!(e.limit, RenderLimit::Iterations);

    // depth: 65 distinct open loops.
    let deep: String = (0..65).map(|i| format!("<VK-each:a{i}>")).collect();
    assert_eq!(limit(&deep, &[]).limit, RenderLimit::Depth);
    let ok: String = (0..64).map(|i| format!("<VK-each:a{i}>")).collect();
    assert!(render(&ok, &[], &Values::new()).is_ok());

    // 100 000 nested markers: refused at 65, no recursion (small stack).
    let huge: String = (0..100_000).map(|i| format!("<VK-each:a{i}>")).collect();
    let r = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || render(&huge, &[], &Values::new()))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(r.unwrap_err().limit, RenderLimit::Depth);

    // steps: empty inner loops visited 1 000 x 1 001 times, zero output.
    let inner: String = (0..1001).map(|_| "<VK-each:e>x<VK-end:e>").collect();
    let code = format!("<VK-each:o>{inner}<VK-end:o>");
    let vars = [var("VK-o", numbered(1000)), var("VK-e", strs(&[]))];
    assert_eq!(limit(&code, &vars).limit, RenderLimit::Steps);

    // steps: repeated join walks that emit nothing.
    let big = VksValue::List(VksList::Records(
        (0..1000).map(|_| rec(&[("y", s("1"))])).collect(),
    ));
    let vars = [var("VK-o", numbered(1000)), var("VK-big", big)];
    let e = limit("<VK-each:o><VK-join:big.x><VK-end:o>", &vars);
    assert_eq!(e.limit, RenderLimit::Steps);

    // output_bytes before every append, separators included.
    let vars = [var("VK-o", numbered(1000))];
    let code = format!("<VK-each:o:\"{}\">x<VK-end:o>", "ab".repeat(600));
    assert_eq!(limit(&code, &vars).limit, RenderLimit::OutputBytes);
}

#[test]
fn fuzz_never_panics_and_stays_in_limit() {
    let frags = [
        "<VK-each:users>",
        "<VK-end:users>",
        "<VK-join:users.tags>",
        "<VK-users.name>",
    ];
    for (i, f) in common::fuzz_strings(0xD1E7, 5_000, 40)
        .into_iter()
        .enumerate()
    {
        let code = format!("{f}{}{f}", frags[i % frags.len()]);
        if let Ok(r) = render(&code, &sample(), &Values::new()) {
            assert!(r.output.len() <= MAX_OUTPUT_BYTES);
        }
    }
}
