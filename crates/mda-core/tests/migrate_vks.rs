//! vks migration rewrite (spec §11.2): legacy bodies → YAML, whole file or nothing.
#![allow(clippy::expect_used, clippy::panic)] // reason: assertion helpers outside #[test] fns

use mda_core::error::{VKS_BAD_KEY, VKS_CONTROL_CHAR, VKS_DUPLICATE_KEY, VKS_LIMIT};
use mda_core::migrate::Skip;
use mda_core::migrate::vks::{VksRewrite, rewrite};

const V: &str = "Variables/v.md";
const FM: &str = "---\nartifactType: Variables\ntitle: V\n---\n\n";

fn ok(c: &str, path: &str) -> VksRewrite {
    rewrite(c, path).expect("no skip").expect("a change")
}

fn skip(c: &str, path: &str) -> Skip {
    rewrite(c, path).expect_err("a skip")
}

fn vars_code(s: &Skip) -> &'static str {
    match s {
        Skip::Vars(e) => e.code,
        other => panic!("not Vars: {other:?}"),
    }
}

#[test]
fn migrates_seeds() {
    let r = ok(
        &format!("{FM}```yaml VKs\n# c\nVK-host=localhost\nVK-port=8080\n```\n"),
        V,
    );
    assert_eq!(
        r.content,
        format!("{FM}```yaml VKs\nVK-host: localhost\nVK-port: '8080'\n```\n")
    );
    assert_eq!((r.fences, r.vars, r.dropped_lines), (1, 2, 1));

    let sets = "## Dev\n\n```yaml VKs\nVK-a=1\n```\n\n## Prod\n\n```yaml VKs\nVK-a=2\n```\n";
    let r = ok(sets, V);
    assert_eq!(
        r.content,
        "## Dev\n\n```yaml VKs\nVK-a: '1'\n```\n\n## Prod\n\n```yaml VKs\nVK-a: '2'\n```\n"
    );
    assert_eq!((r.fences, r.vars), (2, 2));

    let t = "```sh\necho <VK-a>\n```\n\n```yaml VKs\nVK-a=hi there\n```\n";
    assert_eq!(
        ok(t, "Snippets/s.md").content,
        t.replace("VK-a=hi there", "VK-a: hi there")
    );

    let second = "## A\n```sh\nx\n```\n```yaml VKs\nVK-a=1\n```\n```yaml VKs\nVK-b=2\n```\n";
    assert_eq!(
        ok(second, "Snippets/t.md").content,
        second.replacen("VK-a=1", "VK-a: '1'", 1)
    );

    let q = format!("{FM}```yaml VKs\nVK-a=x: y\nVK-b=true\nVK-c=\n```\n");
    assert_eq!(
        ok(&q, V).content,
        format!("{FM}```yaml VKs\nVK-a: 'x: y'\nVK-b: 'true'\nVK-c:\n```\n")
    );

    let same = format!("{FM}```yaml VKs\nVK-a=1```\n");
    assert_eq!(
        ok(&same, V).content,
        format!("{FM}```yaml VKs\nVK-a: '1'\n```\n")
    );

    let flagged = "%%oa:start%%\nReview <VK-repo>.\n%%oa:end%%\n\n```yaml VKs\nVK-repo=mda\n```\n";
    assert_eq!(
        ok(flagged, "AIPrompts/f.md").content,
        flagged.replace("VK-repo=mda", "VK-repo: mda")
    );
}

#[test]
fn crlf_spans_only() {
    let c = format!(
        "{}```yaml VKs\r\nVK-a=x\r\n```\r\n",
        FM.replace('\n', "\r\n")
    );
    let r = ok(&c, V);
    let start = c.find("VK-a").expect("body");
    let end = c
        .find("```\r\n")
        .map(|_| c.len() - "```\r\n".len())
        .expect("end");
    assert_eq!(r.content.get(..start), c.get(..start));
    assert_eq!(
        r.content.get(r.content.len() - (c.len() - end)..),
        c.get(end..)
    );
    assert!(r.content.contains("VK-a: x\r\n```"));
    assert!(!r.content.replace("\r\n", "").contains('\n'));

    let lf_open = format!("{FM}```yaml VKs\nVK-a=x\n```\n")
        .replace('\n', "\r\n")
        .replace("VKs\r\n", "VKs\n");
    assert!(
        ok(&lf_open, V)
            .content
            .contains("```yaml VKs\nVK-a: x\n```")
    );
}

