//! Join engine (ER-7, spec §4.3): techniques, the line scanner's refusals, multi-line values, limits.
#![allow(clippy::unwrap_used, clippy::panic)] // reason: test assertions unwrap known-good joins and panic on a wrong outcome

use mda_core::error::{LimitExceeded, RenderLimit};
use mda_core::model::ParsedVar;
use mda_core::render::{
    JoinError, JoinTechnique as T, MAX_OUTPUT_BYTES, UnjoinReason as R, Unjoinable, Values,
    check_join_values, join_lines,
};
use mda_core::vks::{VksList, VksRecord, VksValue};

fn refused(src: &str) -> Unjoinable {
    match join_lines(src, T::And) {
        Err(JoinError::Unjoinable(u)) => u,
        other => panic!("expected refusal for {src:?}, got {other:?}"),
    }
}

fn reason(line: &str) -> R {
    refused(line).reason
}

fn ok(line: &str) {
    assert!(join_lines(line, T::And).is_ok(), "{line:?} should join");
}

#[test]
fn and_joins_lines() {
    assert_eq!(join_lines("a\nb\nc", T::And).unwrap(), "a && b && c");
}

#[test]
fn techniques_on_messy_input() {
    let src = "a\n\n  b  \r\nc\n";
    assert_eq!(join_lines(src, T::And).unwrap(), "a && b && c");
    assert_eq!(join_lines(src, T::Seq).unwrap(), "a ; b ; c");
    assert_eq!(join_lines(src, T::Continuation).unwrap(), "a \\\nb \\\nc");
    assert_eq!(join_lines(src, T::Pwsh).unwrap(), "a ; b ; c");
    assert_eq!(join_lines(src, T::Cmd).unwrap(), "a & b & c");
}

#[test]
fn one_line_unchanged_all_blank_empty() {
    assert_eq!(join_lines("echo a", T::Seq).unwrap(), "echo a");
    assert_eq!(join_lines(" \n\n \r\n", T::And).unwrap(), "");
    assert_eq!(join_lines("", T::And).unwrap(), "");
}

#[test]
fn bare_cr_survives() {
    assert_eq!(join_lines("a\rb\nc", T::And).unwrap(), "a\rb && c");
}

#[test]
fn line_counts_blank_lines() {
    let u = refused("a\n\n\necho x # c");
    assert_eq!((u.line, u.reason, u.name), (4, R::Comment, None));
}

#[test]
fn heredoc() {
    assert_eq!(reason("cat <<EOF"), R::Heredoc);
    assert_eq!(reason("cat <<'EOF'"), R::Heredoc);
    assert_eq!(reason("x = @'"), R::Heredoc);
    assert_eq!(reason("x = @\""), R::Heredoc);
    ok("echo '<<' a");
    ok("echo \"<<\"");
}

#[test]
fn comment() {
    for l in [
        "# x",
        "echo a # x",
        "a;# x",
        "a&&# x",
        "(# x",
        "a|#x",
        "rem",
        "REM x",
        "a & rem x",
        "a && Rem x",
        "a || rem",
        "(rem x",
        ":: x",
        ":label",
        "echo a # it's",
    ] {
        assert_eq!(reason(l), R::Comment, "{l:?}");
    }
    for l in [
        "echo a#b",
        "echo '# x'",
        "echo \"a # b\"",
        "echo remark",
        "echo rem",
    ] {
        ok(l);
    }
}

#[test]
fn open_quote() {
    for l in ["echo 'a", "echo \"a", "echo `a", "echo \"a\\\""] {
        assert_eq!(reason(l), R::OpenQuote, "{l:?}");
    }
    for l in ["echo \"it's\"", "echo `a` b", "echo a\\'b", "echo 'a\\'"] {
        ok(l);
    }
}

#[test]
fn cmd_quotes_are_per_line() {
    // Joined, line 2's quoted text would leave its quotes and execute under cmd.
    let u = refused("echo '\"'\nx \" & del y & \"");
    assert_eq!((u.line, u.reason), (1, R::OpenQuote));
    for l in ["echo C:\\\"", "echo ^\"a\"", "echo \"a\\\"b\""] {
        let u = refused(&format!("ok\n{l}"));
        assert_eq!((u.line, u.reason), (2, R::OpenQuote), "{l:?}");
    }
    ok("echo ^\"a^\"");
}

