//! Ports of `packages-parser.test.ts` plus `MAX_PACKAGES` 8/9 and the shared path rule.

use super::*;
use crate::leetcode::config::{extract_config_blocks, split_frontmatter};
use crate::parse::text::js_lines;

fn run(lines: &[&str]) -> (Vec<PackageSpec>, Vec<Warning>) {
    let mut w = vec![];
    (parse_packages(lines, &mut w), w)
}
fn codes(w: &[Warning]) -> Vec<&'static str> {
    w.iter().map(|x| x.code).collect()
}
fn p<'a>(w: &'a Warning, k: &str) -> &'a str {
    w.params.get(k).map_or("", String::as_str)
}
/// One valid entry named `name` with extra lines.
fn entry(name: &str, extra: &[&str]) -> Vec<String> {
    let mut v = vec![
        format!("  - name: {name}"),
        "    dir: d".to_owned(),
        "    install: [\"i\"]".to_owned(),
        "    start: [\"s\"]".to_owned(),
    ];
    v.extend(extra.iter().map(|s| (*s).to_owned()));
    v
}
fn block(entries: &[Vec<String>]) -> Vec<String> {
    let mut v = vec!["packages:".to_owned()];
    v.extend(entries.iter().flatten().cloned());
    v
}
fn run_s(lines: &[String]) -> (Vec<PackageSpec>, Vec<Warning>) {
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    run(&refs)
}

#[test]
fn absent_is_empty() {
    assert_eq!(run(&["title: X"]), (vec![], vec![]));
}

#[test]
fn not_a_list_warns_with_got() {
    let (pk, w) = run(&["packages: api"]);
    assert!(pk.is_empty());
    assert_eq!(codes(&w), ["leetcode.packages.not_a_list"]);
    assert_eq!(p(&w[0], "got"), "api");
}

#[test]
fn max_packages_8_ok_9_dropped() {
    let names: Vec<String> = (0..9).map(|i| format!("p{i}")).collect();
    let b8 = block(&names[..8].iter().map(|n| entry(n, &[])).collect::<Vec<_>>());
    let (pk, w) = run_s(&b8);
    assert_eq!((pk.len(), w.len()), (8, 0));
    let b9 = block(&names.iter().map(|n| entry(n, &[])).collect::<Vec<_>>());
    let (pk, w) = run_s(&b9);
    assert!(pk.is_empty());
    assert_eq!(codes(&w), ["leetcode.packages.too_many"]);
    assert_eq!((p(&w[0], "count"), p(&w[0], "max")), ("9", "8"));
}

#[test]
fn two_packages_all_fields() {
    let lines = [
        "packages:",
        "  - name: api",
        "    dir: server",
        "    install: [\"pip\", \"install\"]",
        "    start: [\"uvicorn\", \"--port\", \"${PORT}\"]",
        "    ready: \"Uvicorn running\"",
        "  - name: web",
        "    dir: web",
        "    install: [\"npm\", \"ci\"]",
        "    start: [\"npm\", \"run\", \"--port\", \"${PORT}\"]",
        "    dependsOn: [api]",
        "    exposeAs:",
        "      VITE_API_URL: \"http://127.0.0.1:${PORT}/v1\"",
    ];
    let (pk, w) = run(&lines);
    assert!(w.is_empty());
    assert_eq!(pk[0].ready.as_deref(), Some("Uvicorn running"));
    assert_eq!(pk[0].start, ["uvicorn", "--port", "${PORT}"]);
    assert_eq!(pk[1].depends_on.as_deref(), Some(&["api".to_owned()][..]));
    assert_eq!(
        pk[1].expose_as.as_ref().unwrap().0,
        [(
            "VITE_API_URL".to_owned(),
            "http://127.0.0.1:${PORT}/v1".to_owned()
        )]
    );
}

