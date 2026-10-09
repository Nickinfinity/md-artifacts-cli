//! Conformance harness: walks `conformance/manifest.toml`. Adding a case later needs only a kind
//! handler in `run_case`, not a change to the walker.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test file: a failing test is the point

use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    kind: String,
    input: String,
    expected: Option<String>,
    values: Option<String>,
    #[allow(dead_code)] // recorded decision, read by humans and later kind handlers
    deviates_from_ts: Option<String>,
}

/// Wave that implements each recognised kind; `None` = unknown kind.
fn kind_wave(kind: &str) -> Option<&'static str> {
    match kind {
        "parse" => Some("W1"),
        "serialize" => Some("W2"),
        "render" => Some("W3"),
        _ => None,
    }
}

/// The per-kind handler. Fails loudly until the wave lands; never skips.
fn run_case(c: &Case) -> Result<(), String> {
    match kind_wave(&c.kind) {
        Some(w) => Err(format!(
            "{}: kind {} not implemented until {w}",
            c.id, c.kind
        )),
        None => Err(format!("{}: unknown kind {}", c.id, c.kind)),
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    }; // absent dir tolerated
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn check(dir: &Path) -> Result<(), Vec<String>> {
    let text = std::fs::read_to_string(dir.join("manifest.toml"))
        .map_err(|e| vec![format!("manifest.toml: {e}")])?;
    let m: Manifest = toml::from_str(&text).map_err(|e| vec![format!("manifest.toml: {e}")])?;
    let mut errs = Vec::new();
    let mut ids = HashSet::new();
    let mut listed: HashSet<PathBuf> = HashSet::new();
    for c in &m.cases {
        if !ids.insert(c.id.as_str()) {
            errs.push(format!("{}: duplicate id", c.id));
        }
        for rel in [Some(&c.input), c.expected.as_ref(), c.values.as_ref()]
            .into_iter()
            .flatten()
        {
            listed.insert(dir.join(rel));
            if !dir.join(rel).is_file() {
                errs.push(format!("{}: missing file {rel}", c.id));
            }
        }
        if let Err(e) = run_case(c) {
            errs.push(e);
        }
    }
    let mut files = Vec::new();
    for sub in ["parse", "serialize", "render"] {
        walk(&dir.join(sub), &mut files);
    }
    for f in files {
        if !listed.contains(&f) {
            errs.push(format!("unlisted file {}", f.display()));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn tmp(name: &str, manifest: &str, files: &[&str]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("manifest.toml"), manifest).unwrap();
    for f in files {
        std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
        std::fs::write(d.join(f), "x").unwrap();
    }
    d
}

fn err_of(d: &Path) -> String {
    let e = check(d).unwrap_err().join("\n");
    std::fs::remove_dir_all(d).unwrap();
    e
}

fn case(id: &str, kind: &str, input: &str) -> String {
    format!("[[cases]]\nid = \"{id}\"\nkind = \"{kind}\"\ninput = \"{input}\"\n")
}

#[test]
fn unknown_kind_names_id() {
    let d = tmp(
        "bogus",
        &case("c-bogus", "bogus", "parse/a.md"),
        &["parse/a.md"],
    );
    assert!(err_of(&d).contains("c-bogus"));
}

#[test]
fn missing_input_names_path() {
    let d = tmp("missing", &case("c1", "bogus", "parse/gone.md"), &[]);
    assert!(err_of(&d).contains("parse/gone.md"));
}

#[test]
fn unlisted_file_named() {
    let d = tmp("unlisted", "", &["parse/stray.md"]);
    assert!(err_of(&d).contains("stray.md"));
}

#[test]
fn listed_parse_case_not_implemented() {
    let d = tmp(
        "notimpl",
        &case("c1", "parse", "parse/a.md"),
        &["parse/a.md"],
    );
    assert!(err_of(&d).contains("c1: kind parse not implemented until W1"));
}

#[test]
fn duplicate_id_fails() {
    let m = format!(
        "{}{}",
        case("dup", "bogus", "parse/a.md"),
        case("dup", "bogus", "parse/a.md")
    );
    let d = tmp("dup", &m, &["parse/a.md"]);
    assert!(err_of(&d).contains("dup: duplicate id"));
}

#[test]
fn typo_field_is_rejected() {
    let m = format!("{}expectd = \"x\"\n", case("c1", "parse", "parse/a.md"));
    let d = tmp("typo", &m, &["parse/a.md"]);
    assert!(err_of(&d).contains("expectd"));
}

#[test]
fn real_manifest_ok() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    assert_eq!(check(&dir), Ok(()));
}