#[test]
fn refusals_whole_file() {
    let bad = format!("{FM}```yaml VKs\nVK-a=1\n```\n\n## B\n```yaml VKs\nVK-a.b=1\n```\n");
    assert_eq!(vars_code(&skip(&bad, V)), VKS_BAD_KEY);
    assert_eq!(
        vars_code(&skip(&format!("{FM}```yaml VKs\nVK-a=1\nVK-a=2\n```\n"), V)),
        VKS_DUPLICATE_KEY
    );
    let ctrl = skip(&format!("{FM}```yaml VKs\nVK-a=a\u{1}b\n```\n"), V);
    assert_eq!(vars_code(&ctrl), VKS_CONTROL_CHAR);

    let big = format!("{FM}```yaml VKs\nVK-a={}\n```\n", "x".repeat(70_000));
    let Skip::Vars(e) = skip(&big, V) else {
        panic!()
    };
    assert_eq!(
        (e.code, e.params["limit"].as_str()),
        (VKS_LIMIT, "body_bytes")
    );

    let quotes = format!("{FM}```yaml VKs\nVK-a={}\n```\n", "'".repeat(32_770));
    let Skip::Vars(e) = skip(&quotes, V) else {
        panic!()
    };
    assert_eq!(
        (e.code, e.params["limit"].as_str()),
        (VKS_LIMIT, "body_bytes")
    );

    // 200 vars whose LF emission fits under 64 KiB; the CRLF swap pushes it over.
    let body: String = (0..200)
        .map(|i| format!("VK-v{i:03}={}\n", "y".repeat(317)))
        .collect();
    let lf = format!("{FM}```yaml VKs\n{body}```\n");
    let r = ok(&lf, V);
    let emitted = r.content.len() - lf.len() + body.len();
    assert!(emitted <= 65_536 && emitted + 200 > 65_536, "{emitted}");
    let crlf = lf.replace('\n', "\r\n");
    let Skip::Vars(e) = skip(&crlf, V) else {
        panic!()
    };
    assert_eq!(e.code, VKS_LIMIT);
}

#[test]
fn not_candidates() {
    for body in ["VK-a: 1\n", "# todo\n", "", "#\n\n  \n"] {
        let c = format!("{FM}```yaml VKs\n{body}```\n");
        assert_eq!(rewrite(&c, V), Ok(None), "{body:?}");
    }
    let mixed = format!("{FM}```yaml VKs\nVK-top: t\n```\n\n## D\n\n```yaml VKs\nVK-a=1\n```\n");
    assert_eq!(ok(&mixed, V).content, mixed.replace("VK-a=1", "VK-a: '1'"));
}

#[test]
fn unfenced() {
    let sn = "---\nartifactType: Snippet\n---\n\n```sh\necho <VK-a>\n```\n\n";
    assert_eq!(
        skip(&format!("{sn}vars:\nVK-a=1\n"), "Snippets/u.md"),
        Skip::UnfencedVars
    );
    assert_eq!(
        rewrite(&format!("{sn}vars:\n# only comment\n"), "Snippets/u.md"),
        Ok(None)
    );
    assert_eq!(
        rewrite(
            &format!("{sn}```yaml VKs\nVK-a: 1\n```\nvars:\nVK-b=1\n"),
            "Snippets/u.md"
        ),
        Ok(None)
    );
}

#[test]
fn guard_flagged_inside() {
    let c =
        "---\nartifactType: AIPrompt\n---\n\n%%oa:start%%\n```yaml VKs\nVK-a=1\n```\n%%oa:end%%\n";
    assert_eq!(skip(c, "AIPrompts/i.md"), Skip::RoundTrip { field: "vars" });
}

#[test]
fn guard_keeps_existing_errors() {
    let c = "## Bad\n```yaml VKs\nVK-a: [\n```\n## Old\n```yaml VKs\nVK-b=1\n```\n";
    let r = ok(c, V);
    assert_eq!(r.content, c.replace("VK-b=1", "VK-b: '1'"));
    assert_eq!(r.fences, 1);
}

#[test]
fn idempotent() {
    let cases = [
        (format!("{FM}```yaml VKs\n# c\nVK-a=1\n```\n"), V),
        (
            "## D\n```yaml VKs\nVK-a=1\n```\n## P\n```yaml VKs\nVK-a=2\n```\n".to_owned(),
            V,
        ),
        (format!("{FM}```yaml VKs\r\nVK-a=x\r\n```\r\n"), V),
    ];
    for (c, p) in cases {
        let r = ok(&c, p);
        assert_eq!(rewrite(&r.content, p), Ok(None));
    }
}

#[test]
fn rewrite_is_linear() {
    let c: String = (0..20_000)
        .map(|i| format!("## s{i}\n```yaml VKs\nVK-a=1\n```\n"))
        .collect();
    let t = std::time::Instant::now();
    let r = ok(&c, V);
    assert_eq!(r.fences, 20_000);
    assert!(t.elapsed().as_secs() < 2, "{:?}", t.elapsed());
}