#[test]
fn missing_name_drops_entry_siblings_survive() {
    let mut b = block(&[entry("a", &[])]);
    b.extend([
        "  - dir: x".into(),
        "    install: [\"i\"]".into(),
        "    start: [\"s\"]".into(),
    ]);
    b.extend(entry("b", &[]));
    let (pk, w) = run_s(&b);
    assert_eq!(
        pk.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(codes(&w), ["leetcode.packages.missing_name"]);
}

#[test]
fn argv_refusals() {
    let cases = [
        (
            "    install: pip install",
            "install",
            "leetcode.packages.argv_not_array",
        ),
        (
            "    start: \"npm run dev\"",
            "start",
            "leetcode.packages.argv_not_array",
        ),
        ("    install: []", "install", "leetcode.packages.argv_empty"),
        ("    start: []", "start", "leetcode.packages.argv_empty"),
        (
            "    start: [\"echo\", \"${HOME}\"]",
            "start",
            "leetcode.packages.argv_substitution",
        ),
    ];
    for (line, field, code) in cases {
        let mut e = entry("n", &[]);
        e.retain(|l| !l.trim_start().starts_with(field));
        e.push(line.to_owned());
        let (pk, w) = run_s(&block(&[e]));
        assert!(pk.is_empty(), "{line}");
        assert_eq!(codes(&w), [code], "{line}");
        assert_eq!((p(&w[0], "name"), p(&w[0], "field")), ("n", field));
    }
    let mut e = entry("n", &[]);
    e.retain(|l| !l.contains("install"));
    let (_, w) = run_s(&block(&[e]));
    assert_eq!(codes(&w), ["leetcode.packages.missing_field"]);
    let mut e = entry("n", &[]);
    e.retain(|l| !l.contains("start"));
    e.push("    install: [\"x\"]".into());
    let (_, w) = run_s(&block(&[e]));
    assert_eq!(
        (codes(&w), p(&w[0], "field")),
        (vec!["leetcode.packages.missing_field"], "start")
    );
    let (_, w) = run(&[
        "packages:",
        "  - name: s",
        "    dir: d",
        "    install: [\"i\"]",
        "    start: [\"a\", \"${HOME}\"]",
    ]);
    assert_eq!(p(&w[0], "element"), "${HOME}");
}

#[test]
fn dir_reasons() {
    for (dir, reason) in [
        ("", "empty"),
        ("/srv", "absolute"),
        ("C:\\x", "absolute"),
        ("\\\\h\\s", "absolute"),
        ("../..", "parent_segment"),
        ("NODE_MODULES/react", "reserved_segment"),
        ("a/node_moduleſ", "reserved_segment"),
    ] {
        let mut e = entry("n", &[]);
        e[1] = format!("    dir: {dir}");
        let (pk, w) = run_s(&block(&[e]));
        assert!(pk.is_empty(), "{dir}");
        assert_eq!(codes(&w), ["leetcode.packages.dir_rejected"], "{dir}");
        assert_eq!(
            (p(&w[0], "dir"), p(&w[0], "reason"), p(&w[0], "name")),
            (dir, reason, "n")
        );
    }
}

#[test]
fn duplicate_name_and_cycles_drop_block() {
    let (pk, w) = run_s(&block(&[entry("api", &[]), entry("api", &[])]));
    assert!(pk.is_empty());
    assert_eq!(codes(&w), ["leetcode.packages.duplicate_name"]);
    assert_eq!(p(&w[0], "name"), "api");

    let (pk, w) = run_s(&block(&[
        entry("a", &["    dependsOn: [b]"]),
        entry("b", &["    dependsOn: [a]"]),
    ]));
    assert!(pk.is_empty());
    assert_eq!(codes(&w), ["leetcode.packages.cycle"]);
    assert_eq!(p(&w[0], "path"), "a, b, a");

    let (_, w) = run_s(&block(&[entry("a", &["    dependsOn: [a]"])]));
    assert_eq!(p(&w[0], "path"), "a, a");

    // chain and unknown reference: no cycle
    let (pk, w) = run_s(&block(&[
        entry("a", &["    dependsOn: [b, ghost]"]),
        entry("b", &["    dependsOn: [c]"]),
        entry("c", &[]),
    ]));
    assert_eq!((pk.len(), w.len()), (3, 0));
}

#[test]
fn expose_names_and_values() {
    let mut e = entry("api", &["    exposeAs:"]);
    for l in [
        "lower: \"http://127.0.0.1:${PORT}\"",
        "LD_PRELOAD: \"http://127.0.0.1:${PORT}\"",
        "DYLD_X: \"http://127.0.0.1:${PORT}\"",
        "PORT: \"http://127.0.0.1:${PORT}\"",
        "NODE_PATH: \"http://127.0.0.1:${PORT}\"",
        "CARGO_TARGET_DIR: \"http://127.0.0.1:${PORT}\"",
        "JDK_JAVA_OPTIONS: \"http://127.0.0.1:${PORT}\"",
        "API_PORT: \"${PORT}\"",
        "A1: \"/tmp/x\"",
        "A2: \"-javaagent:x\"",
        "A3: \"http://evil.com:${PORT}\"",
        "A4: \"http://127.0.0.1:${PORT}x\"",
        "A5: \"http://127.0.0.1:${PORT}/a b\"",
        "__proto__: \"http://127.0.0.1:${PORT}\"",
        "OK_URL: \"http://127.0.0.1:${PORT}\"",
        "OK2: 'http://127.0.0.1:${PORT}/v1?q=1'",
    ] {
        e.push(format!("      {l}"));
    }
    let (pk, w) = run_s(&block(&[e]));
    assert_eq!(
        codes(&w),
        [
            "leetcode.packages.expose_name_invalid",
            "leetcode.packages.expose_name_reserved",
            "leetcode.packages.expose_name_reserved",
            "leetcode.packages.expose_name_reserved",
            "leetcode.packages.expose_name_reserved",
            "leetcode.packages.expose_name_reserved",
            "leetcode.packages.expose_name_reserved",
            "leetcode.packages.expose_value_not_loopback",
            "leetcode.packages.expose_value_not_loopback",
            "leetcode.packages.expose_value_not_loopback",
            "leetcode.packages.expose_value_not_loopback",
            "leetcode.packages.expose_value_not_loopback",
            "leetcode.packages.expose_value_not_loopback",
            "leetcode.packages.expose_name_invalid",
        ]
    );
    assert_eq!(
        (p(&w[7], "name"), p(&w[7], "value")),
        ("API_PORT", "${PORT}")
    );
    let keys: Vec<_> = pk[0]
        .expose_as
        .as_ref()
        .unwrap()
        .0
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, ["OK_URL", "OK2"]);
}

