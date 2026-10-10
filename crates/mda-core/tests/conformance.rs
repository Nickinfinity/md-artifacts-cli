//! Conformance harness: walks `conformance/manifest.toml`. Adding a case later needs only a kind
//! handler in `run_case`, not a change to the walker.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test file: a failing test is the point

use mda_core::serialize::SerializeError;
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
    /// Render only: the block to select (`render_artifact`'s `block`).
    block: Option<usize>,
    path: Option<String>,
    #[allow(dead_code)] // reason: provenance, read by humans
    origin: Option<String>,
    #[allow(dead_code)] // recorded decision, read by humans and later kind handlers
    deviates_from_ts: Option<String>,
}

#[path = "conformance/selftest.rs"]
mod selftest;

/// The engine's output limit (`MAX_ARTIFACT_BYTES`), the bound `artifact.create` passes.
const MAX: usize = 1 << 20;

/// First JSON pointer where `a` and `b` differ (object key order is ignored by design).
fn first_diff(a: &serde_json::Value, b: &serde_json::Value, ptr: &str) -> Option<String> {
    use serde_json::Value::{Array, Object};
    let leaf = || Some(format!("{ptr}: got {a} want {b}"));
    match (a, b) {
        (Object(x), Object(y)) => {
            x.keys()
                .chain(y.keys())
                .find_map(|k| match (x.get(k), y.get(k)) {
                    (Some(l), Some(r)) => first_diff(l, r, &format!("{ptr}/{k}")),
                    (l, r) => Some(format!("{ptr}/{k}: got {l:?} want {r:?}")),
                })
        }
        (Array(x), Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (l, r))| first_diff(l, r, &format!("{ptr}/{i}"))),
        _ if a == b => None,
        _ => leaf(),
    }
}

fn run_parse(dir: &Path, c: &Case) -> Result<(), String> {
    let (Some(path), Some(exp)) = (&c.path, &c.expected) else {
        return Err(format!("{}: parse needs path and expected", c.id));
    };
    let io = |e: std::io::Error| format!("{}: {e}", c.id);
    let bytes = std::fs::read(dir.join(&c.input)).map_err(io)?;
    let parsed = mda_core::parse::parse_from_content(&mda_core::parse::decode(&bytes), path);
    let got = serde_json::to_value(parsed).map_err(|e| format!("{}: {e}", c.id))?;
    let want = std::fs::read_to_string(dir.join(exp)).map_err(io)?;
    let want: serde_json::Value =
        serde_json::from_str(&want).map_err(|e| format!("{}: expected: {e}", c.id))?;
    match first_diff(&got, &want, "") {
        None => Ok(()),
        Some(d) => Err(format!("{}: differs at {d}", c.id)),
    }
}

/// Model JSON -> `serialize` -> exact bytes. Compares bytes (not parse JSON): the serializer's own
/// round-trip guard is the model-level comparator.
fn run_serialize(dir: &Path, c: &Case) -> Result<(), String> {
    let Some(exp) = &c.expected else {
        return Err(format!("{}: serialize needs expected", c.id));
    };
    let io = |e: std::io::Error| format!("{}: {e}", c.id);
    let model = std::fs::read_to_string(dir.join(&c.input)).map_err(io)?;
    let model: mda_core::model::ArtifactModel =
        serde_json::from_str(&model).map_err(|e| format!("{}: model: {e}", c.id))?;
    let got = mda_core::serialize::serialize(&model, MAX)
        .map_err(|e| format!("{}: refused {e:?}", c.id))?;
    let want = std::fs::read_to_string(dir.join(exp)).map_err(io)?;
    let (mut g, mut w) = (got.split('\n'), want.split('\n'));
    for n in 1.. {
        match (g.next(), w.next()) {
            (None, None) => return Ok(()),
            (a, b) if a == b => {}
            (a, b) => return Err(format!("{}: line {n}: got {a:?} want {b:?}", c.id)),
        }
    }
    Ok(())
}

