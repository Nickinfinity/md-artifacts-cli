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
    path: Option<String>,
    #[allow(dead_code)] // reason: provenance, read by humans
    origin: Option<String>,
    #[allow(dead_code)] // recorded decision, read by humans and later kind handlers
    deviates_from_ts: Option<String>,
}

/// The engine's output limit (`MAX_ARTIFACT_BYTES`), the bound `artifact.create` passes.
const MAX: usize = 1 << 20;

/// Wave that implements each recognised kind; `None` = unknown kind.
fn kind_wave(kind: &str) -> Option<&'static str> {
    match kind {
        "parse" => Some("W1"),
        "serialize" => Some("W2"),
        "render" => Some("W3"),
        _ => None,
    }
}

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

/// The per-kind handler. Fails loudly until the wave lands; never skips.
fn run_case(dir: &Path, c: &Case) -> Result<(), String> {
    match c.kind.as_str() {
        "parse" => return run_parse(dir, c),
        "serialize" => return run_serialize(dir, c),
        _ => {}
    }
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

fn tmp_with(name: &str, manifest: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("manifest.toml"), manifest).unwrap();
    for (f, bytes) in files {
        std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
        std::fs::write(d.join(f), bytes).unwrap();
    }
    d
}

fn tmp(name: &str, manifest: &str, files: &[&str]) -> PathBuf {
    let files: Vec<(&str, &[u8])> = files.iter().map(|f| (*f, &b"x"[..])).collect();
    tmp_with(name, manifest, &files)
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
fn listed_render_case_not_implemented() {
    let d = tmp(
        "notimpl",
        &case("c1", "render", "render/a.json"),
        &["render/a.json"],
    );
    assert!(err_of(&d).contains("c1: kind render not implemented until W3"));
}

const MODEL: &str = r#"{"artifactType":"Snippet","title":"Hi","blocks":[{"code":"x"}]}"#;
const MODEL_MD: &str = "---\nartifactType: Snippet\ntitle: Hi\n---\n\n```\nx\n```\n";

fn ser_case(model: &str, md: &str, name: &str) -> PathBuf {
    let m = format!(
        "{}expected = \"serialize/a.md\"\n",
        case("s1", "serialize", "serialize/a.model.json")
    );
    tmp_with(
        name,
        &m,
        &[
            ("serialize/a.model.json", model.as_bytes()),
            ("serialize/a.md", md.as_bytes()),
        ],
    )
}

#[test]
fn serialize_case_matches() {
    let d = ser_case(MODEL, MODEL_MD, "sok");
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn serialize_diff_names_id_and_line() {
    let d = ser_case(
        MODEL,
        "---\nartifactType: Snippet\ntitle: Wrong\n---\n\n```\nx\n```\n",
        "sdiff",
    );
    let e = err_of(&d);
    assert!(e.contains("s1: line 3: got") && e.contains("Wrong"), "{e}");
}

#[test]
fn serialize_unknown_model_field_names_id() {
    let d = ser_case(r#"{"artifactType":"Snippet","bogus":1}"#, MODEL_MD, "sunk");
    let e = err_of(&d);
    assert!(e.contains("s1") && e.contains("bogus"), "{e}");
}

#[test]
fn serialize_refusal_names_id() {
    let d = ser_case(
        r#"{"artifactType":"Template","blocks":[{},{}]}"#,
        MODEL_MD,
        "sref",
    );
    assert!(err_of(&d).contains("s1: refused"));
}

#[test]
fn serialize_case_without_expected_names_id() {
    let m = case("s1", "serialize", "serialize/a.model.json");
    let d = tmp_with(
        "snoexp",
        &m,
        &[("serialize/a.model.json", MODEL.as_bytes())],
    );
    assert!(err_of(&d).contains("s1: serialize needs expected"));
}

const MD: &[u8] = b"---\ntitle: Hi\n---\n";

fn parse_case(extra: &str) -> String {
    format!("{}{extra}", case("p1", "parse", "parse/a.md"))
}

/// Expected JSON for `MD`, taken from the real parser so the test needs no hand-written model.
fn expected_json(bytes: &[u8]) -> String {
    let p = mda_core::parse::parse_from_content(&mda_core::parse::decode(bytes), "a.md");
    serde_json::to_string(&p).unwrap()
}

#[test]
fn parse_case_matches() {
    let m = parse_case("path = \"a.md\"\nexpected = \"parse/a.json\"\n");
    let exp = expected_json(MD);
    let d = tmp_with(
        "pok",
        &m,
        &[("parse/a.md", MD), ("parse/a.json", exp.as_bytes())],
    );
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn parse_case_diff_names_pointer() {
    let m = parse_case("path = \"a.md\"\nexpected = \"parse/a.json\"\n");
    let mut v: serde_json::Value = serde_json::from_str(&expected_json(MD)).unwrap();
    v["frontmatter"]["title"] = "Wrong".into();
    let d = tmp_with(
        "pdiff",
        &m,
        &[
            ("parse/a.md", MD),
            ("parse/a.json", v.to_string().as_bytes()),
        ],
    );
    let e = err_of(&d);
    assert!(e.contains("p1") && e.contains("/frontmatter/title"), "{e}");
}

#[test]
fn parse_case_without_path_names_id() {
    let m = parse_case("expected = \"parse/a.json\"\n");
    let d = tmp_with(
        "pnopath",
        &m,
        &[("parse/a.md", MD), ("parse/a.json", b"{}")],
    );
    assert!(err_of(&d).contains("p1: parse needs path and expected"));
}

#[test]
fn parse_case_decodes_bom() {
    let bom = [&b"\xEF\xBB\xBF"[..], MD].concat();
    let m = parse_case("path = \"a.md\"\nexpected = \"parse/a.json\"\n");
    let exp = expected_json(MD); // no-BOM parse: BOM must have been stripped
    let d = tmp_with(
        "pbom",
        &m,
        &[("parse/a.md", &bom), ("parse/a.json", exp.as_bytes())],
    );
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
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