#[test]
fn expose_as_first_field_does_not_swallow_siblings() {
    let (pk, w) = run(&[
        "packages:",
        "  - exposeAs:",
        "      A_URL: \"http://127.0.0.1:${PORT}\"",
        "    name: api",
        "    dir: d",
        "    install: [\"i\"]",
        "    start: [\"s\"]",
    ]);
    assert!(w.is_empty());
    assert_eq!(pk[0].name, "api");
    assert!(pk[0].expose_as.is_some());
}

#[test]
fn proto_field_ignored() {
    let (pk, w) = run_s(&block(&[entry(
        "a",
        &["    __proto__: x", "    constructor: y"],
    )]));
    assert_eq!((pk.len(), w.len()), (1, 0));
}

#[test]
fn is_safe_expose_name_table() {
    for ok in ["VITE_API_URL", "A", "X_1"] {
        assert!(is_safe_expose_name(ok), "{ok}");
    }
    for bad in [
        "PATH",
        "PORT",
        "NODE_OPTIONS",
        "NODE_PATH",
        "VIRTUAL_ENV",
        "CLASSPATH",
        "CARGO_TARGET_DIR",
        "DYLD_INSERT_LIBRARIES",
        "lower",
        "",
        "1A",
        "__proto__",
        "A-B",
        "RUSTFLAGS",
        "JAVA_TOOL_OPTIONS",
        "PYTHONSTARTUP",
        "LD_AUDIT",
        "NODE_EXTRA_CA_CERTS",
        "NODE_REPL_EXTERNAL_MODULE",
        "PYTHONPATH",
        "LD_LIBRARY_PATH",
        "JDK_JAVA_OPTIONS",
        "LD_PRELOAD",
    ] {
        assert!(!is_safe_expose_name(bad), "{bad}");
    }
}

/// `conformance/leetcode/packages-parser/*`. Differences from the export, both recorded: its cycle
/// `path` joins with ` → ` (spec W36: `, `), and its `argv_substitution` carries a stray `PORT`
/// param (a template-literal artefact of the exporter; spec W30 lists `name`, `field`, `element`).
#[test]
fn conformance_cases() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/leetcode/packages-parser"
    );
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        if path.extension().is_none_or(|x| x != "md") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let raw = extract_config_blocks(split_frontmatter(&src).1).raw;
        let lines: Vec<&str> = js_lines(&raw).collect();
        let (pk, w) = run(&lines);
        let exp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path.with_extension("expected.json")).unwrap(),
        )
        .unwrap();
        let want_pk = exp
            .get("packages")
            .cloned()
            .unwrap_or(serde_json::json!([]));
        assert_eq!(serde_json::to_value(&pk).unwrap(), want_pk, "{path:?}");
        let mut want_w = exp["warnings"].as_array().unwrap().clone();
        for x in &mut want_w {
            if let Some(m) = x["params"].as_object_mut() {
                m.remove("PORT");
                if let Some(s) = m
                    .get("path")
                    .and_then(|s| s.as_str())
                    .map(|s| s.replace(" → ", ", "))
                {
                    m.insert("path".into(), s.into());
                }
            }
        }
        let got: Vec<_> = w.iter().map(|x| serde_json::to_value(x).unwrap()).collect();
        assert_eq!(got, want_w, "{path:?}");
        n += 1;
    }
    assert_eq!(n, 8);
}

#[test]
fn hostile_100k_depends_on_is_linear() {
    let names: Vec<String> = (0..100_000).map(|i| format!("x{i}")).collect();
    let dep = format!("    dependsOn: [{}]", names.join(","));
    let t = std::time::Instant::now();
    let (pk, w) = run_s(&block(&[entry("a", &[&dep])]));
    assert!(t.elapsed().as_secs() < 1, "took {:?}", t.elapsed());
    assert_eq!((pk.len(), w.len()), (1, 0));
}
