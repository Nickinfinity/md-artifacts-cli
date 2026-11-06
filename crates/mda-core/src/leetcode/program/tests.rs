//! Ports of `program-config.test.ts` plus the Rust-only Windows-absolute rule (Q-32).

use super::*;
use crate::leetcode::ProgramChannel as C;
use crate::leetcode::config::{extract_config_blocks, split_frontmatter};
use crate::parse::text::js_lines;

fn run(lines: &[&str]) -> (Option<ProgramConfig>, Vec<Warning>) {
    let mut w = vec![];
    (parse_program(lines, &mut w), w)
}
fn codes(w: &[Warning]) -> Vec<&'static str> {
    w.iter().map(|x| x.code).collect()
}
fn param<'a>(w: &'a Warning, k: &str) -> &'a str {
    w.params.get(k).map_or("", String::as_str)
}

#[test]
fn absent_block_is_none() {
    let (c, w) = run(&["title: X"]);
    assert_eq!((c, w), (None, vec![]));
}

#[test]
fn valid_channel_kept() {
    let (c, w) = run(&["program:", "  channel: stdin"]);
    assert_eq!(c.map(|c| c.channel), Some(C::Stdin));
    assert!(w.is_empty());
}

#[test]
fn unknown_channel_degrades_with_param() {
    for raw in ["bogus", "argv; rm -rf / #"] {
        let line = format!("  channel: {raw}");
        let (c, w) = run(&["program:", &line]);
        assert_eq!(c.map(|c| c.channel), Some(C::Argv));
        assert_eq!(codes(&w), ["leetcode.program.unknown_channel"]);
        assert_eq!(param(&w[0], "channel"), raw);
    }
}

#[test]
fn missing_channel_warns() {
    let (c, w) = run(&["program:", "  entry: main.py"]);
    let c = c.unwrap();
    assert_eq!((c.channel, c.entry.as_deref()), (C::Argv, Some("main.py")));
    assert_eq!(codes(&w), ["leetcode.program.no_channel"]);
    assert!(w[0].params.is_empty());
}

#[test]
fn entry_reasons() {
    let cases = [
        ("src/main.py", None),
        ("/etc/passwd", Some("absolute")),
        ("\\\\srv\\x", Some("absolute")),
        ("\\x", Some("absolute")),
        ("C:\\x", Some("absolute")),
        ("c:/x", Some("absolute")),
        ("../../escape", Some("parent_segment")),
        ("a\\..\\b", Some("parent_segment")),
        ("client/node_modules/.bin/evil", Some("reserved_segment")),
        ("NODE_MODULES/x", Some("reserved_segment")),
        ("node_moduleſ/x.js", Some("reserved_segment")),
    ];
    for (entry, reason) in cases {
        let line = format!("  entry: {entry}");
        let (c, w) = run(&["program:", "  channel: argv", &line]);
        let c = c.unwrap();
        match reason {
            None => assert_eq!((c.entry.as_deref(), w.len()), (Some(entry), 0), "{entry}"),
            Some(r) => {
                assert_eq!(c.entry, None, "{entry}");
                assert_eq!(codes(&w), ["leetcode.program.entry_rejected"], "{entry}");
                assert_eq!((param(&w[0], "entry"), param(&w[0], "reason")), (entry, r));
            }
        }
    }
}

#[test]
fn empty_entry_is_rejected_empty() {
    let (c, w) = run(&["program:", "  channel: argv", "  entry:"]);
    assert_eq!(c.unwrap().entry, None);
    assert_eq!(param(&w[0], "reason"), "empty");
}

#[test]
fn hostile_long_entry_is_fast_and_truncated() {
    let line = format!("  entry: ../{}", "a".repeat(10_000));
    let t = std::time::Instant::now();
    let (c, w) = run(&["program:", "  channel: argv", &line]);
    assert!(t.elapsed().as_millis() < 500);
    assert_eq!(c.unwrap().entry, None);
    assert_eq!(param(&w[0], "entry").chars().count(), 200);
}

#[test]
fn flags_inline_block_and_absent() {
    let v = |l: &[&str]| run(l).0.unwrap().flags;
    let want = Some(vec!["--nums".to_owned(), "--target".to_owned()]);
    assert_eq!(v(&["program:", "  flags: [--nums, --target]"]), want);
    assert_eq!(
        v(&["program:", "  flags:", "    - --nums", "    - --target"]),
        want
    );
    assert_eq!(v(&["program:", "  channel: argv"]), None);
    assert_eq!(
        v(&["program:", "  flags: ['a', \"b\"]"]).unwrap(),
        ["a", "b"]
    );
}

#[test]
fn unknown_fields_ignored() {
    let (c, _) = run(&[
        "program:",
        "  __proto__: pwn",
        "  constructor: evil",
        "  channel: argv",
    ]);
    assert_eq!(
        c,
        Some(ProgramConfig {
            channel: C::Argv,
            entry: None,
            flags: None
        })
    );
}

#[test]
fn fully_declared() {
    let (c, w) = run(&[
        "program:",
        "  channel: flags",
        "  entry: src/main.py",
        "  flags: [--nums, --target]",
    ]);
    assert_eq!(c.unwrap().channel, C::Flags);
    assert!(w.is_empty());
}

/// `conformance/leetcode/program-config/*`: the `program` field and the warnings.
#[test]
fn conformance_cases() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/leetcode/program-config"
    );
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "md") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        let raw = extract_config_blocks(split_frontmatter(&src).1).raw;
        let lines: Vec<&str> = js_lines(&raw).collect();
        let (c, w) = run(&lines);
        let exp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(p.with_extension("expected.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            exp.get("program").cloned().unwrap_or_default(),
            "{p:?}"
        );
        let got: Vec<_> = w.iter().map(|x| serde_json::to_value(x).unwrap()).collect();
        // Warnings sort: the case files list them in emission order.
        let want: Vec<_> = exp["warnings"].as_array().unwrap().clone();
        assert_eq!(got, want, "{p:?}");
        n += 1;
    }
    assert_eq!(n, 6);
}
