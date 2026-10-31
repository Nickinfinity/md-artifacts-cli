//! Debug mode through the real binary: stdout stays clean, the log goes where told, secrets stay out.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: tests fail loudly

mod common;
use common::{bin, json_lines, run};
use std::process::Stdio;

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"2.0"}}"#;
const OPS: &str = r#"{"jsonrpc":"2.0","id":2,"method":"system.ops"}"#;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mda-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn has_op_line(log: &str) -> bool {
    log.lines()
        .any(|l| l.contains(r#"op "system.ops""#) && l.contains("µs"))
}

#[test]
fn debug_does_not_change_stdout() {
    let plain = run(&["system", "ops", "--json"], b"");
    let dbg = run(&["system", "ops", "--json", "--debug"], b"");
    assert!(plain.status.success() && dbg.status.success());
    assert_eq!(plain.stdout, dbg.stdout);
    assert!(plain.stderr.is_empty());
    let err = String::from_utf8(dbg.stderr).unwrap();
    assert!(has_op_line(&err), "stderr: {err}");
}

#[test]
fn trace_serve_stdout_is_protocol_and_warning_is_first() {
    let input = format!("{INIT}\n{OPS}\n");
    let mut child = bin()
        .arg("serve")
        .env("MDA_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(json_lines(&out.stdout).len(), 2);
    let err = String::from_utf8(out.stderr).unwrap();
    let first = err.lines().next().unwrap();
    assert!(
        first.contains("WARN mda::debug trace output may include secrets"),
        "{first}"
    );
    assert!(has_op_line(&err), "stderr: {err}");
}

#[test]
fn log_file_gets_lines_and_stderr_is_empty() {
    let d = tmp("logfile");
    let f = d.join("x.log");
    let out = run(
        &[
            "system",
            "ops",
            "--json",
            "--debug",
            "--log-file",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert!(out.status.success());
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
    assert!(has_op_line(&std::fs::read_to_string(&f).unwrap()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn bad_log_file_dir_exits_3_naming_io_failed() {
    let d = tmp("badlog");
    let f = d.join("missing").join("x.log");
    let out = run(
        &[
            "system",
            "ops",
            "--debug",
            "--log-file",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .starts_with("io.failed: ")
    );
    assert!(!d.join("missing").exists());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn params_secret_never_logged_at_debug() {
    let out = run(
        &["call", "system.ops", r#"{"secret":"s3cr3t"}"#, "--debug"],
        b"",
    );
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("op.bad_request"), "stderr: {err}");
    assert!(!err.contains("s3cr3t"), "stderr: {err}");
    // The CLI's own error line may name the key; the debug log lines must not.
    let log: Vec<&str> = err.lines().filter(|l| l.contains(" DEBUG ")).collect();
    assert!(
        !log.is_empty() && log.iter().all(|l| !l.contains("secret")),
        "{log:?}"
    );
}

#[test]
fn debug_show_keeps_values_out_of_stderr() {
    let vault = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault");
    let out = run(
        &[
            "--vault",
            vault,
            "--debug",
            "artifact",
            "show",
            "Snippets/hello.md",
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("marker-7f3a")
    );
    assert!(
        !String::from_utf8(out.stderr)
            .unwrap()
            .contains("marker-7f3a")
    );
}

#[test]
fn debug_write_keeps_model_values_out_of_stderr() {
    let d = tmp("dbgwrite");
    std::fs::create_dir_all(d.join(".obsidian")).unwrap();
    let m = d.join("m.json");
    std::fs::write(
        &m,
        r#"{"artifactType":"Template","title":"N","blocks":[{"language":"ts","code":"marker-9c1e"}]}"#,
    )
    .unwrap();
    let out = run(
        &[
            "--vault",
            d.to_str().unwrap(),
            "--debug",
            "artifact",
            "new",
            "Templates/n.md",
            "--model",
            m.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert!(
        std::fs::read_to_string(d.join("Templates/n.md"))
            .unwrap()
            .contains("marker-9c1e")
    );
    assert!(
        !String::from_utf8(out.stderr)
            .unwrap()
            .contains("marker-9c1e")
    );
    std::fs::remove_dir_all(d).unwrap();
}

// L-9: debug never logs variable values.
#[test]
fn debug_render_does_not_log_values() {
    let out = run(
        &[
            "--vault",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault"),
            "--debug",
            "artifact",
            "render",
            "Snippets/hello.md",
            "--values",
            "-",
        ],
        br#"{"VK-name":"marker-3b9e"}"#,
    );
    assert_eq!(out.status.code(), Some(0));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(!err.contains("marker-3b9e"), "{err}");
    // Not vacuous: the debug logger ran, and the value really rendered.
    assert!(err.contains("artifact.render"), "{err}");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("marker-3b9e")
    );
}

// L-9: `--debug index run` never logs variable values.
#[test]
fn debug_index_run_does_not_log_values() {
    let ws = tmp("idx-run");
    let vals = ws.join("v.json");
    std::fs::write(&vals, r#"{"VK-name":"marker-9c4d"}"#).unwrap();
    let steps = ws.join("s.json");
    std::fs::write(&steps, r#"[{"relPath":"edit.md","destDir":""}]"#).unwrap();
    let out = run(
        &[
            "--vault",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault"),
            "--debug",
            "index",
            "run",
            "Templates/index.md",
            "--workspace",
            ws.to_str().unwrap(),
            "--steps",
            steps.to_str().unwrap(),
            "--values",
            vals.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(out.status.code(), Some(0));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(!err.contains("marker-9c4d"), "{err}");
    assert!(err.contains("index.run"), "{err}");
    assert!(String::from_utf8(out.stdout).unwrap().contains("1 written"));
    assert!(
        std::fs::read_to_string(ws.join("Edit me.ts"))
            .unwrap()
            .contains("marker-9c4d")
    );
    std::fs::remove_dir_all(ws).unwrap();
}

#[test]
fn debug_select_never_logs_machine_file_bytes() {
    let cfg = tmp("sel-cfg");
    std::fs::write(cfg.join("config.toml"), "marker = \"m-4d1e\"\n").unwrap();
    let v = tmp("sel-vault");
    std::fs::create_dir(v.join(".obsidian")).unwrap();
    let out = common::run_cfg(
        &["--debug=debug", "vault", "select", v.to_str().unwrap()],
        b"",
        &cfg,
    );
    assert_eq!(out.status.code(), Some(0));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains(r#"op "vault.select""#), "stderr: {err}");
    assert!(!err.contains("m-4d1e"), "stderr: {err}");
}

#[test]
fn debug_vars_set_keeps_values_out_of_stderr() {
    let d = tmp("varsset");
    common::copy_tree(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault")),
        &d,
    );
    let v = d.to_str().unwrap();
    let h = "c42fac55361cbad76ff13f44e1429d039493ed1d46e60598e4774ba30b03b0cb";
    let args = |lvl: &'static str| {
        [
            "--vault",
            v,
            lvl,
            "vars",
            "var",
            "set",
            "Variables/flat.md",
            "VK-host",
            "--value",
            "SECRET-m5v",
            "--hash",
            h,
        ]
    };
    let o = run(&args("--debug"), b"");
    assert_eq!(o.status.code(), Some(0), "{:?}", o.stderr);
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(err.contains(r#"op "vars.var.set""#), "{err}");
    assert!(!err.contains("SECRET-m5v"), "{err}");
    std::fs::remove_dir_all(&d).unwrap();
    common::copy_tree(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault")),
        &d,
    );
    let o = run(&args("--debug=trace"), b"");
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(
        err.lines()
            .next()
            .unwrap()
            .contains("trace output may include secrets"),
        "{err}"
    );
    assert!(err.contains("SECRET-m5v"), "{err}");
    std::fs::remove_dir_all(&d).unwrap();
}

const VARS_FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault_vars");

#[test]
fn debug_vars_search_keeps_query_out_of_stderr() {
    let args = |lvl: &'static str| ["--vault", VARS_FIX, lvl, "vars", "search", "SECRET-q9"];
    let err = String::from_utf8(run(&args("--debug"), b"").stderr).unwrap();
    assert!(err.contains(r#"op "vars.search""#), "{err}");
    assert!(!err.contains("SECRET-q9"), "{err}");
    let err = String::from_utf8(run(&args("--debug=trace"), b"").stderr).unwrap();
    assert!(
        err.lines()
            .next()
            .unwrap()
            .contains("trace output may include secrets"),
        "{err}"
    );
    assert!(err.contains("SECRET-q9"), "{err}");
}

#[test]
fn debug_varsets_apply_keeps_values_out() {
    let d = tmp("applyvals");
    let f = d.join("v.json");
    std::fs::write(&f, r#"{"VK-host":"SECRET-v4"}"#).unwrap();
    let o = run(
        &[
            "--vault",
            VARS_FIX,
            "--debug",
            "varsets",
            "apply",
            "Variables/dev.md",
            "--values",
            f.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(o.status.code(), Some(0), "{:?}", o.stderr);
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(err.contains(r#"op "varsets.apply""#), "{err}");
    assert!(!err.contains("SECRET-v4"), "{err}");
    std::fs::remove_dir_all(&d).unwrap();
}

/// `seen` proves the secret file was actually processed: `Some(bytes)` = its migrated content,
/// `None` = its path appears on stdout (validate lists it: its secret sits in an untagged fence).
fn debug_vault_run(tag: &str, args: &[&str], ops: &[&str], seen: Option<&str>) {
    let v = tmp(tag);
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault_migrate");
    common::copy_tree(&src, &v);
    std::fs::write(
        v.join("Variables/s.md"),
        "---\nartifactType: Variables\ntitle: S\n---\n\n```yaml\nVK-a: SECRET-mig-77\n```\n",
    )
    .unwrap();
    let mut a = vec!["--vault", v.to_str().unwrap(), "--debug"];
    a.extend_from_slice(args);
    let o = run(&a, b"");
    let err = String::from_utf8(o.stderr).unwrap();
    for op in ops {
        assert!(err.contains(&format!(r#"op "{op}""#)), "{err}");
    }
    assert!(!err.contains("SECRET-mig-77"), "{err}");
    match seen {
        Some(want) => assert_eq!(
            std::fs::read_to_string(v.join("Variables/s.md")).unwrap(),
            want
        ),
        None => assert!(
            String::from_utf8(o.stdout)
                .unwrap()
                .contains("Variables/s.md")
        ),
    }
    std::fs::remove_dir_all(&v).unwrap();
}

#[test]
fn debug_validate_keeps_values_out() {
    debug_vault_run("valdbg", &["vault", "validate"], &["vault.validate"], None);
}