/// Parse + render the input with the client values; compares the whole `Rendered` JSON, so a
/// warning regression cannot hide behind an unchanged `output`.
fn run_render(dir: &Path, c: &Case) -> Result<(), String> {
    use mda_core::render::{Values, check_values, render_artifact};
    let (Some(path), Some(exp)) = (&c.path, &c.expected) else {
        return Err(format!("{}: render needs path and expected", c.id));
    };
    let io = |e: std::io::Error| format!("{}: {e}", c.id);
    let values: Values = match &c.values {
        Some(v) => serde_json::from_str(&std::fs::read_to_string(dir.join(v)).map_err(io)?)
            .map_err(|e| format!("{}: values: {e}", c.id))?,
        None => Values::new(),
    };
    check_values(&values).map_err(|e| format!("{}: values: {}", c.id, e.code))?;
    let bytes = std::fs::read(dir.join(&c.input)).map_err(io)?;
    let parsed = mda_core::parse::parse_from_content(&mda_core::parse::decode(&bytes), path);
    let r = render_artifact(&parsed, c.block, None, &values)
        .map_err(|e| format!("{}: refused {e:?}", c.id))?;
    let got = serde_json::to_value(r).map_err(|e| format!("{}: {e}", c.id))?;
    let want = std::fs::read_to_string(dir.join(exp)).map_err(io)?;
    let want: serde_json::Value =
        serde_json::from_str(&want).map_err(|e| format!("{}: expected: {e}", c.id))?;
    match first_diff(&got, &want, "") {
        None => Ok(()),
        Some(d) => Err(format!("{}: differs at {d}", c.id)),
    }
}

/// The per-kind handler. Never skips.
fn run_case(dir: &Path, c: &Case) -> Result<(), String> {
    match c.kind.as_str() {
        "parse" => run_parse(dir, c),
        "serialize" => run_serialize(dir, c),
        "render" => run_render(dir, c),
        k => Err(format!("{}: unknown kind {k}", c.id)),
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
        if let Err(e) = run_case(dir, c) {
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

#[test]
fn real_manifest_ok() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    assert_eq!(check(&dir), Ok(()));
}

/// Parse cases the serializer refuses by design (B-RISK 3): id -> reason. Anything else refused fails.
const ROUND_TRIP_REFUSALS: &[(&str, &str, &str)] = &[(
    "q-legacy-dup-appended-twice",
    "vks.duplicate_key",
    "legacy duplicate VK key: YAML mapping cannot carry it (vks.duplicate_key)",
)];

/// D-9: every clean `parse` case re-serializes through the guard (`Ok` is the assertion).
#[test]
fn parse_cases_round_trip() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    let m: Manifest =
        toml::from_str(&std::fs::read_to_string(dir.join("manifest.toml")).unwrap()).unwrap();
    let (mut bad, mut seen) = (Vec::new(), Vec::new());
    for c in m.cases.iter().filter(|c| c.kind == "parse") {
        let content = mda_core::parse::decode(&std::fs::read(dir.join(&c.input)).unwrap());
        let p = mda_core::parse::parse_from_content(&content, c.path.as_deref().unwrap());
        if p.vars_error.is_some()
            || p.blocks.iter().any(|b| b.vars_error.is_some())
            || p.frontmatter.index == Some(true)
            || mda_core::parse::is_flagged(&content)
        {
            continue;
        }
        let model = mda_core::serialize::from_parsed(&p);
        if let Err(e) = mda_core::serialize::serialize(&model, MAX) {
            seen.push(c.id.as_str());
            let code = match &e {
                SerializeError::Vars(v) => v.code,
                SerializeError::Field { .. } => "artifact.unrepresentable",
                SerializeError::Vks(_) => "vks.unrepresentable",
                SerializeError::TooLarge { .. } => "file.too_large",
            };
            if !ROUND_TRIP_REFUSALS
                .iter()
                .any(|(id, k, _)| *id == c.id && *k == code)
            {
                bad.push(format!("{}: refused {e:?}", c.id));
            }
        }
    }
    for (id, _, _) in ROUND_TRIP_REFUSALS {
        if !seen.contains(id) {
            bad.push(format!("{id}: listed but not refused (stale entry)"));
        }
    }
    eprintln!("round-trip refusals: {seen:#?}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