#[test]
fn rem_variants() {
    for l in ["rem.x", "REM/x", "rem:x", "a & rem.x"] {
        assert_eq!(reason(l), R::Comment, "{l:?}");
    }
    ok("echo remx");
    ok("remx");
}

#[test]
fn trailing_operator() {
    for l in ["a \\", "a ^", "a &&", "a ||", "a |", "a &", "a ;", "a `"] {
        let r = reason(l);
        assert!(r == R::TrailingOperator || r == R::OpenQuote, "{l:?} {r:?}");
    }
    for l in ["a \\", "a ^", "a &&", "a ||", "a |", "a &", "a ;"] {
        assert_eq!(reason(l), R::TrailingOperator, "{l:?}");
    }
    ok("echo a&b");
    ok("echo a;b");
}

#[test]
fn block_keyword() {
    for l in [
        "if x; then",
        "IF x echo a",
        "Foreach",
        "else{",
        "for x in y",
        "case x",
        "fi",
        "done",
        "try",
        "switch x",
        "function f",
        "{ a",
        "a }",
        "(a",
        "a )",
        "if(",
        "While true",
    ] {
        assert_eq!(reason(l), R::BlockKeyword, "{l:?}");
    }
    // `done;` / `fi;` end in `;`: trailing_operator is checked first (spec order).
    assert_eq!(reason("done;"), R::TrailingOperator);
    assert_eq!(reason("fi;"), R::TrailingOperator);
    for l in ["echo if", "iffy x", "format c", "echo done", "ifconfig"] {
        ok(l);
    }
}

#[test]
fn first_match_wins_and_stops_at_comment() {
    assert_eq!(reason("cat <<EOF # c"), R::Heredoc);
    assert_eq!(reason("echo 'a # b' # c"), R::Comment);
    assert_eq!(reason("if x # it's"), R::Comment);
    // comment hides the later heredoc
    assert_eq!(reason("echo a # <<"), R::Comment);
}

#[test]
fn hostile_refusal_yields_no_output() {
    let r = join_lines("echo ok\ncat <<EOF\nrm -rf x\nEOF", T::Seq);
    assert!(matches!(
        r,
        Err(JoinError::Unjoinable(Unjoinable { line: 2, .. }))
    ));
}

#[test]
fn output_limit() {
    let src = "a\n".repeat(400_000);
    assert!(src.len() <= MAX_OUTPUT_BYTES);
    let e = join_lines(&src, T::And).unwrap_err();
    assert_eq!(
        e,
        JoinError::Limit(LimitExceeded {
            limit: RenderLimit::OutputBytes,
            max: MAX_OUTPUT_BYTES
        })
    );
}

fn s(x: &str) -> VksValue {
    VksValue::Str(x.into())
}

fn var(n: &str, v: VksValue) -> ParsedVar {
    ParsedVar {
        name: n.into(),
        value: v,
    }
}

#[test]
fn multiline_values() {
    let vars = vec![
        var("VK-a", s("one")),
        var("VK-ml", s("x\ny")),
        var(
            "VK-rec",
            VksValue::Record(VksRecord(vec![("f".into(), s("p\nq"))])),
        ),
        var(
            "VK-list",
            VksValue::List(VksList::Strings(vec!["ok".into(), "b\nc".into()])),
        ),
    ];
    let none = Values::new();
    assert!(check_join_values("echo <VK-a>", &vars, &none).is_ok());
    // unused multi-line var passes
    assert!(check_join_values("echo <VK-a>", &vars, &none).is_ok());
    let u = check_join_values("echo <VK-ml>", &vars, &none).unwrap_err();
    assert_eq!(
        u,
        Unjoinable {
            line: 0,
            reason: R::MultilineValue,
            name: Some("VK-ml".into())
        }
    );
    assert_eq!(
        check_join_values("<VK-rec.f>", &vars, &none)
            .unwrap_err()
            .name,
        Some("VK-rec".into())
    );
    assert_eq!(
        check_join_values("<VK-list>", &vars, &none)
            .unwrap_err()
            .name,
        Some("VK-list".into())
    );
    // typed value overrides the default; a clean typed value over a multi-line default passes
    let mut typed = Values::new();
    typed.insert("VK-ml".into(), s("clean"));
    assert!(check_join_values("<VK-ml>", &vars, &typed).is_ok());
    typed.insert("VK-a".into(), s("l1\nl2"));
    assert_eq!(
        check_join_values("<VK-a>", &vars, &typed).unwrap_err().name,
        Some("VK-a".into())
    );
}
